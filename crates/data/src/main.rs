#![recursion_limit = "256"]

use buildplane_config::env_vars::EnvVarConfig as SharedEnvVarConfig;
use buildplane_config::tracing_setup::TracingSetup;
use buildplane_data::{env_config::EnvConfig, job::Job, orchestrator::Orchestrator};
use buildplane_time::deadline::Deadline;
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use serde_json::{Value, json};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let shared_config = SharedEnvVarConfig::from_env();
    TracingSetup::setup(&shared_config)?;
    info!("set up tracing");
    run(service_fn(move |event| {
        let shared_config = shared_config.clone();
        async move { function_handler(event, shared_config).await }
    }))
    .await
}

async fn function_handler(
    event: LambdaEvent<Value>,
    shared_config: SharedEnvVarConfig,
) -> Result<Value, Error> {
    let env_config = EnvConfig::from_shared(&shared_config)?;
    let deadline = Deadline::new(event.context.deadline(), env_config.deadline_buffer);
    let job = Job::from_value(event.payload)?;
    info!(
        "running lambda function {} with {} job",
        env_config.lambda_function_name,
        job.mode()
    );
    let outcome = Orchestrator::run_job(&env_config, &job, deadline).await?;
    let status = if outcome.windows_skipped == 0 {
        "ok"
    } else {
        "incomplete"
    };
    Ok(json!({
        "status": status,
        "mode": job.mode(),
        "windows_ingested": outcome.windows_ingested,
        "windows_skipped": outcome.windows_skipped,
        "compaction_targets": outcome.compaction_targets,
    }))
}
