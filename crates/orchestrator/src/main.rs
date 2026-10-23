use crate::{context::RsOrchestratorContext, sqs_handler::SqsHandler};
use aws_lambda_events::event::sqs::SqsEvent;
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_config::tracing_setup::TracingSetup;
use buildplane_errors::errors::{BpError, BpResult};
use buildplane_metrics::metrics::descriptions::DescriptionsMetrics;
use lambda_runtime::{LambdaEvent, service_fn};
use metrics_cloudwatch_embedded::lambda::service::run;
use std::sync::Arc;

mod aws_ops;
mod context;
mod sqs_handler;
mod tasks;

const DEFAULT_FUNCTION_NAME: &str = "buildplane-orchestrator";

#[tokio::main]
async fn main() -> BpResult<()> {
    let config = EnvVarConfig::from_env();
    TracingSetup::setup(&config)?;
    let recorder = metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace("buildplane/orchestrator")
        .with_dimension(
            "function",
            config.lambda_function_name(DEFAULT_FUNCTION_NAME)?,
        )
        .lambda_cold_start_metric(buildplane_metrics::metrics::descriptions::COLD_STARTS_TOTAL)
        .init()?;
    DescriptionsMetrics::describe();
    let context = Arc::new(RsOrchestratorContext::new(&config).await?);
    let handler = {
        move |event: LambdaEvent<SqsEvent>| {
            let context = context.clone();
            async move { SqsHandler.handle(&context, event).await }
        }
    };
    run(recorder, service_fn(handler))
        .await
        .map_err(BpError::external)
}
