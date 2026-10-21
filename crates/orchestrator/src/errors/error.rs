use crate::errors::{
    archive::RsArchiveError, invoke::RsLambdaInvokeError, s3_presign::RsS3PresignError,
};
use aws_sdk_lambda::operation::invoke::InvokeError;
use buildplane_shared::errors::{error::RsSharedError, retryability::Retryability};
use lambda_runtime::Diagnostic;

pub type RsOrchestratorResult<T> = Result<T, RsOrchestratorError>;

#[derive(Debug, thiserror::Error)]
pub enum RsOrchestratorError {
    #[error("Archive: {0}")]
    Archive(#[from] RsArchiveError),
    #[error("Run error: {0}")]
    Run(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("Lambda invoke error: {0}")]
    Lambda(#[from] Box<InvokeError>),
    #[error("Lambda invocation failed: {0}")]
    LambdaInvoke(#[from] RsLambdaInvokeError),
    #[error("S3Presign: {0}")]
    S3Presign(#[from] RsS3PresignError),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Shared error: {0}")]
    Shared(#[from] RsSharedError),
}

impl RsOrchestratorError {
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::Archive(_) => "archive",
            Self::Run(_) => "run",
            Self::Lambda(_) => "lambda",
            Self::LambdaInvoke(_) => "lambda_invoke",
            Self::S3Presign(_) => "s3_presign",
            Self::Serde(_) => "serde",
            Self::Shared(_) => "shared",
        }
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Run(_) => Retryability::Retryable,
            Self::Lambda(error) => Self::lambda_retryability(error),
            Self::LambdaInvoke(error) => error.retryability(),
            Self::S3Presign(error) => error.retryability(),
            Self::Serde(_) | Self::Archive(_) => Retryability::Permanent,
            Self::Shared(error) => error.retryability(),
        }
    }

    fn lambda_retryability(error: &InvokeError) -> Retryability {
        crate::errors::invoke::invoke_retryability(error)
    }
}

impl From<RsOrchestratorError> for Diagnostic {
    fn from(value: RsOrchestratorError) -> Self {
        Diagnostic {
            error_type: value.error_type().into(),
            error_message: value.to_string(),
        }
    }
}
