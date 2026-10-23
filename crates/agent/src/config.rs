use buildplane_config::env_vars::EnvVars;
use buildplane_errors::{BpError, BpResult};
use std::{num::NonZeroU64, path::PathBuf, time::Duration};

const AWS_LAMBDA_FUNCTION_NAME: &str = "AWS_LAMBDA_FUNCTION_NAME";
const BUILD_AGENT_BUILD_CACHE_DIR: &str = "BUILD_AGENT_BUILD_CACHE_DIR";
const BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES: &str =
    "BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES";
const BUILD_AGENT_SOURCE_DIR: &str = "BUILD_AGENT_SOURCE_DIR";
const BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS: &str = "BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS";
const BUILD_AGENT_TIMEOUT_SECS: &str = "BUILD_AGENT_TIMEOUT_SECS";
const RUST_LOG: &str = "RUST_LOG";
const S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES: &str = "S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES";

/// Configuration required by the build agent process.
///
/// Every value is parsed and validated before the Lambda handler is installed,
/// so a missing or invalid environment variable fails startup instead of the
/// first build.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub log_filter: String,
    pub lambda_function_name: String,
    pub build_timeout: Duration,
    pub source_dir: PathBuf,
    pub build_cache_dir: PathBuf,
    pub source_download_timeout: Duration,
    pub max_source_archive_size_bytes: u64,
    pub max_extracted_source_size_bytes: NonZeroU64,
}

impl AgentConfig {
    pub fn from_env() -> BpResult<Self> {
        Self::from_vars(&EnvVars::from_env())
    }

    pub fn from_vars(vars: &EnvVars) -> BpResult<Self> {
        let build_timeout = vars.required_duration_secs(BUILD_AGENT_TIMEOUT_SECS)?;
        if build_timeout.is_zero() {
            return Err(BpError::config_invalid(
                BUILD_AGENT_TIMEOUT_SECS,
                "must be greater than zero",
            ));
        }
        let source_download_timeout =
            vars.required_duration_secs(BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS)?;
        if source_download_timeout.is_zero() {
            return Err(BpError::config_invalid(
                BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS,
                "must be greater than zero",
            ));
        }

        Ok(Self {
            log_filter: vars.required_log_filter(RUST_LOG)?,
            lambda_function_name: vars.required(AWS_LAMBDA_FUNCTION_NAME)?,
            build_timeout,
            source_dir: vars.required_path(BUILD_AGENT_SOURCE_DIR)?,
            build_cache_dir: vars.required_path(BUILD_AGENT_BUILD_CACHE_DIR)?,
            source_download_timeout,
            max_source_archive_size_bytes: vars
                .required_parsed(S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES)?,
            max_extracted_source_size_bytes: vars
                .required_nonzero_u64(BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUIRED: [(&str, &str); 8] = [
        (RUST_LOG, "info"),
        (AWS_LAMBDA_FUNCTION_NAME, "buildplane-agent"),
        (BUILD_AGENT_TIMEOUT_SECS, "300"),
        (BUILD_AGENT_SOURCE_DIR, "/var/task"),
        (BUILD_AGENT_BUILD_CACHE_DIR, "/tmp/buildplane-build-cache"),
        (BUILD_AGENT_SOURCE_DOWNLOAD_TIMEOUT_SECS, "60"),
        (BUILD_AGENT_MAX_EXTRACTED_SOURCE_SIZE_BYTES, "67108864"),
        (S3_MAX_SOURCE_ARCHIVE_SIZE_BYTES, "16777216"),
    ];

    fn vars(values: &[(&str, &str)]) -> EnvVars {
        EnvVars::from_map(values.iter().copied())
    }

    #[test]
    fn parses_all_required_values() {
        let config = AgentConfig::from_vars(&vars(&REQUIRED)).unwrap();

        assert_eq!(config.lambda_function_name, "buildplane-agent");
        assert_eq!(config.build_timeout, Duration::from_secs(300));
        assert_eq!(config.source_dir, PathBuf::from("/var/task"));
        assert_eq!(config.max_source_archive_size_bytes, 16 * 1024 * 1024);
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
                AgentConfig::from_vars(&vars(&values)).is_err(),
                "{missing} should be required"
            );
        }
    }

    #[test]
    fn rejects_invalid_values() {
        let mut values = REQUIRED.to_vec();
        values[2] = (BUILD_AGENT_TIMEOUT_SECS, "not a number");
        assert!(AgentConfig::from_vars(&vars(&values)).is_err());

        let mut values = REQUIRED.to_vec();
        values[2] = (BUILD_AGENT_TIMEOUT_SECS, "0");
        assert!(AgentConfig::from_vars(&vars(&values)).is_err());

        let mut values = REQUIRED.to_vec();
        values[0] = (RUST_LOG, "not a valid [directive");
        assert!(AgentConfig::from_vars(&vars(&values)).is_err());
    }
}
