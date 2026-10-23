use crate::git::DirectoryLimits;
use buildplane_errors::{BpError, BpResult};
use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
    time::Duration,
};
use tracing_subscriber::EnvFilter;
use url::Url;

const AWS_ENDPOINT_URL_S3: &str = "AWS_ENDPOINT_URL_S3";
const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";
const BACKFILL_START: &str = "BACKFILL_START";
const BUILD_AGENT_BUILD_CACHE_DIR: &str = "BUILD_AGENT_BUILD_CACHE_DIR";
const BUILD_AGENT_FUNCTION_NAME: &str = "BUILD_AGENT_FUNCTION_NAME";
const BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES: &str =
    "BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES";
const BUILD_AGENT_SOURCE_DIR: &str = "BUILD_AGENT_SOURCE_DIR";
const BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS: &str = "BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS";
const BUILD_AGENT_TIMEOUT_SECS: &str = "BUILD_AGENT_TIMEOUT_SECS";
const DEADLINE_BUFFER_SECS: &str = "DEADLINE_BUFFER_SECS";
const ENTERPRISE_ALLOWED_HOSTS: &str = "GITHUB_ENTERPRISE_ALLOWED_HOSTS";
const GITHUB_APP_ID: &str = "GITHUB_APP_ID";
const GITHUB_APP_PRIVATE_KEY: &str = "GITHUB_APP_PRIVATE_KEY";
const GITHUB_MAX_CACHED_INSTALLATIONS: &str = "GITHUB_MAX_CACHED_INSTALLATIONS";
const GIT_MAX_DIRECTORY_BYTES: &str = "GIT_MAX_DIRECTORY_BYTES";
const GIT_MAX_DIRECTORY_DEPTH: &str = "GIT_MAX_DIRECTORY_DEPTH";
const GIT_MAX_DIRECTORY_FILES: &str = "GIT_MAX_DIRECTORY_FILES";
const INFISICAL_BASE_URL: &str = "INFISICAL_BASE_URL";
const INFISICAL_ENVIRONMENT: &str = "INFISICAL_ENVIRONMENT";
const INFISICAL_IDENTITY_ID: &str = "INFISICAL_IDENTITY_ID";
const INFISICAL_PROJECT_ID: &str = "INFISICAL_PROJECT_ID";
const RUST_LOG: &str = "RUST_LOG";
const S3_BUCKET: &str = "S3_BUCKET";
const S3_BUCKET_USER_ARTIFACTS: &str = "S3_BUCKET_USER_ARTIFACTS";
const S3_MAX_ARTIFACT_SIZE_BYTES: &str = "S3_MAX_ARTIFACT_SIZE_BYTES";
const S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES: &str = "S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES";
pub const S3_SIGNED_URL_EXPIRY_SECS: &str = "S3_SIGNED_URL_EXPIRY_SECS";
const SQS_DEDUP_MAX_MESSAGES: &str = "SQS_DEDUP_MAX_MESSAGES";
const SQS_DEDUP_TTL_SECS: &str = "SQS_DEDUP_TTL_SECS";
const SQS_PROCESS_CONCURRENCY: &str = "SQS_PROCESS_CONCURRENCY";
const SQS_QUEUE_URL: &str = "SQS_QUEUE_URL";
const WEBHOOK_COLD_START_REFRESH_TIMEOUT_SECS: &str = "WEBHOOK_COLD_START_REFRESH_TIMEOUT_SECS";
const WEBHOOK_DEDUP_MAX_DELIVERIES: &str = "WEBHOOK_DEDUP_MAX_DELIVERIES";
const WEBHOOK_DEDUP_TTL_SECS: &str = "WEBHOOK_DEDUP_TTL_SECS";
const WEBHOOK_REFRESH_COOLDOWN_SECS: &str = "WEBHOOK_REFRESH_COOLDOWN_SECS";
const WEBHOOK_SECRET_FAILURE_BACKOFF_SECS: &str = "WEBHOOK_SECRET_FAILURE_BACKOFF_SECS";

