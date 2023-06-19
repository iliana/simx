use crate::id::{PlayerId, TeamId};
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

    pub(crate) games_today: Vec<Game>,
}

impl Database {
    // A database stored as above is compatible with Rust's memory model, but can easily become
    // inconsistent due to bugs. Instead of reaching for SQL (which makes iliana's brain turn to
    // mush) in the core sim, we will define some invariants about what must be correct in the
    // database. This function checks those invariants during:
    //
    // 1. The `Deserialize` implementation of `Database`
    // 2. When debug assertions are enabled, at the end of each `Sim::tick`
    pub(crate) fn check_consistency(&self) -> Result<(), String> {
        let mut problems = Vec::new();

        macro_rules! nil_check {
            ($iter:expr) => {
                for obj in $iter {
                    if obj.id.0.is_nil() {
                        problems.push(format!("- nil uuid: {:?}", obj))
                    }
                }
            };
        }
        nil_check!(self.teams.values());
        nil_check!(self.players.values());
        nil_check!(&self.games_today);

        macro_rules! key_check {
            ($iter:expr, $kind:expr) => {
                for (key, obj) in $iter {
                    if obj.id != *key {
                        problems.push(format!("- {} {} is keyed with {}", $kind, obj.id, key));
                    }
                }
            };
        }
        key_check!(&self.teams, "team");
        key_check!(&self.players, "player");

        // TODO: check more things

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }
}
