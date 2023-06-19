use crate::id::{PlayerId, TeamId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Team {
    pub id: TeamId,
    pub location: String,
    pub nickname: String,
    pub lineup: Vec<PlayerId>,
    pub rotation: Vec<PlayerId>,
    pub shadows: Vec<PlayerId>,
    pub rotation_slot: usize,
}

impl Team {
    pub fn name(&self) -> String {
        format!("{} {}", self.location, self.nickname)
    }
}
