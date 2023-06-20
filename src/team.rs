use crate::database::{CheckEntity, Database};
use crate::id::{PlayerId, TeamId};
use crate::DatabaseError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Team {
    pub id: TeamId,
    pub location: String,
    pub nickname: String,
    pub lineup: Vec<PlayerId>,
    pub rotation: Vec<PlayerId>,
    pub shadows: Vec<PlayerId>,
    #[serde(alias = "rotationSlot")]
    pub rotation_slot: usize,
}

impl Team {
    pub fn name(&self) -> String {
        format!("{} {}", self.location, self.nickname)
    }
}

impl CheckEntity for Team {
    fn problems(&self, database: &Database) -> Vec<DatabaseError> {
        let mut problems = Vec::new();
        if self.id.0.is_nil() {
            problems.push(DatabaseError::NilId);
        }
        let mut roster: HashMap<PlayerId, usize> = HashMap::new();
        for player in self
            .lineup
            .iter()
            .chain(&self.rotation)
            .chain(&self.shadows)
        {
            *roster.entry(*player).or_default() += 1;
        }
        for (player, count) in roster {
            if !database.players.contains_key(&player) {
                problems.push(DatabaseError::BadReference {
                    kind: "player",
                    id: player.0,
                });
            }
            if count > 1 {
                problems.push(DatabaseError::DuplicatePlayer { player });
            }
        }
        problems
    }
}
