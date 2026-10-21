use crate::errors::{
    config::RsConfigError,
    error::{RsSharedError, RsSharedResult},
};
use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    num::{NonZeroU64, NonZeroUsize},
    time::Duration,
};
use url::Url;

const AWS_ENDPOINT_URL_S3: &str = "AWS_ENDPOINT_URL_S3";
const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";
const BACKFILL_START: &str = "BACKFILL_START";
const BUILD_AGENT_CLUSTER_ARN: &str = "BUILD_AGENT_CLUSTER_ARN";
const BUILD_AGENT_CONTAINER_NAME: &str = "BUILD_AGENT_CONTAINER_NAME";
const BUILD_AGENT_TASK_DEFINITION_ARN: &str = "BUILD_AGENT_TASK_DEFINITION_ARN";
const DEADLINE_BUFFER: &str = "DEADLINE_BUFFER";
const ENTERPRISE_ALLOWED_HOSTS: &str = "GITHUB_ENTERPRISE_ALLOWED_HOSTS";
const GITHUB_APP_ID: &str = "GITHUB_APP_ID";
const GITHUB_APP_PRIVATE_KEY: &str = "GITHUB_APP_PRIVATE_KEY";
const GITHUB_MAX_CACHED_INSTALLATIONS: &str = "GITHUB_MAX_CACHED_INSTALLATIONS";
const GITHUB_MAX_DIRECTORY_BYTES: &str = "GITHUB_MAX_DIRECTORY_BYTES";
const GITHUB_MAX_DIRECTORY_DEPTH: &str = "GITHUB_MAX_DIRECTORY_DEPTH";
const GITHUB_MAX_DIRECTORY_FILES: &str = "GITHUB_MAX_DIRECTORY_FILES";
const INFISICAL_BASE_URL: &str = "INFISICAL_BASE_URL";
const INFISICAL_ENVIRONMENT: &str = "INFISICAL_ENVIRONMENT";
const INFISICAL_IDENTITY_ID: &str = "INFISICAL_IDENTITY_ID";
const INFISICAL_PROJECT_ID: &str = "INFISICAL_PROJECT_ID";
const RUST_LOG: &str = "RUST_LOG";
const S3_BUCKET: &str = "S3_BUCKET";
const S3_BUCKET_USER_ARTIFACTS: &str = "S3_BUCKET_USER_ARTIFACTS";
const S3_MAX_OBJECT_SIZE_BYTES: &str = "S3_MAX_OBJECT_SIZE_BYTES";
pub const S3_SIGNED_URL_EXPIRY: &str = "S3_SIGNED_URL_EXPIRY";
const SQS_PROCESS_CONCURRENCY: &str = "SQS_PROCESS_CONCURRENCY";
const SQS_QUEUE_URL: &str = "SQS_QUEUE_URL";
const WEBHOOK_COLD_START_REFRESH_TIMEOUT: &str = "WEBHOOK_COLD_START_REFRESH_TIMEOUT";
const WEBHOOK_DEDUP_MAX_DELIVERIES: &str = "WEBHOOK_DEDUP_MAX_DELIVERIES";
const WEBHOOK_DEDUP_TTL: &str = "WEBHOOK_DEDUP_TTL";
const WEBHOOK_REFRESH_COOLDOWN: &str = "WEBHOOK_REFRESH_COOLDOWN";
const WEBHOOK_SECRET_FAILURE_BACKOFF: &str = "WEBHOOK_SECRET_FAILURE_BACKOFF";