const DEFAULT_BUILD_AGENT_BUILD_CACHE_DIR: &str = "/tmp/buildplane-build-cache";
const DEFAULT_BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_BUILD_AGENT_SOURCE_DIR: &str = "/var/task";
const DEFAULT_BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_BUILD_AGENT_TIMEOUT: Duration = Duration::from_secs(300);
const DEFAULT_DEADLINE_BUFFER: Duration = Duration::from_secs(5);
const DEFAULT_GITHUB_MAX_CACHED_INSTALLATIONS: usize = 128;
const DEFAULT_S3_MAX_ARTIFACT_SIZE_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_S3_SIGNED_URL_EXPIRY: Duration = Duration::from_secs(60 * 60);
const DEFAULT_SQS_DEDUP_MAX_MESSAGES: usize = 4096;
const DEFAULT_SQS_DEDUP_TTL: Duration = Duration::from_secs(3600);
const DEFAULT_SQS_PROCESS_CONCURRENCY: usize = 10;
const DEFAULT_WEBHOOK_COLD_START_REFRESH_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_WEBHOOK_DEDUP_MAX_DELIVERIES: usize = 4096;
const DEFAULT_WEBHOOK_DEDUP_TTL: Duration = Duration::from_secs(3600);
const DEFAULT_WEBHOOK_REFRESH_COOLDOWN: Duration = Duration::from_secs(30);
const DEFAULT_WEBHOOK_SECRET_FAILURE_BACKOFF: Duration = Duration::from_secs(5);
const MAX_SIGNED_URL_EXPIRY: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// The complete set of environment variables the services read.
///
/// The process environment is captured once by [`EnvVarConfig::from_env`] and
/// every value is exposed through a typed accessor, so environment parsing is
/// collated in a single place and can be injected in tests via
/// [`EnvVarConfig::from_map`].
#[derive(Debug, Clone, Default)]
pub struct EnvVarConfig {
    vars: HashMap<OsString, OsString>,
}

