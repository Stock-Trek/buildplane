use crate::{
    constants::PARQUET_SUFFIX,
    parquet_reader::ParquetReader,
    parquet_writer::writer_properties,
    plan,
    s3::{Bucket, ObjectKey, S3},
    subfolders::SubFolders,
};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use buildplane_errors::{BpError, BpResult};
use buildplane_metrics::market_data::MarketDataMetrics;
use buildplane_time::deadline::Deadline;
use futures::TryStreamExt;
use parquet::arrow::{AsyncArrowWriter, ParquetRecordBatchStreamBuilder};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use tokio::fs::File;
use tracing::{info, warn};

const READ_BATCH_SIZE: usize = 8192;
const STAGING_SUFFIX: &str = ".staging";
const TEMP_FILE_PREFIX: &str = "market-data-updater-compaction-";

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempCompactionFile {
    path: PathBuf,
    removed: bool,
}

impl TempCompactionFile {
    fn new() -> Self {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "{TEMP_FILE_PREFIX}{}-{counter}.parquet",
            std::process::id()
        ));
        Self {
            path,
            removed: false,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    async fn remove(&mut self) {
        if self.removed {
            return;
        }
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => self.removed = true,
            Err(error) if error.kind() == ErrorKind::NotFound => self.removed = true,
            Err(error) => Self::log_removal_error(&self.path, &error),
        }
    }

    fn log_removal_error(path: &Path, error: &std::io::Error) {
        if error.kind() != ErrorKind::NotFound {
            warn!(
                "failed to remove temporary compaction file {}: {error}",
                path.display()
            );
        }
    }
}

impl Drop for TempCompactionFile {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        self.removed = true;
        if let Err(error) = std::fs::remove_file(&self.path) {
            Self::log_removal_error(&self.path, &error);
        }
    }
}

pub struct Compactor;

struct SubfoldersFiles {
    pub subfolders: Vec<String>,
    pub files: Vec<ObjectKey>,
}

impl Compactor {
    /// Compact one specific trading-pair prefix.
    ///
    /// Ready children are rolled up recursively, preserving the existing
    /// staged-copy and delete-source safety behavior.
    pub async fn prefix(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        prefix: &ObjectKey,
        deadline: Deadline,
    ) -> BpResult<()> {
        let started_at = Instant::now();
        let result = Self::folder(
            s3_client,
            transfer_client,
            bucket,
            prefix,
            PARQUET_SUFFIX,
            deadline,
        )
        .await;
        match &result {
            Ok(()) => {
                MarketDataMetrics::record_compaction_success(started_at.elapsed());
            }
            Err(error) => {
                MarketDataMetrics::record_compaction_failure(
                    error.error_type(),
                    started_at.elapsed(),
                );
            }
        }
        result
    }

