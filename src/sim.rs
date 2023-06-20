use crate::id::PlayerId;
use crate::{Ballpark, Database, Date, Game, Inning, Player, Rng, Sim, TeamSelect};
use std::cmp::Ordering;
use std::fmt::Write;
use std::ops::ControlFlow;

// some consts, so we know what to fix when we start implementing modifications
const BALLS_NEEDED: u8 = 4;
const STRIKES_NEEDED: u8 = 3;
const OUTS_NEEDED: u8 = 3;
const HOME_BASE: u8 = 4;

// some newtypes so i write fewer bugs
struct Batter<'a>(&'a Player);
struct Fielder<'a>(&'a Player);
struct Pitcher<'a>(&'a Player);

impl Sim {
    // TODO: Right now this returns nothing, but in the future I'd like it to return a batch of
    // events (think The Feed) that can get shoved into a database. (Databases are strictly outside
    // the scope of this crate.)
    pub fn tick(&mut self) {
        // We're splitting these apart to tell/convince the borrow checker that these are separate
        // mutable borrows. This lets us hold a mutable reference to something in the database and
        // still be able to roll the RNG.
        let rng = &mut self.rng;
        let database = &mut self.database;

        // To mutably borrow an individual game and the rest of the sim at the same time, we swap
        // the game out of the sim (replacing it with a default nil game), run `Game::tick`, and
        // then swap the game back into the sim.
        for i in 0..database.games_today.len() {
            if !database.games_today[i].is_finished() {
                let mut game = std::mem::take(&mut database.games_today[i]);
                game.last_update = into_update(game.tick(rng, database));
                database.games_today[i] = game;
            }
        }

        // In debug mode (dev/test profiles), panic if we've introduced a database consistency
        // problem.
        database.debug_check();
    }
}

enum Never {}

fn into_update(c: ControlFlow<String, Never>) -> String {
    match c {
        ControlFlow::Continue(nothing) => match nothing {},
        ControlFlow::Break(update) => update,
    }
}

impl Game {
    fn tick(&mut self, rng: &mut Rng, database: &mut Database) -> ControlFlow<String, Never> {
        self.handle_game_over(database)?;
        if self.inning == Inning::default() {
            self.inning = Inning::Top(1);
            return ControlFlow::Break("Play ball!".into());
        }
        if matches!(self.inning, Inning::Mid(_) | Inning::End(_)) {
            self.inning.advance();
            return ControlFlow::Break(format!(
                "{} of {}, {} batting.",
                self.inning.word(),
                self.inning.number(),
                self.teams
                    .select(self.inning.batting())
                    .id
                    .load(database)
                    .name(),
            ));
        }

        let pitcher = self.get_pitcher(rng, database);
        let batter = self.get_batter(rng, database)?;
        let pitcher = Pitcher(pitcher.load(database));
        let batter = Batter(batter.load(database));

        self.handle_steal(rng, database)?;
        let strike = roll_strike(rng, database.date, &pitcher, &batter);
        if !roll_swing(rng, database.date, &pitcher, &batter, strike) {
            return if strike {
                self.handle_strike(&batter, "looking")
            } else {
                self.handle_ball(&batter, database)
            };
        }
        if !roll_contact(rng, database.date, &pitcher, &batter, strike) {
            return self.handle_strike(&batter, "swinging");
        }
        if roll_foul(rng, database.date, &batter) {
            self.strikes = 2.min(self.strikes + 1);
            return ControlFlow::Break(format!("Foul Ball. {}-{}", self.balls, self.strikes));
        }
        let fielder = self.roll_fielder(rng, database);
        if roll_out(rng, database.date, &pitcher, &fielder, &batter) {
            // TODO: double play / fielder's choice
            let kind = if roll_flyout(rng, &batter) {
                "flyout"
            } else {
                // TODO: ground out advances
                "ground out"
            };
            return ControlFlow::Break(format!(
                "{} hit a {} to {}.",
                batter.0.name, kind, fielder.0.name
            ));
        }
        if roll_home_run(rng, database.date, &pitcher, &batter) {
            return self.handle_home_run(&batter);
        }
        let defender = self.roll_fielder(rng, database);
        self.handle_base_hit(
            &batter,
            database,
            roll_base_hit(rng, database.date, &pitcher, &defender, &batter),
        )
    }
}

