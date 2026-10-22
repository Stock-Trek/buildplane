use crate::{errors::retryability::Retryability, protocol::Protocol};
use std::error::Error as StdError;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ExchangeError {
    #[error("Bad response from exchange. {0}")]
    BadResponse(String),
    #[error("Exchange request failed while trying to {operation}: {source}")]
    Exchange {
        operation: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("Unsupported exchange: {exchange}")]
    UnsupportedExchange { exchange: String },
    #[error("Unsupported trading mode: {trading_mode}")]
    UnsupportedTradingMode { trading_mode: String },
    #[error("Cannot find a symbol for trading pair {trading_pair}")]
    MissingSymbol { trading_pair: String },
    #[error("Unknown {exchange} submission outcome while fetching klines")]
    UnknownSubmissionOutcome { exchange: &'static str },
    #[error("Cannot send request {request_type} on protocol {protocol}")]
    BadProtocol {
        request_type: String,
        protocol: Protocol,
    },
    #[error("Cannot use {name}={value}. {reason}")]
    BadValue {
        name: String,
        value: String,
        reason: String,
    },
    #[error("Crypto key error: {0}")]
    CryptoKey(String),
    #[error("Deserialize response error: {0}")]
    DeserializeResponse(serde_json::Error),
    #[error("Parsing error: {0}")]
    ParseError(String),
    #[error("Serialize request error: {0}")]
    SerializeRequest(serde_json::Error),
    #[error("Value for {0} deserialized as Unknown")]
    UnknownValue(String),
}

impl ExchangeError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Exchange { .. } => Retryability::Retryable,
            _ => Retryability::Permanent,
        }
    }
}
