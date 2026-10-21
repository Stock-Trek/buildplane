use crate::{
    compacter::Compactor,
    constants::LOCK_KEY,
    env_config::EnvConfig,
    error::{MduError, MduResult},
    lock::LockGuard,
    s3::{Bucket, ObjectKey, S3},
    updater::KlinesUpdater,
};
use buildplane_shared::{
    env_vars::EnvVarConfig as SharedEnvVarConfig, tracing_setup::TracingSetup,
};
use chrono::{DateTime, Utc};
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use serde_json::{Value, json};
use tracing::{error, info};

mod compacter;
mod constants;
mod env_config;
mod error;
mod lock;
mod parquet_reader;
mod parquet_writer;
mod quantized_time;
mod s3;
mod subfolders;
mod updater;
mod updaters;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let shared_config = SharedEnvVarConfig::from_env();
    setup_tracing(&shared_config)?;
    run(service_fn(move |event| {
        let shared_config = shared_config.clone();
        async move { function_handler(event, shared_config).await }
    }))
    .await
}

fn setup_tracing(config: &SharedEnvVarConfig) -> MduResult<()> {
    TracingSetup::setup(config)?;
    info!("set up tracing");
    Ok(())
}

async fn function_handler(
    _event: LambdaEvent<Value>,
    shared_config: SharedEnvVarConfig,
) -> Result<Value, Error> {
    let env_config = EnvConfig::from_shared(&shared_config)?;
    info!(
        "running lambda function {}",
        env_config.lambda_function_name
    );
    info!("get s3 and transfer clients");
    let (s3_client, transfer_client) = S3::clients().await;
    let bucket = &env_config.s3_bucket;
    let lock_key = ObjectKey::new(LOCK_KEY);
    let mut lock = LockGuard::acquire(&s3_client, bucket, &lock_key).await?;
    let result = process(
        &s3_client,
        &transfer_client,
        bucket,
        env_config.backfill_start,
    )
    .await;
    if let Err(release_error) = lock.release().await {
        error!("failed to release lock: {release_error}");
    }
    result?;
    Ok(json!({ "status": "ok" }))
}

async fn process(
    s3_client: &aws_sdk_s3::Client,
    transfer_client: &aws_sdk_s3_transfer_manager::Client,
    bucket: &Bucket,
    backfill_start: Option<DateTime<Utc>>,
) -> MduResult<()> {
    let updater = KlinesUpdater::try_new()?;
    let update_result = updater
        .update(s3_client, transfer_client, bucket, backfill_start)
        .await;
    if let Err(error) = &update_result {
        error!("update failed: {error}");
    }
    let compact_result = Compactor::all(s3_client, transfer_client, bucket).await;
    if let Err(error) = &compact_result {
        error!("compaction failed: {error}");
    }
    match (update_result, compact_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(update_error), Err(compact_error)) => {
            Err(MduError::multiple([update_error, compact_error]))
        }
    }
}