macro_rules! next_in_order {
    (
        rng = $rng:expr,
        database = $db:expr,
        current = $current:expr,
        data = $data:expr,
        pos = $pos:expr,
        field = $team_field:ident,
        new_name = $new_name:expr,
    ) => {
        if let Some(current) = $current {
            ControlFlow::Continue(current)
        } else {
            let pos = $pos;
            let data = $data;

            let player = if let Some(player) = {
                let team = data.id.load($db);
                team.$team_field
                    .get(pos)
                    .or_else(|| team.$team_field.first())
                    .copied()
            } {
                player
            } else {
                let player = Player::generate_with_name($rng, $new_name.to_string());
                let player_id = player.id;
                $db.players.insert(player_id, player);
                data.id.load_mut($db).$team_field.push(player_id);
                player_id
            };

            // `Option::expect`: At this point, `player` was either fetched from `$team_field`, or a
            // Machine was generated and pushed to `$team_field`.
            // TODO: We can probably return the index in the above logic.
            $pos = data
                .id
                .load($db)
                .$team_field
                .iter()
                .position(|x| *x == player)
                .expect("player not in order");
            $current = Some(player);
            ControlFlow::Break(player)
        }
    };
}

impl Game {
    fn get_pitcher(&mut self, rng: &mut Rng, database: &mut Database) -> PlayerId {
        match next_in_order!(
            rng = rng,
            database = database,
            current = self.teams.select_mut(self.inning.fielding()).pitcher,
            data = self.teams.select_mut(self.inning.fielding()),
            pos = self
                .teams
                .select_mut(self.inning.fielding())
                .id
                .load_mut(database)
                .rotation_slot,
            field = rotation,
            new_name = "Pitching Machine",
        ) {
            ControlFlow::Continue(player) | ControlFlow::Break(player) => player,
        }
    }

    fn get_batter(
        &mut self,
        rng: &mut Rng,
        database: &mut Database,
    ) -> ControlFlow<String, PlayerId> {
        match next_in_order!(
            rng = rng,
            database = database,
            current = self.at_bat,
            data = self.teams.select_mut(self.inning.batting()),
            pos = self.teams.select_mut(self.inning.batting()).lineup_slot,
            field = lineup,
            new_name = "Batting Machine",
        ) {
            ControlFlow::Continue(player) => ControlFlow::Continue(player),
            ControlFlow::Break(player) => ControlFlow::Break(format!(
                "{} batting for the {}.",
                player.load(database).name,
                self.teams
                    .select(self.inning.batting())
                    .id
                    .load(database)
                    .nickname,
            )),
        }
    }

    fn handle_game_over(&mut self, database: &mut Database) -> ControlFlow<String> {
        let winner = match (self.inning, self.teams.away.runs.cmp(&self.teams.home.runs)) {
            (Inning::Mid(n) | Inning::End(n), Ordering::Less) if n >= 9 => TeamSelect::Home,
            (Inning::End(n), Ordering::Greater) if n >= 9 => TeamSelect::Away,
            _ => return ControlFlow::Continue(()),
        };
        self.winner = Some(self.teams.select(winner).id);
        for team in self.teams.iter() {
            team.id.load_mut(database).rotation_slot += 1;
        }
        ControlFlow::Break(format!(
            "Game over. {} {}, {} {}",
            self.teams.away.id.load(database).nickname,
            self.teams.away.runs,
            self.teams.home.id.load(database).nickname,
            self.teams.home.runs
        ))
    }

