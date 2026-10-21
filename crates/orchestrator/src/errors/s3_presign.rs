use aws_credential_types::provider::error::CredentialsError;
use buildplane_shared::errors::retryability::Retryability;

#[derive(Debug, thiserror::Error)]
pub enum RsS3PresignError {
    #[error("failed to load AWS credentials: {0}")]
    Credentials(#[from] CredentialsError),
    #[error("invalid S3 endpoint URL: {source}")]
    InvalidEndpoint {
        #[source]
        source: url::ParseError,
    },
    #[error("S3 endpoint {endpoint:?} does not contain a host")]
    InvalidEndpointHost { endpoint: String },
    #[error("missing AWS credentials provider")]
    MissingCredentials,
    #[error(
        "object {key:?} is {size} bytes, exceeding the maximum object size of {max_bytes} bytes"
    )]
    ObjectTooLarge {
        key: String,
        size: usize,
        max_bytes: u64,
    },
    #[error("failed to upload object to S3: {0}")]
    Upload(#[source] reqwest::Error),
    #[error("S3 {operation} failed for {target}: {source}")]
    Operation {
        operation: &'static str,
        target: String,
        retryable: bool,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("S3 rejected upload for key {key:?} with status {status}: {body}")]
    UploadRejected {
        key: String,
        status: u16,
        body: String,
    },
}

impl RsS3PresignError {
    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Credentials(error) => Self::credentials_retryability(error),
            Self::InvalidEndpoint { .. }
            | Self::InvalidEndpointHost { .. }
            | Self::MissingCredentials { .. }
            | Self::ObjectTooLarge { .. } => Retryability::Permanent,
            Self::Upload(_) => Retryability::Retryable,
            Self::Operation { retryable, .. } => {
                if *retryable {
                    Retryability::Retryable
                } else {
                    Retryability::Permanent
                }
            }
            Self::UploadRejected { status, .. } => match status {
                408 | 429 => Retryability::Retryable,
                status if *status >= 500 => Retryability::Retryable,
                status if (400..500).contains(status) => Retryability::Permanent,
                _ => Retryability::Retryable,
            },
        }
    }

    fn credentials_retryability(error: &CredentialsError) -> Retryability {
        match error {
            CredentialsError::ProviderTimedOut(_) | CredentialsError::ProviderError(_) => {
                Retryability::Retryable
            }
            _ => Retryability::Permanent,
        }
    }
}
