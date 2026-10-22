use crate::errors::retryability::Retryability;
use std::error::Error as StdError;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum SystemError {
    #[error(transparent)]
    External(#[from] Box<dyn StdError + Send + Sync + 'static>),
    #[error("Poisoned Lock")]
    PoisonedLock,
    #[error("Concurrency semaphore was closed while waiting for a permit")]
    SemaphoreClosed,
    #[error("System Time was before epoch")]
    SystemTimeBeforeEpoch,
    #[error("Background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    #[error("failed to install tracing subscriber")]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),
}

impl SystemError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Task(_) | Self::External(_) => Retryability::Retryable,
            Self::Tracing(_)
            | Self::PoisonedLock
            | Self::SystemTimeBeforeEpoch
            | Self::SemaphoreClosed => Retryability::Permanent,
        }
    }
}