    fn roll_fielder<'a>(&mut self, rng: &mut Rng, database: &'a Database) -> Fielder<'a> {
        Fielder(
            rng.choose(
                self.teams
                    .select(self.inning.fielding())
                    .id
                    .load(database)
                    .lineup
                    .clone(),
            )
            .expect("lineup was empty")
            .load(database),
        )
    }

    fn handle_steal(&mut self, rng: &mut Rng, database: &Database) -> ControlFlow<String> {
        let _fielder = self.roll_fielder(rng, database);
        let occupied = self.bases_occupied();
        let mut caught = None;
        for (idx, entry) in self.baserunners.iter_mut().enumerate() {
            let base = &mut entry.1;
            if !occupied.contains(&(*base + 1)) {
                let runner = entry.0.load(database);
                let attempt_roll = rng.next_f64();
                // TODO: steal attempt formula is not yet known
                if attempt_roll < 0.02 {
                    let next_base_display = crate::util::BaseDisplay {
                        base: *base + 1,
                        home: HOME_BASE,
                    };
                    let success_roll = rng.next_f64();
                    // TODO: steal success formula is not yet known
                    if success_roll < 0.5 {
                        *base += 1;
                        return ControlFlow::Break(format!(
                            "{} steals {}!",
                            runner.name, next_base_display
                        ));
                    }
                    caught = Some((
                        idx,
                        format!(
                            "{} gets caught stealing {}.",
                            runner.name, next_base_display
                        ),
                    ));
                    break;
                }
            }
        }
        if let Some((idx, message)) = caught {
            self.baserunners.remove(idx);
            self.handle_out();
            ControlFlow::Break(message)
        } else {
            ControlFlow::Continue(())
        }
    }

    fn handle_out(&mut self) {
        self.outs += 1;
        if self.outs >= OUTS_NEEDED {
            self.balls = 0;
            self.strikes = 0;
            self.outs = 0;
            self.at_bat = None;
            self.teams.select_mut(self.inning.batting()).lineup_slot += 1;
            self.baserunners = Vec::new();
            self.inning.advance();
        }
    }

    fn handle_ball(
        &mut self,
        batter: &Batter<'_>,
        database: &Database,
    ) -> ControlFlow<String, Never> {
        self.balls += 1;
        ControlFlow::Break(if self.balls >= BALLS_NEEDED {
            let mut message = format!("{} draws a walk.", batter.0.name);
            let occupied = self.bases_occupied();
            for (runner, mut base) in std::mem::take(&mut self.baserunners) {
                if (1..base).all(|b| occupied.contains(&b)) {
                    base += 1;
                }
                if base >= HOME_BASE {
                    self.teams.select_mut(self.inning.batting()).runs += 1;
                    write!(message, " {} scores!", runner.load(database).name)
                        .expect("std::fmt::Write does not fail on String");
                } else {
                    self.baserunners.push((runner, base));
                }
            }
            self.baserunners.push((batter.0.id, 1));
            self.at_bat = None;
            self.teams.select_mut(self.inning.batting()).lineup_slot += 1;
            format!("{} draws a walk.", batter.0.name)
        } else {
            format!("Ball. {}-{}", self.balls, self.strikes)
        })
    }

    fn handle_strike(
        &mut self,
        batter: &Batter<'_>,
        kind: &'static str,
    ) -> ControlFlow<String, Never> {
        self.strikes += 1;
        ControlFlow::Break(if self.strikes >= STRIKES_NEEDED {
            self.handle_out();
            self.at_bat = None;
            self.teams.select_mut(self.inning.batting()).lineup_slot += 1;
            format!("{} strikes out {}.", batter.0.name, kind)
        } else {
            format!("Strike, {}. {}-{}", kind, self.balls, self.strikes)
        })
    }

    fn handle_home_run(&mut self, batter: &Batter<'_>) -> ControlFlow<String, Never> {
        let mut runs = 0;
        for _ in self.baserunners.drain(..) {
            runs += 1;
        }
        self.teams.select_mut(self.inning.batting()).runs += runs;
        ControlFlow::Break(if runs == 1 {
            format!("{} hits a solo home run!", batter.0.name)
        } else {
            format!("{} hits a {}-run home run!", batter.0.name, runs)
        })
    }

    fn handle_base_hit(
        &mut self,
        batter: &Batter<'_>,
        database: &Database,
        bases: u8,
    ) -> ControlFlow<String, Never> {
        let mut message = match bases {
            1 => format!("{} hits a Single!", batter.0.name),
            2 => format!("{} hits a Double!", batter.0.name),
            3 => format!("{} hits a Triple!", batter.0.name),
            4 => format!("{} hits a Quadruple!", batter.0.name),
            _ => format!("{} hits a {}-base Hit!", batter.0.name, bases),
        };
        for (runner, mut base) in std::mem::take(&mut self.baserunners) {
            // TODO: extra base advancement
            base += bases;
            if base >= HOME_BASE {
                self.teams.select_mut(self.inning.batting()).runs += 1;
                write!(message, " {} scores!", runner.load(database).name)
                    .expect("std::fmt::Write does not fail on String");
            } else {
                self.baserunners.push((runner, base));
            }
        }
        ControlFlow::Break(message)
    }
}

