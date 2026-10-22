use crate::errors::retryability::Retryability;
use std::time::Duration;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum TimeoutError {
    #[error("build timed out after {0:?}")]
    BuildTimeout(Duration),
    #[error("{operation} timed out")]
    Operation { operation: &'static str },
    #[error("deadline reached before operation could start")]
    DeadlineExceeded,
    #[error("Request timed out waiting for a response")]
    TimedOut,
}

impl TimeoutError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        Retryability::Retryable
    }
}
