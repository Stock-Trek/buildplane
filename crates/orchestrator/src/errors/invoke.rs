use aws_sdk_lambda::{
    error::ProvideErrorMetadata, operation::invoke::InvokeError as LambdaInvokeError,
};
use buildplane_shared::errors::retryability::Retryability;

#[derive(Debug, thiserror::Error)]
pub enum RsLambdaInvokeError {
    #[error("Lambda function returned {function_error}: {detail}", detail = .detail.as_deref().unwrap_or("no detail"))]
    Function {
        function_error: String,
        detail: Option<String>,
    },
    #[error("Lambda invoke returned unexpected status code {status_code}")]
    Status { status_code: i32 },
}

impl RsLambdaInvokeError {
    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Function { .. } | Self::Status { .. } => Retryability::Permanent,
        }
    }
}

pub fn invoke_retryability(error: &LambdaInvokeError) -> Retryability {
    match error.code() {
        Some(
            "InvalidParameterValueException"
            | "InvalidRequestContentException"
            | "RecursiveInvocationException"
            | "ResourceConflictException"
            | "ResourceNotFoundException"
            | "UnsupportedMediaTypeException",
        ) => Retryability::Permanent,
        _ => Retryability::Retryable,
    }
}
