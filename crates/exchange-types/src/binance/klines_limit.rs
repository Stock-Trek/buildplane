use crate::error::{ETError, ETResult};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct BinanceKlinesLimit(u32);

impl BinanceKlinesLimit {
    pub const MINIMUM: u32 = 1;
    pub const MAXIMUM: u32 = 1000;

    pub fn try_new(klines_limit: u32) -> ETResult<Self> {
        if Self::is_valid(klines_limit) {
            Ok(Self(klines_limit))
        } else {
            Err(ETError::BadValue {
                name: "klines_limit".into(),
                value: klines_limit.to_string(),
                reason: format!("Out of bounds >={} <={}", Self::MINIMUM, Self::MAXIMUM),
            })
        }
    }

    pub fn is_valid(klines_limit: u32) -> bool {
        (Self::MINIMUM..=Self::MAXIMUM).contains(&klines_limit)
    }
}

impl fmt::Display for BinanceKlinesLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
