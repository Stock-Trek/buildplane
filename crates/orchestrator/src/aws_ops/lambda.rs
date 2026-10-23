use crate::aws_ops::build_request::BuildRequest;
use aws_config::SdkConfig;
use aws_config::timeout::TimeoutConfig;
use aws_sdk_lambda::{Client as LambdaClient, primitives::Blob, types::InvocationType};
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::BpError;
use buildplane_errors::BpResult;
use buildplane_errors::LambdaError;
use buildplane_errors::TimeoutError;
use buildplane_messages::build_request::BuildOutcome;
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

    pub async fn run_build(
        &self,
        request: BuildRequest,
        deadline: Deadline,
    ) -> BpResult<BuildOutcome> {
        let payload = request.payload()?;
        tracing::info!(
            function = %self.config.function_name,
            "Invoking build agent Lambda"
        );
        let timeout = deadline.remaining()?;
        let mut timeout_config = TimeoutConfig::builder();
        timeout_config
            .set_connect_timeout(self.timeouts.connect.into())
            .set_operation_timeout(Some(timeout));
        let timeout_config = timeout_config.build();
        let output = tokio::time::timeout(
            timeout,
            self.client
                .invoke()
                .function_name(self.config.function_name.as_str())
                .invocation_type(InvocationType::RequestResponse)
                .payload(Blob::new(payload))
                .customize()
                .config_override(
                    aws_sdk_lambda::config::Builder::default().timeout_config(timeout_config),
                )
                .send(),
        )
        .await
        .map_err(|_| {
            BpError::Timeout(TimeoutError::Operation {
                operation: "Lambda invoke",
            })
        })?
        .map_err(|error| error.into_service_error())?;
        Self::parse_outcome(output)
    }

    fn parse_outcome(
        output: aws_sdk_lambda::operation::invoke::InvokeOutput,
    ) -> BpResult<BuildOutcome> {
        let status_code = output.status_code();
        let payload = output.payload().map(|payload| payload.as_ref());
        if let Some(function_error) = output.function_error() {
            let detail = payload
                .filter(|payload| !payload.is_empty())
                .map(|payload| String::from_utf8_lossy(payload).into_owned());
            return Err(BpError::Lambda(LambdaError::Function {
                function_error: function_error.to_string(),
                detail,
            }));
        }
        if status_code != 200 {
            return Err(BpError::Lambda(LambdaError::Status { status_code }));
        }
        let payload = payload.ok_or(BpError::Lambda(LambdaError::MissingPayload))?;
        serde_json::from_slice(payload)
            .map_err(|source| BpError::Lambda(LambdaError::InvalidPayload { source }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildplane_errors::Retryability;

    fn output(
        status_code: i32,
        payload: Option<&[u8]>,
        function_error: Option<&str>,
    ) -> aws_sdk_lambda::operation::invoke::InvokeOutput {
        let mut builder =
            aws_sdk_lambda::operation::invoke::InvokeOutput::builder().status_code(status_code);
        if let Some(payload) = payload {
            builder = builder.payload(Blob::new(payload));
        }
        if let Some(function_error) = function_error {
            builder = builder.function_error(function_error);
        }
        builder.build()
    }

    #[test]
    fn parses_successful_build_outcomes() {
        let payload = serde_json::to_vec(&BuildOutcome::success()).unwrap();

        let outcome = Lambda::parse_outcome(output(200, Some(&payload), None)).unwrap();

        assert!(outcome.is_success());
    }

    #[test]
    fn parses_permanent_build_failures() {
        let payload =
            serde_json::to_vec(&BuildOutcome::failure("build", "compile failed", false)).unwrap();

        let outcome = Lambda::parse_outcome(output(200, Some(&payload), None)).unwrap();

        assert!(outcome.is_failure());
        assert_eq!(outcome.error().unwrap().message, "compile failed");
    }

    #[test]
    fn function_errors_remain_retryable() {
        let error = Lambda::parse_outcome(output(200, None, Some("Unhandled"))).unwrap_err();

        assert_eq!(error.error_type(), "function");
        assert_eq!(error.retryability(), Retryability::Retryable);
    }

    #[test]
    fn rejects_missing_or_invalid_outcomes() {
        let missing = Lambda::parse_outcome(output(200, None, None)).unwrap_err();
        let invalid = Lambda::parse_outcome(output(200, Some(b"not json"), None)).unwrap_err();
        let status = Lambda::parse_outcome(output(500, None, None)).unwrap_err();

        assert_eq!(missing.error_type(), "missing_payload");
        assert_eq!(invalid.error_type(), "invalid_payload");
        assert_eq!(status.error_type(), "status");
    }
}
