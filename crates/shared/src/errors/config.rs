use crate::errors::retryability::Retryability;
use std::error::Error as StdError;

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ConfigError {
    #[error("{name} is missing")]
    Missing { name: String },
    #[error("{name} must not be empty")]
    Empty { name: String },
    #[error("{name} is not valid UTF-8")]
    NotUnicode { name: String },
    #[error("{name} is invalid: {message}")]
    Invalid {
        name: String,
        message: String,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    },
    #[error("invalid configuration: {0}")]
    Build(String),
}

impl ConfigError {
    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn retryability(&self) -> Retryability {
        Retryability::Permanent
    }
}
