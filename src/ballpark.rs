use crate::id::{BallparkId, TeamId};

#[derive(Debug)]
#[non_exhaustive]
pub struct Ballpark {
    pub id: BallparkId,
    pub team_id: TeamId,
    pub name: String,
    pub nickname: String,
    pub ominousness: f64,
    pub forwardness: f64,
    pub obtuseness: f64,
    pub grandiosity: f64,
    pub fortification: f64,
    pub elongation: f64,
    pub inconvenience: f64,
    pub viscosity: f64,
    pub hype: f64,
    pub mysticism: f64,
    pub luxuriousness: f64,
    pub filthiness: f64,
    pub birds: i32,
}

impl Default for Ballpark {
    fn default() -> Ballpark {
        Ballpark {
            id: BallparkId::default(),
            team_id: TeamId::default(),
            name: String::default(),
            nickname: String::default(),
            ominousness: 0.5,
            forwardness: 0.5,
            obtuseness: 0.5,
            grandiosity: 0.5,
            fortification: 0.5,
            elongation: 0.5,
            inconvenience: 0.5,
            viscosity: 0.5,
            hype: 0.0,
            mysticism: 0.5,
            luxuriousness: 0.0,
            filthiness: 0.0,
            birds: 0,
        }
    }
}
