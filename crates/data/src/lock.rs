use aws_sdk_s3::{
    Client as S3Client,
    error::{ProvideErrorMetadata, SdkError},
    primitives::ByteStream,
};
use chrono::{DateTime, Utc};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time::{Instant, sleep};
use tracing::{info, warn};

use crate::{
    error::{MduError, MduResult},
    s3::{Bucket, ObjectKey, S3},
};

const STALE_AFTER: Duration = Duration::from_secs(16 * 60);
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
static NEXT_LOCK_TOKEN: AtomicU64 = AtomicU64::new(0);

pub struct LockGuard {
    s3_client: S3Client,
    bucket: Bucket,
    key: ObjectKey,
    e_tag: String,
    released: bool,
}

impl LockGuard {
    pub async fn acquire(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
    ) -> MduResult<Self> {
        let deadline = Instant::now() + ACQUIRE_TIMEOUT;
        loop {
            if let Some(guard) = Self::try_acquire(s3_client, bucket, key).await? {
                return Ok(guard);
            }
            if Self::take_over_stale(s3_client, bucket, key).await? {
                continue;
            }
            if Instant::now() >= deadline {
                warn!("timed out waiting for lock {}", key);
                return Err(MduError::LockContention {
                    key: key.as_str().to_string(),
                });
            }
            sleep(RETRY_INTERVAL).await;
        }
    }

    async fn try_acquire(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
    ) -> MduResult<Option<Self>> {
        let target = S3::object_target(bucket, key);
        let token = Self::new_lock_token();
        let output = match s3_client
            .put_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .if_none_match("*")
            .body(ByteStream::from(token.clone().into_bytes()))
            .send()
            .await
        {
            Ok(output) => output,
            Err(error) if Self::is_precondition_failed(&error) => return Ok(None),
            Err(error) => return Err(MduError::s3("acquire lock", target, error)),
        };
        let Some(e_tag) = Self::resolve_e_tag(s3_client, bucket, key, output.e_tag()).await? else {
            Self::remove_unreleasable_lock(s3_client, bucket, key, &token).await;
            return Err(MduError::MissingLockETag {
                key: key.as_str().to_string(),
            });
        };
        info!("acquired lock {} ({})", key, e_tag);
        Ok(Some(Self {
            s3_client: s3_client.clone(),
            bucket: bucket.clone(),
            key: key.clone(),
            e_tag,
            released: false,
        }))
    }

    async fn resolve_e_tag(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
        e_tag: Option<&str>,
    ) -> MduResult<Option<String>> {
        if let Some(e_tag) = e_tag.filter(|e_tag| !e_tag.is_empty()) {
            return Ok(Some(e_tag.to_string()));
        }
        let target = S3::object_target(bucket, key);
        match s3_client
            .head_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .send()
            .await
        {
            Ok(head) => Ok(head
                .e_tag()
                .filter(|e_tag| !e_tag.is_empty())
                .map(str::to_string)),
            Err(SdkError::ServiceError(error)) if error.err().is_not_found() => Ok(None),
            Err(error) => Err(MduError::s3("read lock metadata", target, error)),
        }
    }

    fn new_lock_token() -> String {
        let sequence = NEXT_LOCK_TOKEN.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        format!("locked:{}:{timestamp}:{sequence}", std::process::id())
    }

    async fn remove_unreleasable_lock(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
        token: &str,
    ) {
        let output = match s3_client
            .get_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .send()
            .await
        {
            Ok(output) => output,
            Err(SdkError::ServiceError(error)) if error.err().is_no_such_key() => return,
            Err(error) => {
                warn!(
                    "failed to inspect lock {} acquired without an ETag: {error}",
                    key
                );
                return;
            }
        };
        let e_tag = output
            .e_tag()
            .filter(|e_tag| !e_tag.is_empty())
            .map(str::to_string);
        let Some(e_tag) = e_tag else {
            warn!(
                "failed to remove lock {} acquired without an ETag: still missing an ETag",
                key
            );
            return;
        };
        let body = match output.body.collect().await {
            Ok(body) => body.into_bytes(),
            Err(error) => {
                warn!(
                    "failed to read lock {} acquired without an ETag: {error}",
                    key
                );
                return;
            }
        };
        if body.as_ref() != token.as_bytes() {
            warn!(
                "not removing lock {} because it is owned by another invocation",
                key
            );
            return;
        }
        match s3_client
            .delete_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .if_match(e_tag)
            .send()
            .await
        {
            Ok(_) => warn!("removed lock {} acquired without an ETag", key),
            Err(error) if Self::is_precondition_failed(&error) => {
                warn!("lock {} was taken over before it could be removed", key);
            }
            Err(error) => warn!(
                "failed to remove lock {} acquired without an ETag: {error}",
                key
            ),
        }
    }

