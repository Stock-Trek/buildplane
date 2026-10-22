use crate::errors::{
    config::ConfigError, connector::ConnectorError, data::DataError, exchange::ExchangeError,
    github::GitHubError, http::HttpError, io::IoError, lambda::LambdaError,
    rate_limit::RateLimitError, retryability::Retryability, secrets::SecretsError, sqs::SqsError,
    storage::StorageError, system::SystemError, timeout::TimeoutError, validation::ValidationError,
};
use lambda_runtime::Diagnostic;
use std::{path::PathBuf, process::ExitStatus, time::Duration};

pub type BpResult<T> = Result<T, BpError>;

#[derive(Debug, thiserror::Error)]
pub enum BpError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error(transparent)]
    GitHub(#[from] GitHubError),
    #[error(transparent)]
    Exchange(#[from] ExchangeError),
    #[error(transparent)]
    Connector(#[from] ConnectorError),
    #[error(transparent)]
    RateLimit(#[from] RateLimitError),
    #[error(transparent)]
    Sqs(#[from] SqsError),
    #[error(transparent)]
    Lambda(#[from] LambdaError),
    #[error(transparent)]
    Data(#[from] DataError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Secrets(#[from] SecretsError),
    #[error(transparent)]
    Timeout(#[from] TimeoutError),
}

impl BpError {
    pub fn config_missing(name: impl Into<String>) -> Self {
        Self::Config(ConfigError::Missing { name: name.into() })
    }

    pub fn config_empty(name: impl Into<String>) -> Self {
        Self::Config(ConfigError::Empty { name: name.into() })
    }

    pub fn config_not_unicode(name: impl Into<String>) -> Self {
        Self::Config(ConfigError::NotUnicode { name: name.into() })
    }

    pub fn config_invalid(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Config(ConfigError::Invalid {
            name: name.into(),
            message: message.into(),
            source: None,
        })
    }

    pub fn config_invalid_source(
        name: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config(ConfigError::Invalid {
            name: name.into(),
            message: source.to_string(),
            source: Some(Box::new(source)),
        })
    }

    pub fn invalid_path(value: impl Into<String>) -> Self {
        Self::Validation(ValidationError::InvalidPath {
            value: value.into(),
        })
    }

    pub fn io<E>(operation: &'static str, path: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::Io(IoError::Io {
            operation,
            path: path.into(),
            source: source.into(),
        })
    }

    pub fn s3<E>(operation: &'static str, target: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::s3_with_retryability(operation, target, true, source)
    }

    pub fn s3_with_retryability<E>(
        operation: &'static str,
        target: impl Into<String>,
        retryable: bool,
        source: E,
    ) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::Storage(StorageError::S3 {
            operation,
            target: target.into(),
            retryable,
            source: source.into(),
        })
    }

    pub fn exchange<E>(operation: &'static str, source: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::Exchange(ExchangeError::Exchange {
            operation,
            source: source.into(),
        })
    }

    pub fn candle_conversion<E>(field: &'static str, source: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self::Data(DataError::CandleConversion {
            field,
            source: source.into(),
        })
    }

    pub fn http_request(
        operation: &'static str,
        target: impl Into<String>,
        source: reqwest::Error,
    ) -> Self {
        Self::Http(HttpError::Request {
            operation,
            target: target.into(),
            source,
        })
    }

    pub fn http_rejected(
        operation: &'static str,
        target: impl Into<String>,
        status: u16,
        body: impl Into<String>,
    ) -> Self {
        Self::Http(HttpError::Rejected {
            operation,
            target: target.into(),
            status,
            body: body.into(),
        })
    }

    pub fn command_failed(
        operation: &'static str,
        path: impl Into<PathBuf>,
        status: ExitStatus,
    ) -> Self {
        Self::Io(IoError::CommandFailed {
            operation,
            path: path.into(),
            status,
        })
    }

    pub fn external(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::System(SystemError::External(source.into()))
    }

    pub fn send_not_sent(source: Self) -> Self {
        Self::Connector(ConnectorError::send_not_sent(source))
    }

    pub fn send_not_sent_external(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Connector(ConnectorError::send_not_sent_external(source))
    }

    pub fn send_failed(source: Self) -> Self {
        Self::Connector(ConnectorError::send_failed(source))
    }

    pub fn send_unknown(source: Self) -> Self {
        Self::Connector(ConnectorError::send_unknown(source))
    }

    pub fn send_unknown_external(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Connector(ConnectorError::send_unknown_external(source))
    }

    pub fn multiple(failures: impl IntoIterator<Item = BpError>) -> Self {
        Self::Data(DataError::multiple(failures))
    }

    pub fn is_skippable(&self) -> bool {
        matches!(
            self,
            Self::Exchange(
                ExchangeError::MissingSymbol { .. }
                    | ExchangeError::UnsupportedExchange { .. }
                    | ExchangeError::UnsupportedTradingMode { .. }
            ) | Self::Data(DataError::LockContention { .. })
        )
    }

    pub fn any_sent(&self) -> bool {
        matches!(self, Self::Sqs(error) if error.any_sent())
    }

    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::GitHub(GitHubError::HostInvalid { .. } | GitHubError::HostNotAllowed { .. })
        )
    }

    pub fn was_not_sent(&self) -> bool {
        matches!(self, Self::Connector(error) if error.was_not_sent())
    }

    pub fn has_unknown_response(&self) -> bool {
        matches!(self, Self::Connector(error) if error.has_unknown_response())
    }

    pub fn error_type(&self) -> &'static str {
        match self {
            Self::Config(error) => error.error_type(),
            Self::Validation(error) => error.error_type(),
            Self::Io(error) => error.error_type(),
            Self::Storage(error) => error.error_type(),
            Self::Http(error) => error.error_type(),
            Self::GitHub(error) => error.error_type(),
            Self::Exchange(error) => error.error_type(),
            Self::Connector(error) => error.error_type(),
            Self::RateLimit(error) => error.error_type(),
            Self::Sqs(error) => error.error_type(),
            Self::Lambda(error) => error.error_type(),
            Self::Data(error) => error.error_type(),
            Self::System(error) => error.error_type(),
            Self::Secrets(error) => error.error_type(),
            Self::Timeout(error) => error.error_type(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retryability() == Retryability::Retryable
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Config(error) => error.retryability(),
            Self::Validation(error) => error.retryability(),
            Self::Io(error) => error.retryability(),
            Self::Storage(error) => error.retryability(),
            Self::Http(error) => error.retryability(),
            Self::GitHub(error) => error.retryability(),
            Self::Exchange(error) => error.retryability(),
            Self::Connector(error) => error.retryability(),
            Self::RateLimit(error) => error.retryability(),
            Self::Sqs(error) => error.retryability(),
            Self::Lambda(error) => error.retryability(),
            Self::Data(error) => error.retryability(),
            Self::System(error) => error.retryability(),
            Self::Secrets(error) => error.retryability(),
            Self::Timeout(error) => error.retryability(),
        }
    }
}

