use crate::id::{GameId, PlayerId, TeamId};
use crate::{Date, Game, Player, Team};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Database {
    #[serde(flatten)]
    pub(crate) date: Date,

    pub(crate) first_names: Vec<String>,
    pub(crate) last_names: Vec<String>,
    pub(crate) rituals: Vec<String>,

    pub(crate) teams: BTreeMap<TeamId, Team>,
    pub(crate) players: BTreeMap<PlayerId, Player>,

    pub(crate) games_today: BTreeMap<GameId, Game>,
}

impl Database {
    pub(crate) fn check_consistency(&self) -> Result<(), String> {
        // TODO: this
        Ok(())
    }
}
