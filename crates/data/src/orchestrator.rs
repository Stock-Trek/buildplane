use crate::{
    compacter::Compactor,
    constants::{LOCK_KEY, common_trading_pairs},
    env_config::EnvConfig,
    job::{Job, JobOutcome},
    lock::LockGuard,
    s3::{Bucket, ObjectKey, S3},
    updater::KlinesUpdater,
};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use buildplane_errors::{BpError, BpResult};
use chrono::{DateTime, Utc};
use tracing::{error, info, warn};

pub struct Orchestrator;

impl Orchestrator {
    /// Run the default scheduled update and compaction.
    ///
    /// This keeps the previous entry point available for callers that do not
    /// need to select a job explicitly.
    pub async fn run(env_config: &EnvConfig) -> BpResult<()> {
        Self::run_job(env_config, &Job::Update).await.map(|_| ())
    }

    /// Acquire the global lock and execute one explicitly selected job.
    pub async fn run_job(env_config: &EnvConfig, job: &Job) -> BpResult<JobOutcome> {
        job.validate()?;
        info!("get s3 and transfer clients");
        let (s3_client, transfer_client) = S3::clients().await;
        let bucket = &env_config.s3_bucket;
        let lock_key = ObjectKey::new(LOCK_KEY);
        let mut lock = match LockGuard::acquire(&s3_client, bucket, &lock_key).await {
            Ok(lock) => lock,
            Err(error) if matches!(job, Job::Update) && error.is_skippable() => {
                warn!("skipping update because another invocation holds the lock: {error}");
                return Ok(JobOutcome::default());
            }
            Err(error) => return Err(error),
        };
        let result = Self::process_job(
            &s3_client,
            &transfer_client,
            bucket,
            job,
            env_config.backfill_start,
        )
        .await;
        if let Err(release_error) = lock.release().await {
            error!("failed to release lock: {release_error}");
        }
        result
    }

    async fn process_job(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        job: &Job,
        scheduled_backfill_start: Option<DateTime<Utc>>,
    ) -> BpResult<JobOutcome> {
        match job {
            Job::Update => {
                Self::process_scheduled(
                    s3_client,
                    transfer_client,
                    bucket,
                    scheduled_backfill_start,
                )
                .await
            }
            Job::Backfill(backfill) => {
                info!("running targeted backfill for {:?}", backfill.pair);
                let updater = KlinesUpdater::try_new()?;
                let windows_ingested = updater
                    .backfill(s3_client, transfer_client, bucket, backfill)
                    .await?;
                Ok(JobOutcome {
                    windows_ingested: windows_ingested.windows_ingested,
                    windows_skipped: windows_ingested.windows_skipped,
                    compaction_targets: 0,
                })
            }
            Job::Compact(compact) => {
                let prefix = compact.prefix()?;
                info!("running targeted compaction for {}", prefix);
                Compactor::prefix(s3_client, transfer_client, bucket, &prefix).await?;
                Ok(JobOutcome {
                    windows_ingested: 0,
                    windows_skipped: 0,
                    compaction_targets: 1,
                })
            }
        }
    }

    async fn process_scheduled(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        backfill_start: Option<DateTime<Utc>>,
    ) -> BpResult<JobOutcome> {
        let updater = KlinesUpdater::try_new()?;
        let update_result = updater
            .update(s3_client, transfer_client, bucket, backfill_start)
            .await;
        if let Err(error) = &update_result {
            error!("update failed: {error}");
        }
        let compact_result = Self::compact_all(s3_client, transfer_client, bucket).await;
        if let Err(error) = &compact_result {
            error!("compaction failed: {error}");
        }
        match (update_result, compact_result) {
            (Ok(update), Ok(())) => Ok(JobOutcome {
                windows_ingested: update.windows_ingested,
                windows_skipped: update.windows_skipped,
                compaction_targets: 0,
            }),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(update_error), Err(compact_error)) => {
                Err(BpError::multiple([update_error, compact_error]))
            }
        }
    }

    async fn compact_all(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
    ) -> BpResult<()> {
        info!("compact all");
        let mut prefixes = common_trading_pairs()
            .into_iter()
            .flat_map(|(cex_id, trading_mode, trading_pairs)| {
                trading_pairs.into_iter().map(move |trading_pair| {
                    ObjectKey::new(format!(
                        "{}/{}/{}/{}/",
                        trading_mode, cex_id, trading_pair.base, trading_pair.quote
                    ))
                })
            })
            .collect::<Vec<_>>();
        prefixes.sort_unstable();
        prefixes.dedup();
        let mut errors = Vec::new();
        for prefix in prefixes {
            if let Err(error) = Compactor::prefix(s3_client, transfer_client, bucket, &prefix).await
            {
                warn!("failed to compact folder {}: {error}", prefix);
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(BpError::multiple(errors))
        }
    }
}
