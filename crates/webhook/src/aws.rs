use aws_config::{BehaviorVersion, SdkConfig, timeout::TimeoutConfig};
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::errors::BpResult;
use buildplane_sqs::sqs::Sqs;
use buildplane_time::timeouts::Timeouts;
use std::time::Duration;

const DEFAULT_AWS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_AWS_OPERATION_TIMEOUT: Duration = Duration::from_secs(120);

pub struct Aws {
    pub sqs: Sqs,
}

impl Aws {
    pub fn load_timeouts(config: &EnvVarConfig) -> BpResult<Timeouts> {
        Timeouts::from_env(
            config,
            "AWS",
            DEFAULT_AWS_CONNECT_TIMEOUT,
            DEFAULT_AWS_OPERATION_TIMEOUT,
        )
    }
    pub async fn load_sdk_config(timeouts: Timeouts) -> SdkConfig {
        let mut timeout_config = TimeoutConfig::builder();
        timeout_config
            .set_connect_timeout(timeouts.connect.into())
            .set_operation_timeout(timeouts.operation.into());
        let timeout_config = timeout_config.build();
        aws_config::defaults(BehaviorVersion::latest())
            .timeout_config(timeout_config)
            .load()
            .await
    }

    pub fn new(sqs: Sqs) -> Self {
        Self { sqs }
    }

    pub fn from_env(
        sdk_config: &SdkConfig,
        timeouts: Timeouts,
        config: &EnvVarConfig,
    ) -> BpResult<Self> {
        Ok(Self::new(Sqs::from_env(sdk_config, timeouts, config)?))
    }
}
