use crate::id::PlayerId;
use crate::{Database, Date, Game, Inning, Player, Rng, Sim};
use std::collections::BTreeSet;
use std::ops::ControlFlow;

// some consts, so we know what to fix when we start implementing modifications
const BALLS_NEEDED: u8 = 4;
const STRIKES_NEEDED: u8 = 3;
const OUTS_NEEDED: u8 = 3;
const HOME_BASE: u8 = 4;

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

impl Sim {
    pub fn tick(&mut self) {
        let rng = &mut self.rng;
        let database = &mut self.database;

        // To mutably borrow an individual game and the rest of the sim at the same time, we remove
        // the game from the sim, run `Game::tick`, and then add the game back to the sim.
        let game_ids = database.games_today.keys().copied().collect::<Vec<_>>();
        for id in game_ids {
            if let Some(mut game) = database.games_today.remove(&id) {
                game.tick(rng, database);
                database.games_today.insert(game.id, game);
            }
        }
    }
}

impl Game {
    fn tick(&mut self, rng: &mut Rng, database: &mut Database) {
        loop {
            match self.tick_inner(rng, database) {
                ControlFlow::Continue(()) => {}
                ControlFlow::Break(update) => {
                    self.last_update = update;
                    return;
                }
            }
        }
    }

    fn tick_inner(&mut self, rng: &mut Rng, database: &mut Database) -> ControlFlow<String> {
        if self.inning == Inning::default() {
            self.inning = Inning::Top(1);
            return ControlFlow::Break("Play ball!".into());
        }

        let pitcher = self.get_pitcher(rng, database);
        let batter = self.get_batter(rng, database)?;
        let pitcher = pitcher.load(database);
        let batter = batter.load(database);

        self.handle_steal(rng, database)?;
        let pitch = roll_pitch(rng, database.date, pitcher, batter);
        let swing = roll_swing(rng, database.date, pitcher, batter, pitch);
        if let Swing::Take = swing {
            return ControlFlow::Break(match pitch {
                Pitch::Ball => {
                    self.balls += 1;
                    if self.balls >= BALLS_NEEDED {
                        // FIXME: base advancement
                        self.baserunners.push((batter.id, 1));
                        self.at_bat = None;
                        format!("{} draws a walk.", batter.name)
                    } else {
                        format!("Ball. {}-{}", self.balls, self.strikes)
                    }
                }
                Pitch::Strike => {
                    self.strikes += 1;
                    if self.strikes >= STRIKES_NEEDED {
                        self.outs += 1;
                        self.at_bat = None;
                        format!("{} strikes out looking.", batter.name)
                    } else {
                        format!("Strike, looking. {}-{}", self.balls, self.strikes)
                    }
                }
            });
        }

        ControlFlow::Continue(())
    }

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
        let occupied = self
            .baserunners
            .iter()
            .map(|(_, base)| base)
            .copied()
            .collect::<BTreeSet<_>>();
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
            ControlFlow::Break(message)
        } else {
            ControlFlow::Continue(())
        }
    }

    fn is_decided(&self) -> bool {
        match self.inning {
            Inning::Top(n) if n > 9 => self.teams.away.runs > self.teams.home.runs,
            Inning::Bottom(n) if n > 8 => self.teams.away.runs < self.teams.home.runs,
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
enum Pitch {
    Ball,
    Strike,
}

fn roll_pitch(rng: &mut Rng, date: Date, pitcher: &Player, batter: &Player) -> Pitch {
    let forwardness = 0.5; // TODO: ballparks

    // NOTE: mostly using the season 14 formula
    let threshold = (0.2
        + (0.285 * (pitcher.ruthlessness * (1.0 + 0.2 * pitcher.vibes(date))))
        + (0.2 * forwardness)
        + (0.1 * batter.musclitude))
        .min(0.86);
    if rng.next_f64() < threshold {
        Pitch::Strike
    } else {
        Pitch::Ball
    }
}

#[derive(Clone, Copy)]
enum Swing {
    Swing,
    Take,
}

fn roll_swing(rng: &mut Rng, date: Date, pitcher: &Player, batter: &Player, pitch: Pitch) -> Swing {
    let viscosity = 0.5; // TODO: ballparks
    let batter_vibes_mod = 1.0 + 0.2 * batter.vibes(date);
    let pitcher_vibes_mod = 1.0 + 0.2 * pitcher.vibes(date);

    let threshold = match pitch {
        Pitch::Ball => {
            let moxie = batter.moxie * batter_vibes_mod;
            let path = batter.patheticism;
            let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
            let combined = (12.0 * ruth - 5.0 * moxie + 5.0 * path + 4.0 * viscosity) / 20.0;
            if combined < 0.0 {
                f64::NAN
            } else {
                combined.powf(1.5).clamp(0.1, 0.95)
            }
        }
        Pitch::Strike => {
            let div = batter.divinity * batter_vibes_mod;
            let musc = batter.musclitude * batter_vibes_mod;
            let thwack = batter.thwackability * batter_vibes_mod;
            let invpath = (1.0 - batter.patheticism) * batter_vibes_mod;
            let ruth = pitcher.ruthlessness * pitcher_vibes_mod;
            let combined = (div + musc + invpath + thwack) / 4.0;
            0.6 + (0.35 * combined) - (0.2 * ruth) + (0.2 * (viscosity - 0.5))
        }
    };
    if rng.next_f64() < threshold {
        Swing::Swing
    } else {
        Swing::Take
    }
}