fn roll_strike(rng: &mut Rng, date: Date, pitcher: &Pitcher<'_>, batter: &Batter<'_>) -> bool {
    let ballpark = Ballpark::default(); // TODO

    // NOTE: mostly using the season 14 formula
    let threshold = (0.2
        + (0.285 * (pitcher.0.ruthlessness * (1.0 + 0.2 * pitcher.0.vibes(date))))
        + (0.2 * ballpark.forwardness)
        + (0.1 * batter.0.musclitude))
        .min(0.86);
    rng.next_f64() < threshold
}

fn roll_swing(
    rng: &mut Rng,
    date: Date,
    pitcher: &Pitcher<'_>,
    batter: &Batter<'_>,
    strike: bool,
) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.0.vibes(date);

    let threshold = if strike {
        let div = batter.0.divinity * batter_vibes_mod;
        let musc = batter.0.musclitude * batter_vibes_mod;
        let thwack = batter.0.thwackability * batter_vibes_mod;
        let invpath = (1.0 - batter.0.patheticism) * batter_vibes_mod;
        let ruth = pitcher.0.ruthlessness * pitcher_vibes_mod;
        let combined = (div + musc + invpath + thwack) / 4.0;
        0.6 + (0.35 * combined) - (0.2 * ruth) + (0.2 * (ballpark.viscosity - 0.5))
    } else {
        let moxie = batter.0.moxie * batter_vibes_mod;
        let path = batter.0.patheticism;
        let ruth = pitcher.0.ruthlessness * pitcher_vibes_mod;
        let combined = (12.0 * ruth - 5.0 * moxie + 5.0 * path + 4.0 * ballpark.viscosity) / 20.0;
        if combined < 0.0 {
            f64::NAN
        } else {
            combined.powf(1.5).clamp(0.1, 0.95)
        }
    };
    rng.next_f64() < threshold
}

fn roll_contact(
    rng: &mut Rng,
    date: Date,
    pitcher: &Pitcher<'_>,
    batter: &Batter<'_>,
    strike: bool,
) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let fort = ballpark.fortification - 0.5;
    let visc = ballpark.viscosity - 0.5;
    let fwd = ballpark.forwardness - 0.5;
    let ballpark_sum = (fort + 3.0 * visc - 6.0 * fwd) / 10.0;

    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.0.vibes(date);

    // NOTE: mostly using the season 14 formula
    let threshold = if strike {
        let div = batter.0.divinity;
        let musc = batter.0.musclitude;
        let thwack = batter.0.thwackability;
        let path = batter.0.patheticism;
        let combined = (div + musc + thwack - path) / 2.0 * batter_vibes_mod;
        if combined < 0.0 {
            f64::NAN
        } else {
            let ruth = pitcher.0.ruthlessness * pitcher_vibes_mod;
            (0.78 - (0.08 * ruth) + (0.16 * ballpark_sum) + 0.17 * combined.powf(1.2)).min(0.9)
        }
    } else {
        let path = ((1.0 - batter.0.patheticism) * batter_vibes_mod).max(0.0);
        let ruth = pitcher.0.ruthlessness * pitcher_vibes_mod;
        (0.4 - (0.1 * ruth) + (0.35 * path.powf(1.5)) + (0.14 * ballpark_sum)).min(1.0)
    };
    rng.next_f64() < threshold
}

fn roll_foul(rng: &mut Rng, date: Date, batter: &Batter<'_>) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);

    let batter_sum =
        (batter.0.musclitude + batter.0.thwackability + batter.0.divinity) * batter_vibes_mod / 3.0;
    let threshold =
        0.25 + (0.1 * ballpark.forwardness) - (0.1 * ballpark.obtuseness) + (0.1 * batter_sum);
    rng.next_f64() < threshold
}

