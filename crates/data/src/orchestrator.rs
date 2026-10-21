use crate::{
    compacter::Compactor,
    constants::LOCK_KEY,
    env_config::EnvConfig,
    error::{MduError, MduResult},
    lock::LockGuard,
    s3::{Bucket, ObjectKey, S3},
    updater::KlinesUpdater,
};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use chrono::{DateTime, Utc};
use tracing::{error, info};

pub struct Orchestrator;

impl Orchestrator {
    pub async fn run(env_config: &EnvConfig) -> MduResult<()> {
        info!("get s3 and transfer clients");
        let (s3_client, transfer_client) = S3::clients().await;
        let bucket = &env_config.s3_bucket;
        let lock_key = ObjectKey::new(LOCK_KEY);
        let mut lock = LockGuard::acquire(&s3_client, bucket, &lock_key).await?;
        let result = Self::process(
            &s3_client,
            &transfer_client,
            bucket,
            env_config.backfill_start,
        )
        .await;
        if let Err(release_error) = lock.release().await {
            error!("failed to release lock: {release_error}");
        }
        result
    }

    async fn process(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
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
}