impl EnvVarConfig {
    /// Captures and validates the process environment.
    ///
    /// Every recognised variable that is present is parsed eagerly, so an
    /// invalid value fails at startup instead of on the first request that
    /// happens to read it. Required variables are validated when they are set;
    /// each service still decides which required variables it needs when it
    /// builds its typed configuration.
    pub fn from_env() -> BpResult<Self> {
        let config = Self {
            vars: std::env::vars_os().collect(),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn from_map<I, K, V>(vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self {
            vars: vars
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
        }
    }

    /// Validates every recognised environment variable that is present.
    ///
    /// Defaults are valid by construction, so variables that are absent are
    /// left for the service-specific configuration to require or default.
    pub fn validate(&self) -> BpResult<()> {
        self.lambda_function_name("")?;
        if let Some(directives) = self.log_filter()? {
            EnvFilter::try_new(&directives)
                .map_err(|error| BpError::config_invalid_source(EnvFilter::DEFAULT_ENV, error))?;
        }
        self.backfill_start()?;
        self.deadline_buffer()?;
        self.validate_if_present(GITHUB_APP_ID, || self.github_app_id())?;
        self.validate_if_present(GITHUB_APP_PRIVATE_KEY, || self.github_app_private_key())?;
        self.github_enterprise_allowed_hosts()?;
        self.github_max_cached_installations()?;
        self.git_max_directory_depth()?;
        self.git_max_directory_files()?;
        self.git_max_directory_bytes()?;
        self.validate_if_present(SQS_QUEUE_URL, || self.sqs_queue_url())?;
        self.sqs_dedup_max_messages()?;
        self.sqs_dedup_ttl()?;
        self.sqs_process_concurrency()?;
        self.validate_if_present(S3_BUCKET, || self.s3_bucket())?;
        self.validate_if_present(S3_BUCKET_USER_ARTIFACTS, || self.s3_bucket_user_artifacts())?;
        self.s3_max_source_archive_size_bytes()?;
        self.s3_max_artifact_size_bytes()?;
        self.s3_signed_url_expiry()?;
        self.aws_endpoint_url_s3()?;
        self.build_agent_build_cache_dir()?;
        self.validate_if_present(BUILD_AGENT_FUNCTION_NAME, || {
            self.build_agent_function_name()
        })?;
        self.build_agent_source_dir()?;
        self.build_agent_max_extracted_source_size_bytes()?;
        self.build_agent_source_download_timeout()?;
        self.build_agent_timeout()?;
        self.webhook_dedup_max_deliveries()?;
        self.webhook_dedup_ttl()?;
        self.webhook_refresh_cooldown()?;
        self.webhook_secret_failure_backoff()?;
        self.webhook_cold_start_refresh_timeout()?;
        self.validate_if_present(INFISICAL_IDENTITY_ID, || self.infisical_identity_id())?;
        self.validate_if_present(INFISICAL_PROJECT_ID, || self.infisical_project_id())?;
        self.validate_if_present(INFISICAL_ENVIRONMENT, || self.infisical_environment())?;
        self.validate_if_present(INFISICAL_BASE_URL, || self.infisical_base_url())?;
        for key in [
            "AWS_CONNECT_TIMEOUT_SECS",
            "AWS_OPERATION_TIMEOUT_SECS",
            "GITHUB_CONNECT_TIMEOUT_SECS",
            "GITHUB_OPERATION_TIMEOUT_SECS",
        ] {
            self.validate_if_present(key, || self.duration(key, Duration::ZERO).map(drop))?;
        }
        self.validate_url_overrides()?;
        Ok(())
    }

    fn validate_url_overrides(&self) -> BpResult<()> {
        for (key, value) in &self.vars {
            let Some(key) = key.to_str() else {
                continue;
            };
            let allowed_schemes = if key.ends_with("_HTTP_REAL") || key.ends_with("_HTTP_PAPER") {
                &["http", "https"][..]
            } else if key.ends_with("_WEBSOCKET_REAL") || key.ends_with("_WEBSOCKET_PAPER") {
                &["ws", "wss"][..]
            } else {
                continue;
            };
            let value = value
                .to_str()
                .ok_or_else(|| BpError::config_not_unicode(key))?;
            if value.is_empty() {
                return Err(BpError::config_empty(key));
            }
            let url =
                Url::parse(value).map_err(|error| BpError::config_invalid_source(key, error))?;
            if !allowed_schemes.contains(&url.scheme()) {
                return Err(BpError::config_invalid(
                    key,
                    format!(
                        "must use one of the schemes {}, got \"{}\"",
                        allowed_schemes.join(", "),
                        url.scheme()
                    ),
                ));
            }
        }
        Ok(())
    }

    fn validate_if_present<T>(
        &self,
        key: &str,
        accessor: impl FnOnce() -> BpResult<T>,
    ) -> BpResult<()> {
        if self.vars.contains_key(OsStr::new(key)) {
            accessor()?;
        }
        Ok(())
    }

    pub fn lambda_function_name(&self, default: &str) -> BpResult<String> {
        self.parsed(AWS_LAMBDA_FUNCTION_NAME, default.to_owned())
    }

    pub fn log_filter(&self) -> BpResult<Option<String>> {
        self.optional(RUST_LOG)
    }

    pub fn backfill_start(&self) -> BpResult<Option<DateTime<Utc>>> {
        let Some(value) = self.optional(BACKFILL_START)? else {
            return Ok(None);
        };
        Self::parse_timestamp(BACKFILL_START, value.trim()).map(Some)
    }

    pub fn deadline_buffer(&self) -> BpResult<Duration> {
        self.duration(DEADLINE_BUFFER_SECS, DEFAULT_DEADLINE_BUFFER)
    }

    pub fn github_app_id(&self) -> BpResult<u64> {
        self.required_parsed(GITHUB_APP_ID)
    }

    pub fn github_app_private_key(&self) -> BpResult<String> {
        Ok(self.required(GITHUB_APP_PRIVATE_KEY)?.replace("\\n", "\n"))
    }

    pub fn github_enterprise_allowed_hosts(&self) -> BpResult<Vec<String>> {
        self.optional_csv(ENTERPRISE_ALLOWED_HOSTS)
    }

    pub fn github_max_cached_installations(&self) -> BpResult<NonZeroUsize> {
        let installations = self.parsed(
            GITHUB_MAX_CACHED_INSTALLATIONS,
            DEFAULT_GITHUB_MAX_CACHED_INSTALLATIONS,
        )?;
        NonZeroUsize::new(installations).ok_or_else(|| {
            BpError::config_invalid(GITHUB_MAX_CACHED_INSTALLATIONS, "must be greater than zero")
        })
    }

    pub fn git_max_directory_depth(&self) -> BpResult<NonZeroUsize> {
        let depth = self.parsed(
            GIT_MAX_DIRECTORY_DEPTH,
            DirectoryLimits::default().max_depth,
        )?;
        NonZeroUsize::new(depth).ok_or_else(|| {
            BpError::config_invalid(GIT_MAX_DIRECTORY_DEPTH, "must be greater than zero")
        })
    }

    pub fn git_max_directory_files(&self) -> BpResult<NonZeroUsize> {
        let files = self.parsed(
            GIT_MAX_DIRECTORY_FILES,
            DirectoryLimits::default().max_files,
        )?;
        NonZeroUsize::new(files).ok_or_else(|| {
            BpError::config_invalid(GIT_MAX_DIRECTORY_FILES, "must be greater than zero")
        })
    }

    pub fn git_max_directory_bytes(&self) -> BpResult<NonZeroU64> {
        let bytes = self.parsed(
            GIT_MAX_DIRECTORY_BYTES,
            DirectoryLimits::default().max_total_bytes,
        )?;
        NonZeroU64::new(bytes).ok_or_else(|| {
            BpError::config_invalid(GIT_MAX_DIRECTORY_BYTES, "must be greater than zero")
        })
    }

    pub fn sqs_queue_url(&self) -> BpResult<String> {
        self.required(SQS_QUEUE_URL)
    }

    pub fn sqs_dedup_max_messages(&self) -> BpResult<NonZeroUsize> {
        let messages = self.parsed(SQS_DEDUP_MAX_MESSAGES, DEFAULT_SQS_DEDUP_MAX_MESSAGES)?;
        NonZeroUsize::new(messages).ok_or_else(|| {
            BpError::config_invalid(SQS_DEDUP_MAX_MESSAGES, "must be greater than zero")
        })
    }

    pub fn sqs_dedup_ttl(&self) -> BpResult<Duration> {
        self.duration(SQS_DEDUP_TTL_SECS, DEFAULT_SQS_DEDUP_TTL)
    }

    pub fn sqs_process_concurrency(&self) -> BpResult<NonZeroUsize> {
        let concurrency = self.parsed(SQS_PROCESS_CONCURRENCY, DEFAULT_SQS_PROCESS_CONCURRENCY)?;
        NonZeroUsize::new(concurrency).ok_or_else(|| {
            BpError::config_invalid(SQS_PROCESS_CONCURRENCY, "must be greater than zero")
        })
    }

    pub fn s3_bucket(&self) -> BpResult<String> {
        self.required(S3_BUCKET)
    }

    pub fn s3_bucket_user_artifacts(&self) -> BpResult<String> {
        self.required(S3_BUCKET_USER_ARTIFACTS)
    }

    pub fn s3_max_source_archive_size_bytes(&self) -> BpResult<u64> {
        self.parsed(
            S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES,
            DEFAULT_S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES,
        )
    }

    pub fn s3_max_artifact_size_bytes(&self) -> BpResult<u64> {
        self.parsed(
            S3_MAX_ARTIFACT_SIZE_BYTES,
            DEFAULT_S3_MAX_ARTIFACT_SIZE_BYTES,
        )
    }

    pub fn s3_signed_url_expiry(&self) -> BpResult<Duration> {
        let expiry = self.duration(S3_SIGNED_URL_EXPIRY_SECS, DEFAULT_S3_SIGNED_URL_EXPIRY)?;
        if expiry.is_zero() || expiry > MAX_SIGNED_URL_EXPIRY {
            return Err(BpError::config_invalid(
                S3_SIGNED_URL_EXPIRY_SECS,
                format!(
                    "must be between 1 and {} seconds (7 days)",
                    MAX_SIGNED_URL_EXPIRY.as_secs()
                ),
            ));
        }
        Ok(expiry)
    }

    pub fn aws_endpoint_url_s3(&self) -> BpResult<Option<String>> {
        self.optional(AWS_ENDPOINT_URL_S3)
    }

    pub fn build_agent_build_cache_dir(&self) -> BpResult<PathBuf> {
        self.path(
            BUILD_AGENT_BUILD_CACHE_DIR,
            DEFAULT_BUILD_AGENT_BUILD_CACHE_DIR,
        )
    }

    pub fn build_agent_function_name(&self) -> BpResult<String> {
        self.required(BUILD_AGENT_FUNCTION_NAME)
    }

    pub fn build_agent_source_dir(&self) -> BpResult<PathBuf> {
        self.path(BUILD_AGENT_SOURCE_DIR, DEFAULT_BUILD_AGENT_SOURCE_DIR)
    }

    pub fn build_agent_max_extracted_source_size_bytes(&self) -> BpResult<NonZeroU64> {
        let bytes = self.parsed(
            BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES,
            DEFAULT_BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES,
        )?;
        NonZeroU64::new(bytes).ok_or_else(|| {
            BpError::config_invalid(
                BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES,
                "must be greater than zero",
            )
        })
    }

    pub fn build_agent_source_download_timeout(&self) -> BpResult<Duration> {
        let timeout = self.duration(
            BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS,
            DEFAULT_BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT,
        )?;
        if timeout.is_zero() {
            return Err(BpError::config_invalid(
                BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS,
                "must be greater than zero",
            ));
        }
        Ok(timeout)
    }

    pub fn build_agent_timeout(&self) -> BpResult<Duration> {
        self.duration(BUILD_AGENT_TIMEOUT_SECS, DEFAULT_BUILD_AGENT_TIMEOUT)
    }

    pub fn webhook_dedup_max_deliveries(&self) -> BpResult<NonZeroUsize> {
        let deliveries = self.parsed(
            WEBHOOK_DEDUP_MAX_DELIVERIES,
            DEFAULT_WEBHOOK_DEDUP_MAX_DELIVERIES,
        )?;
        NonZeroUsize::new(deliveries).ok_or_else(|| {
            BpError::config_invalid(WEBHOOK_DEDUP_MAX_DELIVERIES, "must be greater than zero")
        })
    }

    pub fn webhook_dedup_ttl(&self) -> BpResult<Duration> {
        self.duration(WEBHOOK_DEDUP_TTL_SECS, DEFAULT_WEBHOOK_DEDUP_TTL)
    }

    pub fn webhook_refresh_cooldown(&self) -> BpResult<Duration> {
        self.duration(
            WEBHOOK_REFRESH_COOLDOWN_SECS,
            DEFAULT_WEBHOOK_REFRESH_COOLDOWN,
        )
    }

    pub fn webhook_secret_failure_backoff(&self) -> BpResult<Duration> {
        self.duration(
            WEBHOOK_SECRET_FAILURE_BACKOFF_SECS,
            DEFAULT_WEBHOOK_SECRET_FAILURE_BACKOFF,
        )
    }

    pub fn webhook_cold_start_refresh_timeout(&self) -> BpResult<Duration> {
        self.duration(
            WEBHOOK_COLD_START_REFRESH_TIMEOUT_SECS,
            DEFAULT_WEBHOOK_COLD_START_REFRESH_TIMEOUT,
        )
    }

    pub fn infisical_identity_id(&self) -> BpResult<String> {
        self.required(INFISICAL_IDENTITY_ID)
    }

    pub fn infisical_project_id(&self) -> BpResult<String> {
        self.required(INFISICAL_PROJECT_ID)
    }

    pub fn infisical_environment(&self) -> BpResult<String> {
        self.required(INFISICAL_ENVIRONMENT)
    }

    pub fn infisical_base_url(&self) -> BpResult<String> {
        let base_url = self.required(INFISICAL_BASE_URL)?;
        let url = Url::parse(&base_url)
            .map_err(|error| BpError::config_invalid_source(INFISICAL_BASE_URL, error))?;
        if url.scheme() != "https" {
            return Err(BpError::config_invalid(
                INFISICAL_BASE_URL,
                format!("must use the \"https\" scheme, got \"{}\"", url.scheme()),
            ));
        }
        Ok(base_url)
    }

    pub fn duration(&self, key: &str, default: Duration) -> BpResult<Duration> {
        Ok(Duration::from_secs(self.parsed(key, default.as_secs())?))
    }

    fn required_parsed<T>(&self, key: &str) -> BpResult<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.required(key)?
            .parse::<T>()
            .map_err(|error| BpError::config_invalid_source(key, error))
    }

    fn parsed<T>(&self, key: &str, default: T) -> BpResult<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let Some(value) = self.raw(key)? else {
            return Ok(default);
        };
        if value.is_empty() {
            return Err(BpError::config_empty(key));
        }
        value
            .parse::<T>()
            .map_err(|error| BpError::config_invalid_source(key, error))
    }

