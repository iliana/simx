use crate::id::{GameId, PlayerId, TeamId};
use crate::Date;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Game {
    pub id: GameId,
    #[serde(flatten)]
    pub date: Date,
    pub winner: Option<TeamId>,

    pub last_update: String,

    pub teams: AwayHome<GameTeam>,
    #[serde(flatten)]
    pub inning: Inning,
    pub at_bat: Option<PlayerId>,
    pub balls: u8,
    pub strikes: u8,
    pub outs: u8,
    pub baserunners: Vec<(PlayerId, u8)>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct GameTeam {
    pub id: TeamId,
    pub runs: u16,
    pub runs_by_inning: Vec<u16>,
    pub pitcher: Option<PlayerId>,
    pub lineup_slot: usize,
}

impl Game {
    pub fn new(date: Date, teams: AwayHome<TeamId>) -> Game {
        Game {
            id: GameId::new(),
            date,
            teams: teams.map(|id| GameTeam {
                id,
                ..GameTeam::default()
            }),
            ..Game::default()
        }
    }

    pub fn is_finished(&self) -> bool {
        self.winner.is_some()
    }

    pub fn bases_occupied(&self) -> BTreeSet<u8> {
        self.baserunners.iter().map(|(_, base)| *base).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case", tag = "frame", content = "inning")]
pub enum Inning {
    Top(usize),
    Mid(usize),
    Bottom(usize),
    End(usize),
}

impl Inning {
    pub fn advance(&mut self) {
        *self = match *self {
            Inning::Top(n) => Inning::Mid(n),
            Inning::Mid(n) => Inning::Bottom(n),
            Inning::Bottom(n) => Inning::End(n),
            Inning::End(n) => Inning::Top(n + 1),
        };
    }

    pub fn word(self) -> &'static str {
        match self {
            Inning::Top(_) => "Top",
            Inning::Mid(_) => "Mid",
            Inning::Bottom(_) => "Bottom",
            Inning::End(_) => "End",
        }
    }

    pub fn number(self) -> usize {
        match self {
            Inning::Top(n) | Inning::Mid(n) | Inning::Bottom(n) | Inning::End(n) => n,
        }
    }

    pub fn batting(self) -> TeamSelect {
        match self {
            Inning::Top(_) | Inning::Mid(_) => TeamSelect::Away,
            Inning::Bottom(_) | Inning::End(_) => TeamSelect::Home,
        }
    }

    pub fn fielding(self) -> TeamSelect {
        match self {
            Inning::Top(_) | Inning::Mid(_) => TeamSelect::Home,
            Inning::Bottom(_) | Inning::End(_) => TeamSelect::Away,
        }
    }
}

impl Default for Inning {
    fn default() -> Inning {
        Inning::Top(0)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AwayHome<T> {
    pub away: T,
    pub home: T,
}

impl<T> AwayHome<T> {
    pub fn map<U, F>(self, mut op: F) -> AwayHome<U>
    where
        F: FnMut(T) -> U,
    {
        AwayHome {
            away: op(self.away),
            home: op(self.home),
        }
    }

    pub fn select(&self, select: TeamSelect) -> &T {
        match select {
            TeamSelect::Away => &self.away,
            TeamSelect::Home => &self.home,
        }
    }

    pub fn select_mut(&mut self, select: TeamSelect) -> &mut T {
        match select {
            TeamSelect::Away => &mut self.away,
            TeamSelect::Home => &mut self.home,
        }
    }
}

impl<T> AwayHome<Option<T>> {
    pub fn transpose(self) -> Option<AwayHome<T>> {
        Some(AwayHome {
            away: self.away?,
            home: self.home?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TeamSelect {
    Away,
    Home,
}
