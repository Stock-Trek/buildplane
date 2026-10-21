use crate::error::{BaError, BaResult};
use std::{
    env::{self, VarError},
    path::{Path, PathBuf},
    time::Duration,
};
use tracing_subscriber::EnvFilter;

const BUILD_TIMEOUT_ENV: &str = "BUILD_AGENT_TIMEOUT_SECS";
const SOURCE_DIR_ENV: &str = "BUILD_AGENT_SOURCE_DIR";
const BUILD_CACHE_DIR_ENV: &str = "BUILD_AGENT_BUILD_CACHE_DIR";
const DEFAULT_BUILD_TIMEOUT_SECS: u64 = 300;
const DEFAULT_LOG_FILTER: &str = "info";
const DEFAULT_SOURCE_DIR: &str = "/var/task";
const DEFAULT_BUILD_CACHE_DIR: &str = "/tmp/buildplane-build-cache";

/// Process-wide configuration for the build agent.
///
/// Values that change for every build (the signed URLs and repository
/// metadata) are read from the Lambda invocation payload instead. Only the
/// settings that are genuinely part of the function configuration live here.
#[derive(Debug)]
pub struct EnvVarConfig {
    build_timeout: Duration,
    log_filter: EnvFilter,
    source_dir: PathBuf,
    build_cache_dir: PathBuf,
}

impl EnvVarConfig {
    pub fn from_env() -> BaResult<Self> {
        Ok(Self {
            build_timeout: Self::build_timeout_from_env()?,
            log_filter: Self::log_filter_from_env()?,
            source_dir: Self::path_from_env(SOURCE_DIR_ENV, DEFAULT_SOURCE_DIR)?,
            build_cache_dir: Self::path_from_env(BUILD_CACHE_DIR_ENV, DEFAULT_BUILD_CACHE_DIR)?,
        })
    }

    pub fn build_timeout(&self) -> Duration {
        self.build_timeout
    }

    pub fn log_filter(&self) -> EnvFilter {
        self.log_filter.clone()
    }

    pub fn source_dir(&self) -> &Path {
        &self.source_dir
    }

    pub fn build_cache_dir(&self) -> &Path {
        &self.build_cache_dir
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

    fn path_from_env(name: &str, default: &str) -> BaResult<PathBuf> {
        match Self::optional(name)? {
            Some(value) if value.is_empty() => Ok(PathBuf::from(default)),
            Some(value) => Ok(PathBuf::from(value)),
            None => Ok(PathBuf::from(default)),
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
