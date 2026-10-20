use crate::urls::Protocol;

pub type ETResult<T> = Result<T, ETError>;

#[derive(Debug, thiserror::Error)]
pub enum ETError {
    #[error("Cannot send request {request_type} on protocol {protocol}")]
    BadProtocol {
        request_type: String,
        protocol: Protocol,
    },
    #[error("Bad response: {0}")]
    BadResponse(String),
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
