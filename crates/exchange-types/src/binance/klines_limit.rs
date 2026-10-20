use crate::error::{ETError, ETResult};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct BinanceKlinesLimit(u32);

impl BinanceKlinesLimit {
    pub const MAXIMUM: u32 = 1000;

    pub fn try_new(klines_limit: u32) -> ETResult<Self> {
        if Self::is_valid(klines_limit) {
            Ok(Self(klines_limit))
        } else {
            Err(ETError::BadValue {
                name: "recv_window".into(),
                value: klines_limit.to_string(),
                reason: format!("Out of bounds <={}", Self::MAXIMUM),
            })
        }
    }

    pub fn is_valid(klines_limit: u32) -> bool {
        klines_limit <= Self::MAXIMUM
    }
}

impl fmt::Display for BinanceKlinesLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
