use crate::aws_ops::build_request::BuildRequest;
use aws_config::SdkConfig;
use aws_sdk_lambda::{Client as LambdaClient, primitives::Blob, types::InvocationType};
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::errors::BpError;
use buildplane_errors::errors::BpResult;
use buildplane_errors::errors::TimeoutError;
use buildplane_time::deadline::Deadline;
use buildplane_time::timeouts::Timeouts;

pub struct LambdaConfig {
    pub function_name: String,
}

impl LambdaConfig {
    pub fn from_env(config: &EnvVarConfig) -> BpResult<Self> {
        Ok(Self {
            function_name: config.build_agent_function_name()?,
        })
    }
}

pub struct Lambda {
    client: LambdaClient,
    config: LambdaConfig,
    timeouts: Timeouts,
}

impl Lambda {
    pub fn new(client: LambdaClient, timeouts: Timeouts, config: LambdaConfig) -> Self {
        Self {
            client,
            config,
            timeouts,
        }
    }

    pub fn from_env(
        sdk_config: &SdkConfig,
        timeouts: Timeouts,
        config: &EnvVarConfig,
    ) -> BpResult<Self> {
        Ok(Self::new(
            LambdaClient::new(sdk_config),
            timeouts,
            LambdaConfig::from_env(config)?,
        ))
    }

    pub async fn run_build(&self, request: BuildRequest, deadline: Deadline) -> BpResult<()> {
        let payload = request.payload()?;
        tracing::info!(
            function = %self.config.function_name,
            "Invoking build agent Lambda"
        );
        let timeout = self.timeouts.operation_timeout(deadline)?;
        tokio::time::timeout(
            timeout,
            self.client
                .invoke()
                .function_name(self.config.function_name.as_str())
                .invocation_type(InvocationType::Event)
                .payload(Blob::new(payload))
                .send(),
        )
        .await
        .map_err(|_| {
            BpError::Timeout(TimeoutError::Operation {
                operation: "Lambda invoke",
            })
        })?
        .map_err(|error| error.into_service_error())?;
        Ok(())
    }
}
