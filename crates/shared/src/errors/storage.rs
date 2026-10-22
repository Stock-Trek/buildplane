use crate::errors::retryability::Retryability;
use aws_credential_types::provider::error::CredentialsError;
use std::error::Error as StdError;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum StorageError {
    #[error("S3 {operation} failed for {target}: {source}")]
    S3 {
        operation: &'static str,
        target: String,
        retryable: bool,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("S3 delete objects failed for {target}: {failures}")]
    S3DeleteObjects { target: String, failures: String },
    #[error("failed to load AWS credentials: {0}")]
    Credentials(#[from] CredentialsError),
    #[error("invalid S3 endpoint URL: {source}")]
    InvalidEndpoint {
        #[source]
        source: url::ParseError,
    },
    #[error("S3 endpoint {endpoint:?} does not contain a host")]
    InvalidEndpointHost { endpoint: String },
    #[error("missing credentials")]
    MissingCredentials,
    #[error(
        "object {key:?} is {size} bytes, exceeding the maximum object size of {max_bytes} bytes"
    )]
    ObjectTooLarge {
        key: String,
        size: usize,
        max_bytes: u64,
    },
}

impl StorageError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::S3 {
                retryable: true, ..
            }
            | Self::S3DeleteObjects { .. } => Retryability::Retryable,
            Self::S3 {
                retryable: false, ..
            }
            | Self::InvalidEndpoint { .. }
            | Self::InvalidEndpointHost { .. }
            | Self::MissingCredentials
            | Self::ObjectTooLarge { .. } => Retryability::Permanent,
            Self::Credentials(error) => Retryability::from_credentials(error),
        }
    }
}
