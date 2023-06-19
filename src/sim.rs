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
        debug_assert_eq!(database.check_consistency(), Ok(()));
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
        let pitcher = pitcher.load(database);
        let batter = batter.load(database);

        self.handle_steal(rng, database)?;
        let strike = roll_strike(rng, database.date, pitcher, batter);
        if !roll_swing(rng, database.date, pitcher, batter, strike) {
            return if strike {
                self.handle_strike(batter, "looking")
            } else {
                self.handle_ball(batter, database)
            };
        }
        if !roll_contact(rng, database.date, pitcher, batter, strike) {
            return self.handle_strike(batter, "swinging");
        }
        if roll_foul(rng, database.date, batter) {
            self.strikes = 2.min(self.strikes + 1);
            return ControlFlow::Break(format!("Foul Ball. {}-{}", self.balls, self.strikes));
        }

        todo!();
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
                let player = Player::generate_with_name($rng, $db, $new_name.to_string());
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
                self.teams
                    .select(self.inning.batting())
                    .id
                    .load(database)
                    .nickname,
                player.load(database).name
            )),
        }
    }

    fn handle_game_over(&mut self, database: &Database) -> ControlFlow<String> {
        let winner = match (self.inning, self.teams.away.runs.cmp(&self.teams.home.runs)) {
            (Inning::Mid(n) | Inning::End(n), Ordering::Less) if n >= 9 => TeamSelect::Home,
            (Inning::End(n), Ordering::Greater) if n >= 9 => TeamSelect::Away,
            _ => return ControlFlow::Continue(()),
        };
        self.winner = Some(self.teams.select(winner).id);
        ControlFlow::Break(format!(
            "Game over. {} {}, {} {}",
            self.teams.away.id.load(database).nickname,
            self.teams.away.runs,
            self.teams.home.id.load(database).nickname,
            self.teams.home.runs
        ))
    }

    fn roll_fielder<'a>(&mut self, rng: &mut Rng, database: &'a Database) -> &'a Player {
        rng.choose(
            self.teams
                .select(self.inning.fielding())
                .id
                .load(database)
                .lineup
                .clone(),
        )
        .expect("lineup was empty")
        .load(database)
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
            self.outs = 0;
            self.at_bat = None;
            self.baserunners = Vec::new();
            self.inning.advance();
        }
    }

    fn handle_ball(&mut self, batter: &Player, database: &Database) -> ControlFlow<String, Never> {
        self.balls += 1;
        ControlFlow::Break(if self.balls >= BALLS_NEEDED {
            let mut message = format!("{} draws a walk.", batter.name);
            let occupied = self.bases_occupied();
            for (runner, mut base) in std::mem::take(&mut self.baserunners) {
                if (1..base).all(|b| occupied.contains(&b)) {
                    base += 1;
                }
                if base >= HOME_BASE {
                    write!(message, " {} scores!", runner.load(database).name)
                        .expect("std::fmt::Write does not fail on String");
                } else {
                    self.baserunners.push((runner, base));
                }
            }
            self.baserunners.push((batter.id, 1));
            self.at_bat = None;
            format!("{} draws a walk.", batter.name)
        } else {
            format!("Ball. {}-{}", self.balls, self.strikes)
        })
    }

    fn handle_strike(&mut self, batter: &Player, kind: &'static str) -> ControlFlow<String, Never> {
        self.strikes += 1;
        ControlFlow::Break(if self.strikes >= STRIKES_NEEDED {
            self.handle_out();
            self.at_bat = None;
            format!("{} strikes out {}.", batter.name, kind)
        } else {
            format!("Strike, {}. {}-{}", kind, self.balls, self.strikes)
        })
    }
}

fn roll_strike(rng: &mut Rng, date: Date, pitcher: &Player, batter: &Player) -> bool {
    let ballpark = Ballpark::default(); // TODO

    // NOTE: mostly using the season 14 formula
    let threshold = (0.2
        + (0.285 * (pitcher.ruthlessness * (1.0 + 0.2 * pitcher.vibes(date))))
        + (0.2 * ballpark.forwardness)
        + (0.1 * batter.musclitude))
        .min(0.86);
    rng.next_f64() < threshold
}

fn roll_swing(rng: &mut Rng, date: Date, pitcher: &Player, batter: &Player, strike: bool) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.vibes(date);

    let threshold = if strike {
        let div = batter.divinity * batter_vibes_mod;
        let musc = batter.musclitude * batter_vibes_mod;
        let thwack = batter.thwackability * batter_vibes_mod;
        let invpath = (1.0 - batter.patheticism) * batter_vibes_mod;
        let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
        let combined = (div + musc + invpath + thwack) / 4.0;
        0.6 + (0.35 * combined) - (0.2 * ruth) + (0.2 * (ballpark.viscosity - 0.5))
    } else {
        let moxie = batter.moxie * batter_vibes_mod;
        let path = batter.patheticism;
        let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
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
    pitcher: &Player,
    batter: &Player,
    strike: bool,
) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let fort = ballpark.fortification - 0.5;
    let visc = ballpark.viscosity - 0.5;
    let fwd = ballpark.forwardness - 0.5;
    let ballpark_sum = (fort + 3.0 * visc - 6.0 * fwd) / 10.0;

    let batter_vibes_mod = 1.0 + 0.2 * batter.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.vibes(date);

    // NOTE: mostly using the season 14 formula
    let threshold = if strike {
        let div = batter.divinity;
        let musc = batter.musclitude;
        let thwack = batter.thwackability;
        let path = batter.patheticism;
        let combined = (div + musc + thwack - path) / 2.0 * batter_vibes_mod;
        if combined < 0.0 {
            f64::NAN
        } else {
            let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
            (0.78 - (0.08 * ruth) + (0.16 * ballpark_sum) + 0.17 * combined.powf(1.2)).min(0.9)
        }
    } else {
        let path = ((1.0 - batter.patheticism) * batter_vibes_mod).max(0.0);
        let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
        (0.4 - (0.1 * ruth) + (0.35 * path.powf(1.5)) + (0.14 * ballpark_sum)).min(1.0)
    };
    rng.next_f64() < threshold
}

fn roll_foul(rng: &mut Rng, date: Date, batter: &Player) -> bool {
    let ballpark = Ballpark::default(); // TODO
    let batter_vibes_mod = 1.0 + 0.2 * batter.vibes(date);

    let batter_sum =
        (batter.musclitude + batter.thwackability + batter.divinity) * batter_vibes_mod / 3.0;
    let threshold =
        0.25 + (0.1 * ballpark.forwardness) - (0.1 * ballpark.obtuseness) + (0.1 * batter_sum);
    rng.next_f64() < threshold
}
