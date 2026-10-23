use crate::handler::Handler;
use crate::source::SourceLimits;
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_config::tracing_setup::TracingSetup;
use buildplane_errors::{BpError, BpResult};
use buildplane_messages::build_request::{BuildOutcome, BuildRequest};
use buildplane_metrics::build::BuildMetrics;
use buildplane_metrics::descriptions::DescriptionsMetrics;
use lambda_runtime::{LambdaEvent, service_fn};
use metrics_cloudwatch_embedded::lambda::service::run;
use std::time::Instant;

mod artifacts;
mod handler;
mod s3;
mod source;

const DEFAULT_FUNCTION_NAME: &str = "buildplane-agent";

#[tokio::main]
async fn main() -> BpResult<()> {
    let config = EnvVarConfig::from_env();
    TracingSetup::setup(&config)?;
    let recorder = metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace("buildplane/agent")
        .with_dimension(
            "function",
            config.lambda_function_name(DEFAULT_FUNCTION_NAME)?,
        )
        .lambda_cold_start_metric(buildplane_metrics::descriptions::COLD_STARTS_TOTAL)
        .init()?;
    DescriptionsMetrics::describe();
    let source_limits = SourceLimits::new(
        config.build_agent_source_download_timeout()?,
        config.s3_max_source_archive_size_bytes()?,
        config.build_agent_max_extracted_source_size_bytes()?.get(),
    );
    let handler = Handler::with_source_limits(
        config.build_agent_timeout()?,
        config.build_agent_source_dir()?,
        config.build_agent_build_cache_dir()?,
        source_limits,
    );
    run(
        recorder,
        service_fn(move |event| {
            let handler = handler.clone();
            async move { BuildAgent::handle(handler, event).await }
        }),
    )
    .await
    .map_err(BpError::external)
}

struct BuildAgent;

impl BuildAgent {
    async fn handle(handler: Handler, event: LambdaEvent<BuildRequest>) -> BpResult<BuildOutcome> {
        let started_at = Instant::now();
        match handler.handle(event.payload).await {
            Ok(()) => {
                BuildMetrics::record_success(started_at.elapsed());
                Ok(BuildOutcome::success())
            }
            Err(error) => Self::handle_error(error, started_at),
        }
    }

    fn handle_error(error: BpError, started_at: Instant) -> BpResult<BuildOutcome> {
        let duration = started_at.elapsed();
        if error.retryability().is_retryable() {
            BuildMetrics::record_retryable_failure(error.error_type(), duration);
            tracing::error!(
                error_type = error.error_type(),
                %error,
                "build failed with a retryable error"
            );
            // Returning Err lets the Lambda asynchronous invocation retry the
            // build, which is only appropriate for transient failures.
            return Err(error);
        }

        BuildMetrics::record_permanent_failure(error.error_type(), duration);
        tracing::error!(
            error_type = error.error_type(),
            %error,
            "build failed permanently"
        );
        // A permanent failure, such as a user compile error, cannot be fixed by
        // retrying. Return Ok with a structured failure payload so Lambda does
        // not retry it and the outcome is available to callers and logs.
        Ok(BuildOutcome::failure(
            error.error_type(),
            error.to_string(),
            false,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buildplane_errors::{ConfigError, TimeoutError};
    use std::time::Duration;

    #[test]
    fn permanent_build_errors_are_returned_as_structured_outcomes() {
        let error = BpError::Config(ConfigError::Build("compile failed".to_string()));

        let outcome =
            BuildAgent::handle_error(error, Instant::now() - Duration::from_secs(1)).unwrap();

        assert!(outcome.is_failure());
        let error = outcome.error().unwrap();
        assert_eq!(error.error_type, "build");
        assert!(error.message.contains("compile failed"));
        assert!(!error.retryable);
    }

    #[test]
    fn retryable_build_errors_are_returned_so_lambda_retries() {
        let error = BpError::Timeout(TimeoutError::BuildTimeout(Duration::from_secs(300)));

        let error =
            BuildAgent::handle_error(error, Instant::now() - Duration::from_secs(1)).unwrap_err();

        assert_eq!(error.error_type(), "build_timeout");
        assert!(error.retryability().is_retryable());
    }
}