    fn path(&self, key: &str, default: &str) -> BpResult<PathBuf> {
        Ok(self
            .optional(key)?
            .map_or_else(|| PathBuf::from(default), PathBuf::from))
    }

    fn parse_timestamp(name: &str, value: &str) -> BpResult<DateTime<Utc>> {
        if let Ok(date_time) = DateTime::parse_from_rfc3339(value) {
            return Ok(date_time.with_timezone(&Utc));
        }
        if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
                return Err(Self::invalid_timestamp(name, value));
            };
            return Ok(date_time.and_utc());
        }
        Err(Self::invalid_timestamp(name, value))
    }

    fn invalid_timestamp(name: &str, value: &str) -> BpError {
        BpError::config_invalid(
            name,
            format!("must be an RFC 3339 timestamp or YYYY-MM-DD date, got {value}"),
        )
    }

    fn optional_csv(&self, name: &str) -> BpResult<Vec<String>> {
        match self.optional(name)? {
            Some(value) => Ok(Self::split_csv(&value)),
            None => Ok(Vec::new()),
        }
    }

    /// Read an environment variable by name without applying any parsing.
    ///
    /// This is for values whose names are only known at runtime, such as the
    /// per-exchange URL overrides. Missing values are reported as
    /// [`ConfigError::Missing`](buildplane_errors::ConfigError::Missing) and empty
    /// values as [`ConfigError::Empty`](buildplane_errors::ConfigError::Empty).
    pub fn required(&self, name: &str) -> BpResult<String> {
        match self.raw(name)? {
            Some(value) if value.is_empty() => Err(BpError::config_empty(name)),
            Some(value) => Ok(value),
            None => Err(BpError::config_missing(name)),
        }
    }

    pub fn required_lossy(&self, name: &str) -> BpResult<String> {
        self.vars
            .get(OsStr::new(name))
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string_lossy().into_owned())
            .ok_or_else(|| BpError::config_missing(name))
    }

    fn optional(&self, name: &str) -> BpResult<Option<String>> {
        Ok(self.raw(name)?.filter(|value| !value.is_empty()))
    }

    fn raw(&self, name: &str) -> BpResult<Option<String>> {
        match self.vars.get(OsStr::new(name)) {
            Some(value) => value
                .clone()
                .into_string()
                .map(Some)
                .map_err(|_| BpError::config_not_unicode(name)),
            None => Ok(None),
        }
    }

    fn split_csv(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildplane_errors::ConfigError;

    #[test]
    fn validate_rejects_invalid_values_for_present_variables() {
        let config = EnvVarConfig::from_map([
            (BUILD_AGENT_TIMEOUT_SECS, "not a number"),
            (S3_SIGNED_URL_EXPIRY_SECS, "0"),
            (INFISICAL_BASE_URL, "http://infisical.test"),
            ("AWS_CONNECT_TIMEOUT_SECS", "soon"),
        ]);

        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_values_and_absent_required_variables() {
        let config = EnvVarConfig::from_map([
            (BUILD_AGENT_TIMEOUT_SECS, "120"),
            (S3_SIGNED_URL_EXPIRY_SECS, "60"),
            (INFISICAL_BASE_URL, "https://infisical.test"),
            ("AWS_CONNECT_TIMEOUT_SECS", "10"),
            ("BP_URL_TEST_HTTP_REAL", "https://example.test/api"),
            ("BP_URL_TEST_WEBSOCKET_REAL", "wss://example.test/ws"),
            ("RUST_LOG", "info,buildplane_config=debug"),
        ]);

        config.validate().unwrap();
    }

    #[test]
    fn validate_rejects_invalid_url_overrides() {
        let config = EnvVarConfig::from_map([("BP_URL_TEST_HTTP_REAL", "not a url")]);

        assert!(config.validate().is_err());

        let config =
            EnvVarConfig::from_map([("BP_URL_TEST_WEBSOCKET_REAL", "https://example.test")]);

        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_log_filters() {
        let config = EnvVarConfig::from_map([("RUST_LOG", "not a valid [directive")]);

        assert!(config.validate().is_err());
    }

    #[test]
    fn required_reports_missing_variable_name() {
        let config = EnvVarConfig::from_map(Vec::<(String, String)>::new());

        let error = config
            .required("BUILDPLANE_TEST_MISSING_ENV_VAR")
            .unwrap_err();

        assert!(matches!(
            &error,
            BpError::Config(ConfigError::Missing { name })
                if name == "BUILDPLANE_TEST_MISSING_ENV_VAR"
        ));
        assert!(
            error
                .to_string()
                .contains("BUILDPLANE_TEST_MISSING_ENV_VAR")
        );
    }

    #[test]
    fn required_reports_empty_variable_as_empty() {
        let config = EnvVarConfig::from_map([("BUILDPLANE_TEST_EMPTY_ENV_VAR", "")]);

        let error = config
            .required("BUILDPLANE_TEST_EMPTY_ENV_VAR")
            .unwrap_err();

        assert!(matches!(
            &error,
            BpError::Config(ConfigError::Empty { name })
                if name == "BUILDPLANE_TEST_EMPTY_ENV_VAR"
        ));
    }

    #[test]
    #[cfg(unix)]
    fn required_reports_not_unicode_without_value() {
        use std::os::unix::ffi::OsStringExt;

        let name = "BUILDPLANE_TEST_NOT_UNICODE_ENV_VAR";
        let value = OsString::from_vec(b"super-secret-\xff".to_vec());
        let config = EnvVarConfig::from_map([(OsString::from(name), value)]);

        let error = config.required(name).unwrap_err();

        assert!(matches!(
            &error,
            BpError::Config(ConfigError::NotUnicode { name: error_name })
                if error_name == name
        ));
        let message = error.to_string();
        assert!(message.contains(name));
        assert!(!message.contains("super-secret"));
    }

    #[test]
    fn optional_treats_missing_variable_as_absent() {
        let config = EnvVarConfig::from_map(Vec::<(String, String)>::new());

        let value = config.optional("BUILDPLANE_TEST_MISSING_ENV_VAR").unwrap();

        assert!(value.is_none());
    }

    #[test]
    fn build_agent_settings_have_defaults() {
        let config = EnvVarConfig::from_map(Vec::<(String, String)>::new());

        assert_eq!(
            config.build_agent_timeout().unwrap(),
            Duration::from_secs(300)
        );
        assert_eq!(
            config.build_agent_source_dir().unwrap(),
            PathBuf::from("/var/task")
        );
        assert_eq!(
            config.build_agent_build_cache_dir().unwrap(),
            PathBuf::from("/tmp/buildplane-build-cache")
        );
        assert_eq!(
            config.build_agent_source_download_timeout().unwrap(),
            Duration::from_secs(60)
        );
        assert_eq!(
            config
                .build_agent_max_extracted_source_size_bytes()
                .unwrap()
                .get(),
            64 * 1024 * 1024
        );
    }

    #[test]
    fn build_agent_settings_are_parsed() {
        let config = EnvVarConfig::from_map([
            (BUILD_AGENT_TIMEOUT_SECS, "120"),
            (BUILD_AGENT_SOURCE_DIR, "/srv/agent"),
            (BUILD_AGENT_BUILD_CACHE_DIR, "/var/cache/agent"),
            (BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS, "45"),
            (BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES, "2048"),
        ]);

        assert_eq!(
            config.build_agent_timeout().unwrap(),
            Duration::from_secs(120)
        );
        assert_eq!(
            config.build_agent_source_dir().unwrap(),
            PathBuf::from("/srv/agent")
        );
        assert_eq!(
            config.build_agent_build_cache_dir().unwrap(),
            PathBuf::from("/var/cache/agent")
        );
        assert_eq!(
            config.build_agent_source_download_timeout().unwrap(),
            Duration::from_secs(45)
        );
        assert_eq!(
            config
                .build_agent_max_extracted_source_size_bytes()
                .unwrap()
                .get(),
            2048
        );
    }

    #[test]
    fn build_agent_source_limits_reject_zero() {
        let config = EnvVarConfig::from_map([
            (BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS, "0"),
            (BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES, "0"),
        ]);

        assert!(config.build_agent_source_download_timeout().is_err());
        assert!(
            config
                .build_agent_max_extracted_source_size_bytes()
                .is_err()
        );
    }

    #[test]
    fn git_directory_limits_are_parsed() {
        let config = EnvVarConfig::from_map([
            (GIT_MAX_DIRECTORY_DEPTH, "4"),
            (GIT_MAX_DIRECTORY_FILES, "12"),
            (GIT_MAX_DIRECTORY_BYTES, "2048"),
        ]);

        assert_eq!(config.git_max_directory_depth().unwrap().get(), 4);
        assert_eq!(config.git_max_directory_files().unwrap().get(), 12);
        assert_eq!(config.git_max_directory_bytes().unwrap().get(), 2048);
    }

    #[test]
    fn duration_variables_use_secs_suffix() {
        let config = EnvVarConfig::from_map([
            ("DEADLINE_BUFFER_SECS", "1"),
            ("S3_SIGNED_URL_EXPIRY_SECS", "2"),
            ("SQS_DEDUP_TTL_SECS", "3"),
            ("WEBHOOK_COLD_START_REFRESH_TIMEOUT_SECS", "4"),
            ("WEBHOOK_DEDUP_TTL_SECS", "5"),
            ("WEBHOOK_REFRESH_COOLDOWN_SECS", "6"),
            ("WEBHOOK_SECRET_FAILURE_BACKOFF_SECS", "7"),
        ]);

        assert_eq!(config.deadline_buffer().unwrap(), Duration::from_secs(1));
        assert_eq!(
            config.s3_signed_url_expiry().unwrap(),
            Duration::from_secs(2)
        );
        assert_eq!(config.sqs_dedup_ttl().unwrap(), Duration::from_secs(3));
        assert_eq!(
            config.webhook_cold_start_refresh_timeout().unwrap(),
            Duration::from_secs(4)
        );
        assert_eq!(config.webhook_dedup_ttl().unwrap(), Duration::from_secs(5));
        assert_eq!(
            config.webhook_refresh_cooldown().unwrap(),
            Duration::from_secs(6)
        );
        assert_eq!(
            config.webhook_secret_failure_backoff().unwrap(),
            Duration::from_secs(7)
        );
    }

    #[test]
    fn empty_duration_reports_empty() {
        let config = EnvVarConfig::from_map([("DEADLINE_BUFFER_SECS", "")]);

        let error = config.deadline_buffer().unwrap_err();

        assert!(matches!(
            &error,
            BpError::Config(ConfigError::Empty { name }) if name == "DEADLINE_BUFFER_SECS"
        ));
    }

    #[test]
    fn build_agent_timeout_reports_invalid_values() {
        let config = EnvVarConfig::from_map([(BUILD_AGENT_TIMEOUT_SECS, "not a number")]);

        let error = config.build_agent_timeout().unwrap_err();

        assert!(matches!(
            &error,
            BpError::Config(ConfigError::Invalid { name, .. })
                if name == BUILD_AGENT_TIMEOUT_SECS
        ));
    }

    #[test]
    fn s3_source_archive_and_artifact_limits_are_independent() {
        let config = EnvVarConfig::from_map([
            ("S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES", "1024"),
            ("S3_MAX_ARTIFACT_SIZE_BYTES", "2048"),
        ]);

        assert_eq!(config.s3_max_source_archive_size_bytes().unwrap(), 1024);
        assert_eq!(config.s3_max_artifact_size_bytes().unwrap(), 2048);
    }
}
