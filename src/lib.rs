#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::uninlined_format_args
)]

mod database;
mod game;
pub mod id;
mod player;
mod rng;
mod sim;
mod team;
mod util;

pub use crate::database::Database;
pub use crate::game::{AwayHome, Game, GameTeam, Inning, TeamSelect};
pub use crate::player::Player;
pub use crate::rng::Rng;
pub use crate::team::Team;
pub use crate::util::Date;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Sim {
    rng: Rng,
    #[serde(flatten, deserialize_with = "deserialize_database")]
    database: Database,
}

impl Sim {
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }

    pub fn database(&self) -> &Database {
        &self.database
    }

    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
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
