use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct BinanceError {
    pub code: i64,
    pub msg: String,
    pub data: Option<BinanceErrorData>,
}

#[allow(non_snake_case)]
#[derive(Deserialize, Debug, Clone)]
pub struct BinanceErrorData {
    pub serverTime: Option<i64>,
    pub retryAfter: Option<i64>,
}

impl std::fmt::Display for BinanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.msg)
    }
}

impl std::error::Error for BinanceError {}
