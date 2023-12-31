use crate::database::CheckEntity;
use crate::id::PlayerId;
use crate::{Database, DatabaseError, Date, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Player {
    pub id: PlayerId,
    pub name: String,

    pub thwackability: f64,
    pub moxie: f64,
    pub divinity: f64,
    pub musclitude: f64,
    pub patheticism: f64,
    pub buoyancy: f64,
    #[serde(alias = "baseThirst")]
    pub base_thirst: f64,
    pub laserlikeness: f64,
    #[serde(alias = "groundFriction")]
    pub ground_friction: f64,
    pub continuation: f64,
    pub indulgence: f64,
    pub martyrdom: f64,
    pub tragicness: f64,
    pub shakespearianism: f64,
    pub suppression: f64,
    pub unthwackability: f64,
    pub coldness: f64,
    pub overpowerment: f64,
    pub ruthlessness: f64,
    pub omniscience: f64,
    pub tenaciousness: f64,
    pub watchfulness: f64,
    pub anticapitalism: f64,
    pub chasiness: f64,
    pub pressurization: f64,
    pub cinnamon: f64,

    pub soul: u16,
    #[serde(alias = "peanutAllergy")]
    pub peanut_allergy: bool,
    pub fate: u8,
    pub blood: u8,
    pub coffee: u8,
}

impl Player {
    // TODO: figure out the interface around name/ritual pools and make `pub`
    pub(crate) fn generate_with_name(rng: &mut Rng, name: String) -> Player {
        Player {
            id: PlayerId::new(),
            name,
            thwackability: rng.next_f64(),
            moxie: rng.next_f64(),
            divinity: rng.next_f64(),
            musclitude: rng.next_f64(),
            patheticism: rng.next_f64(),
            buoyancy: rng.next_f64(),
            base_thirst: rng.next_f64(),
            laserlikeness: rng.next_f64(),
            ground_friction: rng.next_f64(),
            continuation: rng.next_f64(),
            indulgence: rng.next_f64(),
            martyrdom: rng.next_f64(),
            tragicness: rng.next_f64(),
            shakespearianism: rng.next_f64(),
            suppression: rng.next_f64(),
            unthwackability: rng.next_f64(),
            coldness: rng.next_f64(),
            overpowerment: rng.next_f64(),
            ruthlessness: rng.next_f64(),
            omniscience: rng.next_f64(),
            tenaciousness: rng.next_f64(),
            watchfulness: rng.next_f64(),
            anticapitalism: rng.next_f64(),
            chasiness: rng.next_f64(),
            pressurization: rng.next_f64(),
            cinnamon: rng.next_f64(),
            soul: rng.choose(2..10).unwrap_or_default(),
            peanut_allergy: rng.choose([true, false]).unwrap_or_default(),
            // ritual gets rolled after fate but we don't pick that yet, so throw away a roll.
            fate: (rng.choose(0..100).unwrap_or_default(), rng.next_f64()).0,
            blood: rng.choose(0..13).unwrap_or_default(),
            coffee: rng.choose(0..13).unwrap_or_default(),
        }
    }

    pub fn vibes(&self, date: Date) -> f64 {
        let frequency = 6.0 + (10.0 * self.buoyancy).round();
        (std::f64::consts::PI * ((2.0 / frequency) * f64::from(date.day) + 0.5)).sin()
    }
}

impl CheckEntity for Player {
    fn problems(&self, _database: &Database) -> Vec<DatabaseError> {
        let mut problems = Vec::new();
        if self.id.0.is_nil() {
            problems.push(DatabaseError::NilId);
        }
        problems
    }
}