impl From<BpError> for Diagnostic {
    fn from(value: BpError) -> Self {
        Diagnostic {
            error_type: value.error_type().into(),
            error_message: value.to_string(),
        }
    }
}

impl From<std::io::Error> for BpError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(IoError::from(error))
    }
}

impl From<zip::result::ZipError> for BpError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Io(IoError::from(error))
    }
}

impl From<arrow_schema::ArrowError> for BpError {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self::Io(IoError::from(error))
    }
}

impl From<parquet::errors::ParquetError> for BpError {
    fn from(error: parquet::errors::ParquetError) -> Self {
        Self::Io(IoError::from(error))
    }
}

impl From<tokio::task::JoinError> for BpError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::System(SystemError::from(error))
    }
}

impl From<serde_json::Error> for BpError {
    fn from(error: serde_json::Error) -> Self {
        Self::Http(HttpError::from(error))
    }
}

impl From<http::Error> for BpError {
    fn from(error: http::Error) -> Self {
        Self::Http(HttpError::from(error))
    }
}

impl From<reqwest::Error> for BpError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(HttpError::from(error))
    }
}

impl From<tracing::subscriber::SetGlobalDefaultError> for BpError {
    fn from(error: tracing::subscriber::SetGlobalDefaultError) -> Self {
        Self::System(SystemError::from(error))
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for BpError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::System(SystemError::External(error))
    }
}

impl From<aws_sdk_lambda::operation::invoke::InvokeError> for BpError {
    fn from(error: aws_sdk_lambda::operation::invoke::InvokeError) -> Self {
        Self::Lambda(LambdaError::from(error))
    }
}

impl From<Duration> for BpError {
    fn from(duration: Duration) -> Self {
        Self::Timeout(TimeoutError::BuildTimeout(duration))
    }
}