    async fn take_over_stale(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
    ) -> MduResult<bool> {
        let target = S3::object_target(bucket, key);
        let head = match s3_client
            .head_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .send()
            .await
        {
            Ok(head) => head,
            Err(SdkError::ServiceError(error)) if error.err().is_not_found() => return Ok(false),
            Err(error) => {
                return Err(MduError::s3("read lock metadata", target.clone(), error));
            }
        };
        let Some(last_modified) = head
            .last_modified()
            .and_then(|value| DateTime::from_timestamp(value.secs(), value.subsec_nanos()))
        else {
            return Ok(false);
        };
        let stale_after =
            chrono::Duration::from_std(STALE_AFTER).map_err(|_| MduError::DateTimeOutOfRange {
                operation: "convert lock staleness duration",
            })?;
        if Utc::now().signed_duration_since(last_modified) < stale_after {
            return Ok(false);
        }
        let Some(e_tag) = head.e_tag().filter(|e_tag| !e_tag.is_empty()) else {
            return Ok(false);
        };
        match s3_client
            .delete_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .if_match(e_tag)
            .send()
            .await
        {
            Ok(_) => {
                info!("removed stale lock {}", key);
                Ok(true)
            }
            Err(error) if Self::is_precondition_failed(&error) => Ok(false),
            Err(error) => Err(MduError::s3("remove stale lock", target, error)),
        }
    }

    pub async fn release(&mut self) -> MduResult<()> {
        if self.released {
            return Ok(());
        }
        let result = Self::delete_lock(&self.s3_client, &self.bucket, &self.key, &self.e_tag).await;
        self.released = result.is_ok();
        result
    }

    async fn delete_lock(
        s3_client: &S3Client,
        bucket: &Bucket,
        key: &ObjectKey,
        e_tag: &str,
    ) -> MduResult<()> {
        let target = S3::object_target(bucket, key);
        if e_tag.is_empty() {
            return Err(MduError::MissingLockETag {
                key: key.as_str().to_string(),
            });
        }
        match s3_client
            .delete_object()
            .bucket(bucket.as_str())
            .key(key.as_str())
            .if_match(e_tag)
            .send()
            .await
        {
            Ok(_) => {
                info!("released lock {}", key);
                Ok(())
            }
            Err(error) if Self::is_precondition_failed(&error) => {
                warn!("lock {} was taken over before release", key);
                Ok(())
            }
            Err(error) => Err(MduError::s3("release lock", target, error)),
        }
    }

    fn is_precondition_failed<E>(error: &SdkError<E>) -> bool
    where
        E: ProvideErrorMetadata,
    {
        if error
            .raw_response()
            .is_some_and(|response| response.status().as_u16() == 412)
        {
            return true;
        }
        error
            .as_service_error()
            .and_then(|service_error| service_error.code())
            .is_some_and(|code| {
                code == "PreconditionFailed" || code == "ConditionalRequestConflict"
            })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        let s3_client = self.s3_client.clone();
        let bucket = self.bucket.clone();
        let key = self.key.clone();
        let e_tag = self.e_tag.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                drop(handle.spawn(async move {
                    if let Err(error) =
                        LockGuard::delete_lock(&s3_client, &bucket, &key, &e_tag).await
                    {
                        warn!("failed to release lock {} on drop: {error}", key);
                    }
                }));
            }
            Err(_) => {
                warn!("no tokio runtime available to release lock {} on drop", key);
            }
        }
    }
}