    async fn folder(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        prefix: &ObjectKey,
        suffix: &str,
        deadline: Deadline,
    ) -> BpResult<()> {
        deadline.remaining()?;
        info!("compact folder with prefix {}", prefix);
        let expected_subfolders = match SubFolders::subfolders_from_path(prefix.as_str()) {
            Ok(subfolders) => subfolders,
            Err(error) => {
                warn!("skipping malformed path {}: {error}", prefix);
                return Ok(());
            }
        };
        let expected_files =
            plan::Planner::expected_compaction_sources_with_suffix(prefix, suffix)?;
        if !expected_subfolders.is_empty() {
            let compacted_file = plan::Planner::compacted_file_path(prefix, suffix);
            let staged_file = Self::staged_file_path(&compacted_file);
            if S3::has_file(s3_client, bucket, &compacted_file).await? {
                info!(
                    "{} is already compacted; deleting remaining source files",
                    compacted_file
                );
                S3::delete(s3_client, bucket, &staged_file).await?;
                Self::delete_sources(s3_client, bucket, prefix, &expected_files, deadline).await?;
                return Ok(());
            }
            if S3::has_file(s3_client, bucket, &staged_file).await? {
                info!("publishing staged compacted file {}", staged_file);
                S3::copy_file(s3_client, bucket, &staged_file, &compacted_file).await?;
                Self::delete_sources(s3_client, bucket, prefix, &expected_files, deadline).await?;
                S3::delete(s3_client, bucket, &staged_file).await?;
                return Ok(());
            }
        }
        let subfolders_files =
            Self::subfolders_files(s3_client, bucket, prefix, suffix, deadline).await?;
        let recursed = !subfolders_files.subfolders.is_empty();
        for subfolder in &subfolders_files.subfolders {
            let subfolder_full_path = ObjectKey::new(format!("{}{}", prefix, subfolder));
            deadline.remaining()?;
            Box::pin(Self::folder(
                s3_client,
                transfer_client,
                bucket,
                &subfolder_full_path,
                suffix,
                deadline,
            ))
            .await?;
        }
        if expected_subfolders.is_empty() {
            info!("no expected subfolders for {}", prefix);
            return Ok(());
        }
        // Recursion may have produced compacted files at this level, so the
        // listing taken before recursing can be stale. Re-list to see them
        // within the same invocation instead of waiting for the next run.
        let subfolders_files = if recursed {
            Self::subfolders_files(s3_client, bucket, prefix, suffix, deadline).await?
        } else {
            subfolders_files
        };
        let missing = expected_files
            .iter()
            .filter(|expected| !subfolders_files.files.contains(*expected))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            info!(
                "Compaction for {} is missing {}/{} files",
                prefix,
                missing.len(),
                expected_files.len()
            );
            return Ok(());
        }
        let Some(target) = plan::Planner::plan_compaction_with_suffix(
            prefix,
            subfolders_files.files.iter().map(ObjectKey::as_str),
            suffix,
        )?
        else {
            info!("Compaction for {} is not ready", prefix);
            return Ok(());
        };
        info!(
            "compact files {:?} to {}",
            target.source_files, target.compacted_file
        );
        Self::files(
            s3_client,
            transfer_client,
            bucket,
            prefix,
            &target.source_files,
            &target.compacted_file,
            deadline,
        )
        .await?;
        Ok(())
    }

    fn staged_file_path(compacted_file: &ObjectKey) -> ObjectKey {
        ObjectKey::new(format!("{}{}", compacted_file, STAGING_SUFFIX))
    }

    async fn delete_sources(
        s3_client: &S3Client,
        bucket: &Bucket,
        prefix: &ObjectKey,
        files: &[ObjectKey],
        deadline: Deadline,
    ) -> BpResult<()> {
        deadline.remaining()?;
        if files.is_empty() {
            return Ok(());
        }
        info!("deleting {} source files under {}", files.len(), prefix);
        S3::delete_all(s3_client, bucket, files).await
    }

    async fn subfolders_files(
        s3_client: &S3Client,
        bucket: &Bucket,
        prefix: &ObjectKey,
        suffix: &str,
        deadline: Deadline,
    ) -> BpResult<SubfoldersFiles> {
        info!("find subfolders and files for prefix {}", prefix);
        let mut list_request = s3_client
            .list_objects_v2()
            .bucket(bucket.as_str())
            .delimiter("/");
        if !prefix.as_str().is_empty() {
            list_request = list_request.prefix(prefix.as_str());
        }
        let mut paginator = list_request.into_paginator().send();
        let mut files = Vec::new();
        let mut subfolders = Vec::new();
        while let Some(page_result) = {
            deadline.remaining()?;
            paginator.next().await
        } {
            let page = page_result.map_err(|error| {
                BpError::s3(
                    "list subfolders and files",
                    S3::object_target(bucket, prefix),
                    error,
                )
            })?;
            subfolders.extend(
                page.common_prefixes()
                    .iter()
                    .flat_map(|cp| cp.prefix())
                    .map(|s| s.strip_prefix(prefix.as_str()).unwrap_or(s).to_string()),
            );
            for object in page.contents() {
                if let Some(key) = object.key()
                    && key.ends_with(suffix)
                {
                    files.push(ObjectKey::new(key));
                }
            }
        }
        Ok(SubfoldersFiles { subfolders, files })
    }

    #[allow(clippy::too_many_arguments)]
    async fn files(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        prefix: &ObjectKey,
        files: &[ObjectKey],
        compacted_file: &ObjectKey,
        deadline: Deadline,
    ) -> BpResult<()> {
        deadline.remaining()?;
        info!("compact files {:?} to {}", files, compacted_file);
        if files.is_empty() {
            warn!("No data to compact");
            return Ok(());
        }
        let staged_file = Self::staged_file_path(compacted_file);
        if !Self::compact(
            s3_client,
            transfer_client,
            bucket,
            files,
            &staged_file,
            deadline,
        )
        .await?
        {
            warn!(
                "no non-empty data to compact for {:?}; removing empty files",
                files
            );
            Self::delete_sources(s3_client, bucket, prefix, files, deadline).await?;
            return Ok(());
        }
        deadline.remaining()?;
        S3::copy_file(s3_client, bucket, &staged_file, compacted_file).await?;
        Self::delete_sources(s3_client, bucket, prefix, files, deadline).await?;
        S3::delete(s3_client, bucket, &staged_file).await?;
        Ok(())
    }

    async fn compact(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        files: &[ObjectKey],
        compacted_file: &ObjectKey,
        deadline: Deadline,
    ) -> BpResult<bool> {
        info!("compact {} files", files.len());
        let mut temp_file = TempCompactionFile::new();
        let mut writer: Option<AsyncArrowWriter<File>> = None;
        for file_key in files {
            deadline.remaining()?;
            let file_size = S3::object_size(s3_client, bucket, file_key).await?;
            let reader = ParquetReader::new(transfer_client, bucket, file_key, file_size);
            let mut stream = ParquetRecordBatchStreamBuilder::new(reader)
                .await?
                .with_batch_size(READ_BATCH_SIZE)
                .build()?;
            while let Some(batch) = stream.try_next().await? {
                deadline.remaining()?;
                if writer.is_none() {
                    let output = File::create(temp_file.path()).await.map_err(|error| {
                        BpError::io(
                            "create temporary compaction file",
                            temp_file.path().display().to_string(),
                            error,
                        )
                    })?;
                    writer = Some(AsyncArrowWriter::try_new(
                        output,
                        batch.schema(),
                        Some(writer_properties()),
                    )?);
                }
                if let Some(writer) = writer.as_mut() {
                    writer.write(&batch).await?;
                }
            }
        }
        let Some(writer) = writer else {
            warn!("all parquet files for compaction are empty");
            return Ok(false);
        };
        writer.close().await?;
        S3::upload_file(transfer_client, bucket, compacted_file, temp_file.path()).await?;
        temp_file.remove().await;
        Ok(true)
    }
}
