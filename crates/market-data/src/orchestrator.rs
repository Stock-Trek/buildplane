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
use buildplane_errors::{BpError, BpResult, DataError, TimeoutError};
use buildplane_time::deadline::Deadline;
use chrono::{DateTime, Utc};
use tracing::{error, info, warn};

pub struct Orchestrator;

impl Orchestrator {
    /// Acquire the global lock and execute one explicitly selected job.
    pub async fn run_job(
        env_config: &EnvConfig,
        job: &Job,
        deadline: Deadline,
    ) -> BpResult<JobOutcome> {
        job.validate()?;
        info!("get s3 and transfer clients");
        let (s3_client, transfer_client) = S3::clients().await;
        let bucket = &env_config.s3_bucket;
        let lock_key = ObjectKey::new(LOCK_KEY);
        let mut lock =
            match LockGuard::acquire_with_deadline(&s3_client, bucket, &lock_key, deadline).await {
                Ok(lock) => lock,
                Err(error) if matches!(job, Job::Update) && error.is_skippable() => {
                    warn!("skipping update because another invocation holds the lock: {error}");
                    return Ok(JobOutcome::default());
                }
                Err(error) => return Err(error),
            };
        let result = match deadline.remaining() {
            Ok(remaining) => {
                tokio::select! {
                    biased;
                    _ = lock.wait_until_lost() => {
                        warn!("lock {} was lost; cancelling job", lock_key);
                        Err(BpError::Data(DataError::LockLost {
                            key: lock_key.as_str().to_string(),
                        }))
                    }
                    result = tokio::time::timeout(
                        remaining,
                        Self::process_job(
                            &s3_client,
                            &transfer_client,
                            bucket,
                            job,
                            env_config.backfill_start,
                            deadline,
                        ),
                    ) => match result {
                        Ok(result) => result,
                        Err(_) => Err(BpError::Timeout(TimeoutError::DeadlineExceeded)),
                    },
                }
            }
            Err(error) => Err(error),
        };
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
        deadline: Deadline,
    ) -> BpResult<JobOutcome> {
        match job {
            Job::Update => {
                Self::process_scheduled(
                    s3_client,
                    transfer_client,
                    bucket,
                    scheduled_backfill_start,
                    deadline,
                )
                .await
            }
            Job::Backfill(backfill) => {
                info!("running targeted backfill for {:?}", backfill.pair);
                let updater = KlinesUpdater::try_new()?;
                let windows_ingested = updater
                    .backfill(s3_client, transfer_client, bucket, backfill, deadline)
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
                Compactor::prefix(s3_client, transfer_client, bucket, &prefix, deadline).await?;
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
        deadline: Deadline,
    ) -> BpResult<JobOutcome> {
        let updater = KlinesUpdater::try_new()?;
        let update_result = updater
            .update(s3_client, transfer_client, bucket, backfill_start, deadline)
            .await;
        if let Err(error) = &update_result {
            error!("update failed: {error}");
        }
        if deadline.remaining().is_err() {
            return update_result.map(|update| JobOutcome {
                windows_ingested: update.windows_ingested,
                // Compaction did not run, so the scheduled job is incomplete
                // even if every update window was processed.
                windows_skipped: update.windows_skipped.max(1),
                compaction_targets: 0,
            });
        }
        let compact_result = Self::compact_all(s3_client, transfer_client, bucket, deadline).await;
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
        deadline: Deadline,
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
            if let Err(error) = deadline.remaining() {
                warn!("stopping compaction before deadline: {error}");
                errors.push(error);
                break;
            }
            if let Err(error) =
                Compactor::prefix(s3_client, transfer_client, bucket, &prefix, deadline).await
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
