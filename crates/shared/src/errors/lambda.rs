use crate::errors::retryability::Retryability;
use aws_sdk_lambda::operation::invoke::InvokeError as SdkInvokeError;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum LambdaError {
    #[error("Lambda function returned {function_error}: {detail}", detail = .detail.as_deref().unwrap_or("no detail"))]
    Function {
        function_error: String,
        detail: Option<String>,
    },
    #[error("Lambda invoke returned unexpected status code {status_code}")]
    Status { status_code: i32 },
    #[error("Lambda invoke error: {0}")]
    SdkInvoke(#[from] Box<SdkInvokeError>),
}

impl LambdaError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::SdkInvoke(error) => Retryability::from_lambda_invoke(error),
            Self::Function { .. } | Self::Status { .. } => Retryability::Permanent,
        }
    }
}
