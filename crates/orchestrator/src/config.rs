use crate::aws_ops::s3::S3Config;
use buildplane_config::{env_vars::EnvVars, git::DirectoryLimits};
use buildplane_errors::{BpError, BpResult};
use buildplane_github::{app_config::GitHubAppConfig, host_allow_list::HostAllowlist};
use buildplane_time::timeouts::Timeouts;
use std::{num::NonZeroUsize, time::Duration};

const AWS_CONNECT_TIMEOUT_SECS: &str = "AWS_CONNECT_TIMEOUT_SECS";
const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";
const AWS_OPERATION_TIMEOUT_SECS: &str = "AWS_OPERATION_TIMEOUT_SECS";
const BUILD_AGENT_FUNCTION_NAME: &str = "BUILD_AGENT_FUNCTION_NAME";
const DEADLINE_BUFFER_SECS: &str = "DEADLINE_BUFFER_SECS";
const GITHUB_APP_ID: &str = "GITHUB_APP_ID";
const GITHUB_APP_PRIVATE_KEY: &str = "GITHUB_APP_PRIVATE_KEY";
const GITHUB_CONNECT_TIMEOUT_SECS: &str = "GITHUB_CONNECT_TIMEOUT_SECS";
const GITHUB_ENTERPRISE_ALLOWED_HOSTS: &str = "GITHUB_ENTERPRISE_ALLOWED_HOSTS";
const GITHUB_MAX_CACHED_INSTALLATIONS: &str = "GITHUB_MAX_CACHED_INSTALLATIONS";
const GITHUB_OPERATION_TIMEOUT_SECS: &str = "GITHUB_OPERATION_TIMEOUT_SECS";
const GIT_MAX_DIRECTORY_BYTES: &str = "GIT_MAX_DIRECTORY_BYTES";
const GIT_MAX_DIRECTORY_DEPTH: &str = "GIT_MAX_DIRECTORY_DEPTH";
const GIT_MAX_DIRECTORY_FILES: &str = "GIT_MAX_DIRECTORY_FILES";
const RUST_LOG: &str = "RUST_LOG";
const S3_BUCKET_USER_ARTIFACTS: &str = "S3_BUCKET_USER_ARTIFACTS";
const S3_MAX_ARTIFACT_SIZE_BYTES: &str = "S3_MAX_ARTIFACT_SIZE_BYTES";
const S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES: &str = "S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES";
const S3_SIGNED_URL_EXPIRY_SECS: &str = "S3_SIGNED_URL_EXPIRY_SECS";
const SQS_DEDUP_MAX_MESSAGES: &str = "SQS_DEDUP_MAX_MESSAGES";
const SQS_DEDUP_TTL_SECS: &str = "SQS_DEDUP_TTL_SECS";
const SQS_PROCESS_CONCURRENCY: &str = "SQS_PROCESS_CONCURRENCY";
const SQS_QUEUE_URL: &str = "SQS_QUEUE_URL";

const MAX_SIGNED_URL_EXPIRY: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct GitHubConfig {
    pub app_id: u64,
    pub private_key: String,
    pub timeouts: Timeouts,
    pub allowed_hosts: HostAllowlist,
    pub max_cached_installations: NonZeroUsize,
    pub directory_limits: DirectoryLimits,
}

/// Configuration required by the orchestrator process.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub log_filter: String,
    pub lambda_function_name: String,
    pub deadline_buffer: Duration,
    pub aws_timeouts: Timeouts,
    pub github: GitHubConfig,
    pub sqs_queue_url: String,
    pub sqs_dedup_max_messages: NonZeroUsize,
    pub sqs_dedup_ttl: Duration,
    pub sqs_process_concurrency: NonZeroUsize,
    pub s3: S3Config,
    pub build_agent_function_name: String,
}

impl OrchestratorConfig {
    pub fn from_env() -> BpResult<Self> {
        Self::from_vars(&EnvVars::from_env())
    }

