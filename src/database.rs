use crate::id::{PlayerId, TeamId};
use crate::{Date, Game, Player, Team};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Database {
    #[serde(flatten)]
    pub(crate) date: Date,

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
    // 2. When debug assertions are enabled, any mutable methods on `Sim`
    pub(crate) fn check_consistency(&self) -> Result<(), String> {
        let mut problems = Vec::new();

        macro_rules! key_check {
            ($iter:expr, $kind:expr) => {
                for (key, obj) in $iter {
                    if obj.id != *key {
                        problems.push(format!("- {} {}: keyed with {}", $kind, obj.id, key));
                    }
                }
            };
        }
        key_check!(&self.teams, "team");
        key_check!(&self.players, "player");

        macro_rules! check_method {
            ($iter:expr, $kind:expr) => {
                for obj in $iter {
                    for error in CheckEntity::problems(obj, self) {
                        problems.push(format!("- {} {}: {}", $kind, obj.id, error));
                    }
                }
            };
        }
        check_method!(self.teams.values(), "team");
        check_method!(self.players.values(), "player");
        check_method!(&self.games_today, "game");

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }

    pub(crate) fn debug_check(&self) {
        debug_assert_eq!(self.check_consistency(), Ok(()));
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("object ID is nil")]
    NilId,
    #[error("reference to nonexistent {kind} {id}")]
    BadReference { kind: &'static str, id: Uuid },
    #[error("player {player} is on the roster multiple times")]
    DuplicatePlayer { player: PlayerId },
}

pub(crate) trait CheckEntity {
    fn problems(&self, database: &Database) -> Vec<DatabaseError>;

    fn check(&self, database: &Database) -> Result<(), DatabaseError> {
        if let Some(problem) = self.problems(database).into_iter().next() {
            Err(problem)
        } else {
            Ok(())
        }
    }
}
