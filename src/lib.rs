#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::uninlined_format_args
)]

mod ballpark;
mod database;
mod game;
pub mod id;
mod player;
mod rng;
mod sim;
mod team;
mod util;

use crate::database::{CheckEntity, Database};
use crate::id::{PlayerId, TeamId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::ballpark::Ballpark;
pub use crate::database::DatabaseError;
pub use crate::game::{AwayHome, Game, GameTeam, Inning, TeamSelect};
pub use crate::player::Player;
pub use crate::rng::Rng;
pub use crate::team::Team;
pub use crate::util::Date;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Sim {
    rng: Rng,
    #[serde(flatten, deserialize_with = "deserialize_database")]
    database: Database,
}

impl Sim {
    pub fn new() -> Sim {
        Sim::default()
    }

    pub fn players(&self) -> &BTreeMap<PlayerId, Player> {
        &self.database.players
    }

    pub fn teams(&self) -> &BTreeMap<TeamId, Team> {
        &self.database.teams
    }

    pub fn games_today(&self) -> &[Game] {
        &self.database.games_today
    }

    /// Add a player to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the player's ID is nil (an all-zero UUID).
    pub fn add_player(&mut self, player: Player) -> Result<(), DatabaseError> {
        player.check(&self.database)?;
        self.database.players.insert(player.id, player);
        self.database.debug_check();
        Ok(())
    }

    /// Add a team to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - the team's ID is nil (an all-zero UUID)
    /// - the team references player IDs that are not in the database
    pub fn add_team(&mut self, team: Team) -> Result<(), DatabaseError> {
        team.check(&self.database)?;
        self.database.teams.insert(team.id, team);
        self.database.debug_check();
        Ok(())
    }

    /// Start a new day of games.
    ///
    /// Returns the previous day of games.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - any game's ID is nil (an all-zero UUID)
    /// - any game references team or player IDs that are not in the database
    pub fn start_day(
        &mut self,
        date: Date,
        games: Vec<Game>,
    ) -> Result<(Date, Vec<Game>), DatabaseError> {
        for game in &games {
            game.check(&self.database)?;
        }
        let old_date = std::mem::replace(&mut self.database.date, date);
        let old_games = std::mem::replace(&mut self.database.games_today, games);
        Ok((old_date, old_games))
    }
}

fn deserialize_database<'de, D>(deserializer: D) -> Result<Database, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let database = Database::deserialize(deserializer)?;
    database
        .check_consistency()
        .map_err(serde::de::Error::custom)?;
    Ok(database)
}