    pub fn from_vars(vars: &EnvVars) -> BpResult<Self> {
        let aws_timeouts = Timeouts::from_durations(
            vars.required_duration_secs(AWS_CONNECT_TIMEOUT_SECS)?,
            vars.required_duration_secs(AWS_OPERATION_TIMEOUT_SECS)?,
        );
        let github_timeouts = Timeouts::from_durations(
            vars.required_duration_secs(GITHUB_CONNECT_TIMEOUT_SECS)?,
            vars.required_duration_secs(GITHUB_OPERATION_TIMEOUT_SECS)?,
        );
        let max_cached_installations =
            vars.required_nonzero_usize(GITHUB_MAX_CACHED_INSTALLATIONS)?;
        let directory_limits = DirectoryLimits {
            max_depth: vars.required_nonzero_usize(GIT_MAX_DIRECTORY_DEPTH)?.get(),
            max_files: vars.required_nonzero_usize(GIT_MAX_DIRECTORY_FILES)?.get(),
            max_total_bytes: vars.required_nonzero_u64(GIT_MAX_DIRECTORY_BYTES)?.get(),
        };
        let allowed_hosts =
            HostAllowlist::from_hosts(vars.required_csv(GITHUB_ENTERPRISE_ALLOWED_HOSTS)?)?;

        let signed_url_expiry = vars.required_duration_secs(S3_SIGNED_URL_EXPIRY_SECS)?;
        if signed_url_expiry.is_zero() || signed_url_expiry > MAX_SIGNED_URL_EXPIRY {
            return Err(BpError::config_invalid(
                S3_SIGNED_URL_EXPIRY_SECS,
                format!(
                    "must be between 1 and {} seconds (7 days)",
                    MAX_SIGNED_URL_EXPIRY.as_secs()
                ),
            ));
        }
        Ok(Self {
            log_filter: vars.required_log_filter(RUST_LOG)?,
            lambda_function_name: vars.required(AWS_LAMBDA_FUNCTION_NAME)?,
            deadline_buffer: vars.required_duration_secs(DEADLINE_BUFFER_SECS)?,
            aws_timeouts,
            github: GitHubConfig {
                app_id: vars.required_parsed(GITHUB_APP_ID)?,
                private_key: vars.required(GITHUB_APP_PRIVATE_KEY)?.replace("\\n", "\n"),
                timeouts: github_timeouts,
                allowed_hosts,
                max_cached_installations,
                directory_limits,
            },
            sqs_queue_url: vars.required(SQS_QUEUE_URL)?,
            sqs_dedup_max_messages: vars.required_nonzero_usize(SQS_DEDUP_MAX_MESSAGES)?,
            sqs_dedup_ttl: vars.required_duration_secs(SQS_DEDUP_TTL_SECS)?,
            sqs_process_concurrency: vars.required_nonzero_usize(SQS_PROCESS_CONCURRENCY)?,
            s3: S3Config {
                user_artifacts_bucket: vars.required(S3_BUCKET_USER_ARTIFACTS)?,
                max_source_archive_size_bytes: vars
                    .required_parsed(S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES)?,
                max_artifact_size_bytes: vars.required_parsed(S3_MAX_ARTIFACT_SIZE_BYTES)?,
                signed_url_expiry,
                endpoint_url: None,
            },
            build_agent_function_name: vars.required(BUILD_AGENT_FUNCTION_NAME)?,
        })
    }

    pub fn github_app_config(&self) -> GitHubAppConfig {
        GitHubAppConfig::new(self.github.app_id, self.github.private_key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUIRED: [(&str, &str); 23] = [
        (RUST_LOG, "info"),
        (AWS_LAMBDA_FUNCTION_NAME, "buildplane-orchestrator"),
        (DEADLINE_BUFFER_SECS, "5"),
        (AWS_CONNECT_TIMEOUT_SECS, "10"),
        (AWS_OPERATION_TIMEOUT_SECS, "120"),
        (GITHUB_APP_ID, "1"),
        (GITHUB_APP_PRIVATE_KEY, "private-key"),
        (GITHUB_CONNECT_TIMEOUT_SECS, "10"),
        (GITHUB_OPERATION_TIMEOUT_SECS, "120"),
        (GITHUB_ENTERPRISE_ALLOWED_HOSTS, "github.com"),
        (GITHUB_MAX_CACHED_INSTALLATIONS, "128"),
        (GIT_MAX_DIRECTORY_DEPTH, "8"),
        (GIT_MAX_DIRECTORY_FILES, "100"),
        (GIT_MAX_DIRECTORY_BYTES, "1048576"),
        (SQS_QUEUE_URL, "https://sqs.test/queue"),
        (SQS_DEDUP_MAX_MESSAGES, "4096"),
        (SQS_DEDUP_TTL_SECS, "3600"),
        (SQS_PROCESS_CONCURRENCY, "10"),
        (S3_BUCKET_USER_ARTIFACTS, "user-artifacts"),
        (S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES, "16777216"),
        (S3_MAX_ARTIFACT_SIZE_BYTES, "16777216"),
        (S3_SIGNED_URL_EXPIRY_SECS, "3600"),
        (BUILD_AGENT_FUNCTION_NAME, "buildplane-agent"),
    ];

    fn vars(values: &[(&str, &str)]) -> EnvVars {
        EnvVars::from_map(values.iter().copied())
    }

    #[test]
    fn parses_all_required_values() {
        let config = OrchestratorConfig::from_vars(&vars(&REQUIRED)).unwrap();

        assert_eq!(config.lambda_function_name, "buildplane-orchestrator");
        assert_eq!(config.s3.user_artifacts_bucket, "user-artifacts");
        assert_eq!(config.sqs_process_concurrency.get(), 10);
        assert_eq!(config.deadline_buffer, Duration::from_secs(5));
    }

    #[test]
    fn every_value_is_required() {
        for (missing, _) in REQUIRED {
            let values = REQUIRED
                .iter()
                .copied()
                .filter(|(name, _)| *name != missing)
                .collect::<Vec<_>>();

            assert!(
                OrchestratorConfig::from_vars(&vars(&values)).is_err(),
                "{missing} should be required"
            );
        }
    }

    #[test]
    fn rejects_invalid_values() {
        for (name, value) in [
            (GITHUB_APP_ID, "not a number"),
            (SQS_PROCESS_CONCURRENCY, "0"),
            (S3_SIGNED_URL_EXPIRY_SECS, "0"),
            (GITHUB_ENTERPRISE_ALLOWED_HOSTS, "not a host!"),
            (RUST_LOG, "not a valid [directive"),
        ] {
            let mut values = REQUIRED.to_vec();
            let index = values
                .iter()
                .position(|(required, _)| *required == name)
                .unwrap();
            values[index] = (name, value);

            assert!(
                OrchestratorConfig::from_vars(&vars(&values)).is_err(),
                "{name}={value} should be rejected"
            );
        }
    }
}
