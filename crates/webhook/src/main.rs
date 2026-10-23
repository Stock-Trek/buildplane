use crate::{context::RsWebhookContext, webhook_handler::WebhookHandler};
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_config::tracing_setup::TracingSetup;
use buildplane_errors::{BpError, BpResult};
use buildplane_metrics::descriptions::DescriptionsMetrics;
use lambda_http::{Request, service_fn};
use metrics_cloudwatch_embedded::lambda::service::run_http;
use std::sync::Arc;

mod aws;
mod context;
mod dedup;
mod dto;
mod event_response;
mod infisical;
mod responses;
mod webhook_handler;

const DEFAULT_FUNCTION_NAME: &str = "buildplane-webhook";

#[tokio::main]
async fn main() -> BpResult<()> {
    let config = EnvVarConfig::from_env();
    TracingSetup::setup(&config)?;
    let recorder = metrics_cloudwatch_embedded::Builder::new()
        .cloudwatch_namespace("buildplane/webhook")
        .with_dimension(
            "function",
            config.lambda_function_name(DEFAULT_FUNCTION_NAME)?,
        )
        .lambda_cold_start_metric(buildplane_metrics::descriptions::COLD_STARTS_TOTAL)
        .init()?;
    DescriptionsMetrics::describe();
    let context = Arc::new(RsWebhookContext::new(&config).await?);
    let handler = {
        move |event: Request| {
            let context = context.clone();
            async move { WebhookHandler.handle(&context, event).await }
        }
    };
    run_http(recorder, service_fn(handler))
        .await
        .map_err(BpError::external)
}
