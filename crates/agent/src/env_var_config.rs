use crate::error::{BaError, BaResult};
use std::{
    env::{self, VarError},
    time::Duration,
};
use tracing_subscriber::EnvFilter;

const BUILD_TIMEOUT_ENV: &str = "BUILD_AGENT_TIMEOUT_SECS";
const DEFAULT_BUILD_TIMEOUT_SECS: u64 = 300;
const DEFAULT_LOG_FILTER: &str = "info";

/// Process-wide configuration for the build agent.
///
/// Values that change for every build (the signed URLs and repository
/// metadata) are read from the Lambda invocation payload instead. Only the
/// settings that are genuinely part of the function configuration live here.
#[derive(Debug)]
pub struct EnvVarConfig {
    build_timeout: Duration,
    log_filter: EnvFilter,
}

impl EnvVarConfig {
    pub fn from_env() -> BaResult<Self> {
        Ok(Self {
            build_timeout: Self::build_timeout_from_env()?,
            log_filter: Self::log_filter_from_env()?,
        })
    }

    pub fn build_timeout(&self) -> Duration {
        self.build_timeout
    }

    pub fn log_filter(&self) -> EnvFilter {
        self.log_filter.clone()
    }

    fn build_timeout_from_env() -> BaResult<Duration> {
        match Self::optional(BUILD_TIMEOUT_ENV)? {
            Some(value) => value
                .parse::<u64>()
                .map(Duration::from_secs)
                .map_err(|error| {
                    BaError::Config(format!("{BUILD_TIMEOUT_ENV} is invalid: {error}"))
                }),
            None => Ok(Duration::from_secs(DEFAULT_BUILD_TIMEOUT_SECS)),
        }
    }

    fn log_filter_from_env() -> BaResult<EnvFilter> {
        match Self::optional(EnvFilter::DEFAULT_ENV)? {
            Some(directives) => EnvFilter::try_new(&directives).map_err(|error| {
                BaError::Config(format!("{} is invalid: {error}", EnvFilter::DEFAULT_ENV))
            }),
            None => Ok(EnvFilter::new(DEFAULT_LOG_FILTER)),
        }
    }

    fn optional(name: &str) -> BaResult<Option<String>> {
        match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(VarError::NotPresent) => Ok(None),
            Err(VarError::NotUnicode(_)) => {
                Err(BaError::Config(format!("{name} is not valid UTF-8")))
            }
        }
    }
}
