use crate::errors::retryability::Retryability;
use std::{error::Error as StdError, path::PathBuf, process::ExitStatus};

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum IoError {
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("I/O error while {operation} at {path}: {source}")]
    Io {
        operation: &'static str,
        path: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{operation} failed for {path} with {status}")]
    CommandFailed {
        operation: &'static str,
        path: PathBuf,
        status: ExitStatus,
    },
}

impl IoError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Arrow(_) | Self::Parquet(_) | Self::Io { .. } | Self::IoError(_) => {
                Retryability::Retryable
            }
            Self::Zip(_) | Self::CommandFailed { .. } => Retryability::Permanent,
        }
    }
}
