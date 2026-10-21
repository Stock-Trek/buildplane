#![recursion_limit = "256"]

use buildplane_data::{env_config::EnvConfig, job::Job, orchestrator::Orchestrator};
use buildplane_shared::{
    env_vars::EnvVarConfig as SharedEnvVarConfig, tracing_setup::TracingSetup,
};
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
    let job = Job::from_value(event.payload)?;
    info!(
        "running lambda function {} with {} job",
        env_config.lambda_function_name,
        job.mode()
    );
    let outcome = Orchestrator::run_job(&env_config, &job).await?;
    Ok(json!({
        "status": "ok",
        "mode": job.mode(),
        "windows_ingested": outcome.windows_ingested,
        "compaction_targets": outcome.compaction_targets,
    }))
}
