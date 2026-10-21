pub type TBResult<T> = Result<T, TBError>;

#[derive(Debug, thiserror::Error)]
pub enum TBError {
    #[error("Bad response from exchange. {0}")]
    BadResponse(String),
    #[error("Missing {0}")]
    Missing(String),
    #[error("Poisoned Lock")]
    PoisonedLock,
    #[error("System Time was before epoch")]
    SystemTimeBeforeEpoch,
    #[error("Unsupported {name}: {value}")]
    Unsupported { name: String, value: String },
    #[error("Invalid value for {name}: {value}")]
    InvalidValue { name: String, value: String },
}