const DEFAULT_DEADLINE_BUFFER: Duration = Duration::from_secs(5);
const DEFAULT_GITHUB_MAX_CACHED_INSTALLATIONS: usize = 128;
const DEFAULT_GITHUB_MAX_DIRECTORY_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_GITHUB_MAX_DIRECTORY_DEPTH: usize = 32;
const DEFAULT_GITHUB_MAX_DIRECTORY_FILES: usize = 10_000;
const DEFAULT_S3_MAX_OBJECT_SIZE_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_S3_SIGNED_URL_EXPIRY: Duration = Duration::from_secs(60 * 60);
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
    pub fn from_env() -> Self {
        Self {
            vars: std::env::vars_os().collect(),
        }
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

    pub fn lambda_function_name(&self, default: &str) -> RsSharedResult<String> {
        self.parsed(AWS_LAMBDA_FUNCTION_NAME, default.to_owned())
    }

    pub fn log_filter(&self) -> RsSharedResult<Option<String>> {
        self.optional(RUST_LOG)
    }

    pub fn backfill_start(&self) -> RsSharedResult<Option<DateTime<Utc>>> {
        let Some(value) = self.optional(BACKFILL_START)? else {
            return Ok(None);
        };
        Self::parse_timestamp(BACKFILL_START, value.trim()).map(Some)
    }

    pub fn deadline_buffer(&self) -> RsSharedResult<Duration> {
        self.duration(DEADLINE_BUFFER, DEFAULT_DEADLINE_BUFFER)
    }

    pub fn github_app_id(&self) -> RsSharedResult<u64> {
        self.required_parsed(GITHUB_APP_ID)
    }

    pub fn github_app_private_key(&self) -> RsSharedResult<String> {
        Ok(self.required(GITHUB_APP_PRIVATE_KEY)?.replace("\\n", "\n"))
    }

    pub fn github_enterprise_allowed_hosts(&self) -> RsSharedResult<Vec<String>> {
        self.optional_csv(ENTERPRISE_ALLOWED_HOSTS)
    }

    pub fn github_max_cached_installations(&self) -> RsSharedResult<NonZeroUsize> {
        let installations = self.parsed(
            GITHUB_MAX_CACHED_INSTALLATIONS,
            DEFAULT_GITHUB_MAX_CACHED_INSTALLATIONS,
        )?;
        NonZeroUsize::new(installations).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                GITHUB_MAX_CACHED_INSTALLATIONS,
                "must be greater than zero",
            ))
        })
    }

    pub fn github_max_directory_depth(&self) -> RsSharedResult<NonZeroUsize> {
        let depth = self.parsed(
            GITHUB_MAX_DIRECTORY_DEPTH,
            DEFAULT_GITHUB_MAX_DIRECTORY_DEPTH,
        )?;
        NonZeroUsize::new(depth).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                GITHUB_MAX_DIRECTORY_DEPTH,
                "must be greater than zero",
            ))
        })
    }

    pub fn github_max_directory_files(&self) -> RsSharedResult<NonZeroUsize> {
        let files = self.parsed(
            GITHUB_MAX_DIRECTORY_FILES,
            DEFAULT_GITHUB_MAX_DIRECTORY_FILES,
        )?;
        NonZeroUsize::new(files).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                GITHUB_MAX_DIRECTORY_FILES,
                "must be greater than zero",
            ))
        })
    }

    pub fn github_max_directory_bytes(&self) -> RsSharedResult<NonZeroU64> {
        let bytes = self.parsed(
            GITHUB_MAX_DIRECTORY_BYTES,
            DEFAULT_GITHUB_MAX_DIRECTORY_BYTES,
        )?;
        NonZeroU64::new(bytes).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                GITHUB_MAX_DIRECTORY_BYTES,
                "must be greater than zero",
            ))
        })
    }

    pub fn sqs_queue_url(&self) -> RsSharedResult<String> {
        self.required(SQS_QUEUE_URL)
    }

    pub fn sqs_process_concurrency(&self) -> RsSharedResult<NonZeroUsize> {
        let concurrency = self.parsed(SQS_PROCESS_CONCURRENCY, DEFAULT_SQS_PROCESS_CONCURRENCY)?;
        NonZeroUsize::new(concurrency).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                SQS_PROCESS_CONCURRENCY,
                "must be greater than zero",
            ))
        })
    }

    pub fn s3_bucket(&self) -> RsSharedResult<String> {
        self.required(S3_BUCKET)
    }

    pub fn s3_bucket_user_artifacts(&self) -> RsSharedResult<String> {
        self.required(S3_BUCKET_USER_ARTIFACTS)
    }

    pub fn s3_max_object_size_bytes(&self) -> RsSharedResult<u64> {
        self.parsed(S3_MAX_OBJECT_SIZE_BYTES, DEFAULT_S3_MAX_OBJECT_SIZE_BYTES)
    }

    pub fn s3_signed_url_expiry(&self) -> RsSharedResult<Duration> {
        let expiry = self.duration(S3_SIGNED_URL_EXPIRY, DEFAULT_S3_SIGNED_URL_EXPIRY)?;
        if expiry.is_zero() || expiry > MAX_SIGNED_URL_EXPIRY {
            return Err(RsSharedError::Config(RsConfigError::invalid(
                S3_SIGNED_URL_EXPIRY,
                format!(
                    "must be between 1 and {} seconds (7 days)",
                    MAX_SIGNED_URL_EXPIRY.as_secs()
                ),
            )));
        }
        Ok(expiry)
    }

    pub fn aws_endpoint_url_s3(&self) -> RsSharedResult<Option<String>> {
        self.optional(AWS_ENDPOINT_URL_S3)
    }

    pub fn build_agent_cluster_arn(&self) -> RsSharedResult<String> {
        self.required(BUILD_AGENT_CLUSTER_ARN)
    }

    pub fn build_agent_container_name(&self) -> RsSharedResult<String> {
        self.required(BUILD_AGENT_CONTAINER_NAME)
    }

    pub fn build_agent_task_definition_arn(&self) -> RsSharedResult<String> {
        self.required(BUILD_AGENT_TASK_DEFINITION_ARN)
    }

    pub fn webhook_dedup_max_deliveries(&self) -> RsSharedResult<NonZeroUsize> {
        let deliveries = self.parsed(
            WEBHOOK_DEDUP_MAX_DELIVERIES,
            DEFAULT_WEBHOOK_DEDUP_MAX_DELIVERIES,
        )?;
        NonZeroUsize::new(deliveries).ok_or_else(|| {
            RsSharedError::Config(RsConfigError::invalid(
                WEBHOOK_DEDUP_MAX_DELIVERIES,
                "must be greater than zero",
            ))
        })
    }

    pub fn webhook_dedup_ttl(&self) -> RsSharedResult<Duration> {
        self.duration(WEBHOOK_DEDUP_TTL, DEFAULT_WEBHOOK_DEDUP_TTL)
    }

    pub fn webhook_refresh_cooldown(&self) -> RsSharedResult<Duration> {
        self.duration(WEBHOOK_REFRESH_COOLDOWN, DEFAULT_WEBHOOK_REFRESH_COOLDOWN)
    }

    pub fn webhook_secret_failure_backoff(&self) -> RsSharedResult<Duration> {
        self.duration(
            WEBHOOK_SECRET_FAILURE_BACKOFF,
            DEFAULT_WEBHOOK_SECRET_FAILURE_BACKOFF,
        )
    }

    pub fn webhook_cold_start_refresh_timeout(&self) -> RsSharedResult<Duration> {
        self.duration(
            WEBHOOK_COLD_START_REFRESH_TIMEOUT,
            DEFAULT_WEBHOOK_COLD_START_REFRESH_TIMEOUT,
        )
    }

    pub fn infisical_identity_id(&self) -> RsSharedResult<String> {
        self.required(INFISICAL_IDENTITY_ID)
    }

    pub fn infisical_project_id(&self) -> RsSharedResult<String> {
        self.required(INFISICAL_PROJECT_ID)
    }

    pub fn infisical_environment(&self) -> RsSharedResult<String> {
        self.required(INFISICAL_ENVIRONMENT)
    }

    pub fn infisical_base_url(&self) -> RsSharedResult<String> {
        let base_url = self.required(INFISICAL_BASE_URL)?;
        let url = Url::parse(&base_url).map_err(|error| {
            RsSharedError::Config(RsConfigError::invalid_source(INFISICAL_BASE_URL, error))
        })?;
        if url.scheme() != "https" {
            return Err(RsSharedError::Config(RsConfigError::invalid(
                INFISICAL_BASE_URL,
                format!("must use the \"https\" scheme, got \"{}\"", url.scheme()),
            )));
        }
        Ok(base_url)
    }

    pub(crate) fn duration(&self, key: &str, default: Duration) -> RsSharedResult<Duration> {
        Ok(Duration::from_secs(self.parsed(key, default.as_secs())?))
    }

    fn required_parsed<T>(&self, key: &str) -> RsSharedResult<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        self.required(key)?
            .parse::<T>()
            .map_err(|error| RsSharedError::Config(RsConfigError::invalid_source(key, error)))
    }

    fn parsed<T>(&self, key: &str, default: T) -> RsSharedResult<T>
    where
        T: std::str::FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let Some(value) = self.raw(key)? else {
            return Ok(default);
        };
        if value.is_empty() {
            return Err(RsSharedError::Config(RsConfigError::empty(key)));
        }
        value
            .parse::<T>()
            .map_err(|error| RsSharedError::Config(RsConfigError::invalid_source(key, error)))
    }

    fn parse_timestamp(name: &str, value: &str) -> RsSharedResult<DateTime<Utc>> {
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

    fn invalid_timestamp(name: &str, value: &str) -> RsSharedError {
        RsSharedError::Config(RsConfigError::invalid(
            name,
            format!("must be an RFC 3339 timestamp or YYYY-MM-DD date, got {value}"),
        ))
    }

    fn optional_csv(&self, name: &str) -> RsSharedResult<Vec<String>> {
        match self.optional(name)? {
            Some(value) => Ok(Self::split_csv(&value)),
            None => Ok(Vec::new()),
        }
    }

    fn required(&self, name: &str) -> RsSharedResult<String> {
        match self.raw(name)? {
            Some(value) if value.is_empty() => {
                Err(RsSharedError::Config(RsConfigError::empty(name)))
            }
            Some(value) => Ok(value),
            None => Err(RsSharedError::Config(RsConfigError::missing(name))),
        }
    }

    fn optional(&self, name: &str) -> RsSharedResult<Option<String>> {
        Ok(self.raw(name)?.filter(|value| !value.is_empty()))
    }

    fn raw(&self, name: &str) -> RsSharedResult<Option<String>> {
        match self.vars.get(OsStr::new(name)) {
            Some(value) => value.clone().into_string().map(Some).map_err(|value| {
                RsSharedError::EnvVar(Box::new(std::env::VarError::NotUnicode(value)))
            }),
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
    use super::EnvVarConfig;
    use chrono::{DateTime, Utc};

    #[test]
    fn log_filter_reads_rust_log() {
        let config = EnvVarConfig::from_map([("RUST_LOG", "debug")]);
        assert_eq!(config.log_filter().unwrap().as_deref(), Some("debug"));
    }

    #[test]
    fn s3_bucket_reads_s3_bucket() {
        let config = EnvVarConfig::from_map([("S3_BUCKET", "market-data")]);
        assert_eq!(config.s3_bucket().unwrap(), "market-data");
    }

    #[test]
    fn backfill_start_is_optional() {
        let config = EnvVarConfig::from_map(std::iter::empty::<(&str, &str)>());
        assert_eq!(config.backfill_start().unwrap(), None);
    }

    #[test]
    fn backfill_start_accepts_rfc3339_timestamp() {
        let config = EnvVarConfig::from_map([("BACKFILL_START", "2024-01-02T03:04:05Z")]);
        assert_eq!(
            config.backfill_start().unwrap(),
            Some(
                DateTime::parse_from_rfc3339("2024-01-02T03:04:05Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn backfill_start_accepts_date() {
        let config = EnvVarConfig::from_map([("BACKFILL_START", "2024-01-02")]);
        assert_eq!(
            config.backfill_start().unwrap(),
            Some(
                DateTime::parse_from_rfc3339("2024-01-02T00:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn backfill_start_rejects_invalid_timestamp() {
        let config = EnvVarConfig::from_map([("BACKFILL_START", "not-a-date")]);
        assert!(config.backfill_start().is_err());
    }
}
