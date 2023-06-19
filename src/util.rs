use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Date {
    pub season: u16,
    pub day: u16,
}

pub(crate) struct BaseDisplay {
    pub(crate) base: u8,
    pub(crate) home: u8,
}

impl Display for BaseDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.base {
            n if n == self.home => write!(f, "home"),
            1 => write!(f, "first base"),
            2 => write!(f, "second base"),
            3 => write!(f, "third base"),
            4 => write!(f, "fourth base"),
            n => write!(
                f,
                "{}{} base",
                n,
                match n % 100 {
                    11 | 12 | 13 => "th",
                    _ => match n % 10 {
                        1 => "st",
                        2 => "nd",
                        3 => "rd",
                        _ => "th",
                    },
                }
            ),
        }
    }
}