fn roll_out(
    rng: &mut Rng,
    date: Date,
    pitcher: &Pitcher<'_>,
    fielder: &Fielder<'_>,
    batter: &Batter<'_>,
) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);
    let fielder_vibes_mod = 1.0 + 0.2 * fielder.0.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.0.vibes(date);

    // rough formula for season 14 from
    // https://github.com/xSke/resim/blob/main/notebooks/find_roll_formula_out.ipynb
    let thwack = batter.0.thwackability * batter_vibes_mod;
    let unthwack = pitcher.0.unthwackability * pitcher_vibes_mod;
    let omni = fielder.0.omniscience * fielder_vibes_mod;
    let grand = ballpark.grandiosity - 0.5;
    let obt = ballpark.obtuseness - 0.5;
    let omin = ballpark.ominousness - 0.5;
    let incon = ballpark.inconvenience - 0.5;
    let visc = ballpark.viscosity - 0.5;
    let fwd = ballpark.forwardness - 0.5;

    let threshold = 0.3115 + (0.1 * thwack) - (0.08 * unthwack) - (0.065 * omni)
        + (0.01 * grand)
        + (0.0085 * obt)
        - (0.0033 * omin)
        - (0.0015 * incon)
        - (0.0033 * visc)
        + (0.01 * fwd);
    rng.next_f64() < threshold
}

fn roll_flyout(rng: &mut Rng, batter: &Batter<'_>) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let omin = ballpark.ominousness - 0.5;

    // https://github.com/xSke/resim/blob/main/notebooks/find_roll_formula_fly.ipynb
    let threshold = 0.18 + (0.3 * batter.0.buoyancy) - (0.16 * batter.0.suppression) - (0.1 * omin);
    rng.next_f64() < threshold
}

fn roll_home_run(rng: &mut Rng, date: Date, pitcher: &Pitcher<'_>, batter: &Batter<'_>) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.0.vibes(date);

    // https://github.com/xSke/resim/blob/main/notebooks/find_roll_formula_hr.ipynb
    let div = batter.0.divinity * batter_vibes_mod;
    let opw = pitcher.0.overpowerment * pitcher_vibes_mod;
    let supp = pitcher.0.suppression * pitcher_vibes_mod;
    let opw_supp = (10.0 * opw + supp) / 11.0;

    let grand = ballpark.grandiosity - 0.5;
    let fort = ballpark.fortification - 0.5;
    let visc = ballpark.viscosity - 0.5;
    let omin = ballpark.ominousness - 0.5;
    let fwd = ballpark.forwardness - 0.5;
    let ballpark_sum = (0.4 * grand) + (0.2 * fort) + (0.08 * visc) + (0.08 * omin) - (0.24 * fwd);

    let threshold = 0.12 + (0.16 * div) - 0.08 * (opw_supp) - (0.18 * ballpark_sum);
    rng.next_f64() < threshold
}

fn roll_base_hit(
    rng: &mut Rng,
    date: Date,
    pitcher: &Pitcher<'_>,
    fielder: &Fielder<'_>,
    batter: &Batter<'_>,
) -> u8 {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.0.vibes(date);
    let fielder_vibes_mod = 1.0 + 0.2 * fielder.0.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.0.vibes(date);

    let gf = batter.0.ground_friction * batter_vibes_mod;
    let musc = batter.0.musclitude * batter_vibes_mod;
    let opw = pitcher.0.overpowerment * pitcher_vibes_mod;
    let chase = fielder.0.chasiness * fielder_vibes_mod;
    let fwd = ballpark.forwardness - 0.5;
    let grand = ballpark.grandiosity - 0.5;
    let obt = ballpark.obtuseness - 0.5;
    let visc = ballpark.viscosity - 0.5;
    let omin = ballpark.ominousness - 0.5;
    let elong = ballpark.elongation - 0.5;

    // season 14
    // https://github.com/xSke/resim/blob/main/notebooks/find_roll_formula_triples_kidror.ipynb
    let triple_threshold = 0.05 + (0.2 * gf) - (0.04 * opw) - (0.06 * chase)
        + (0.02 * fwd)
        + (0.035 * grand)
        + (0.035 * obt)
        - (0.005 * omin)
        - (0.005 * visc);
    // https://github.com/xSke/resim/blob/main/notebooks/find_roll_formula_doubles.ipynb
    let double_threshold = 0.165 + (0.2 * musc) - (0.04 * opw) - (0.009 * chase) + (0.027 * fwd)
        - (0.015 * elong)
        - (0.01 * omin)
        - (0.008 * visc);

    let triple_roll = rng.next_f64();
    let double_roll = rng.next_f64();

    // TODO: Unsure which order these are checked in.
    if triple_roll < triple_threshold {
        3
    } else if double_roll < double_threshold {
        2
    } else {
        1
    }
}
