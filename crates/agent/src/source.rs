//! Download and unpack the user's algorithm sources.
//!
//! The orchestrator uploads a zip archive of the repository's
//! `src/algorithms` directory and passes the pre-signed download URL in the
//! build request as `SIGNED_GET_URL`. Archive paths are rooted at
//! `crates/runner/src/user_generated/` so extracting the archive over the
//! working directory replaces the checked in algorithm stub with the user's
//! source.

use crate::error::{BaError, BaResult};
use std::{
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
};
use tracing::info;
use zip::ZipArchive;

const USER_GENERATED_DIR: &str = "crates/runner/src/user_generated";

const USER_GENERATED_PREFIX: &str = "crates/runner/src/user_generated/";

pub struct Source {
    bytes: Vec<u8>,
}

impl Source {
    pub async fn download(url: &str) -> BaResult<Self> {
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .map_err(BaError::DownloadSource)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(BaError::SourceDownloadRejected {
                status: status.as_u16(),
                body,
            });
        }
        let bytes = response
            .bytes()
            .await
            .map_err(BaError::ReadSourceResponse)?;
        info!(bytes = bytes.len(), "downloaded user source archive");
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }

    pub fn extract(&self, destination: &Path) -> BaResult<()> {
        let mut archive =
            ZipArchive::new(Cursor::new(&self.bytes)).map_err(BaError::ReadSourceArchive)?;

        // Validate every path before writing anything, so a malformed archive
        // cannot leave a partially extracted source tree behind.
        let entry_count = archive.len();
        let mut entries = Vec::with_capacity(entry_count);
        for index in 0..entry_count {
            let entry = archive
                .by_index(index)
                .map_err(BaError::ReadSourceArchive)?;
            let name = entry.name().map_err(BaError::ReadSourceArchive)?;
            entries.push(Self::entry_path(&name)?);
        }

        for (index, relative) in entries.into_iter().enumerate() {
            let mut entry = archive
                .by_index(index)
                .map_err(BaError::ReadSourceArchive)?;
            let path = destination.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&path).map_err(|source| {
                    BaError::CreateSourceDirectory {
                        path: path.clone(),
                        source,
                    }
                })?;
                continue;
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| {
                    BaError::CreateSourceDirectory {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
            let mut output =
                std::fs::File::create(&path).map_err(|source| BaError::WriteSourceFile {
                    path: path.clone(),
                    source,
                })?;
            std::io::copy(&mut entry, &mut output).map_err(|source| BaError::WriteSourceFile {
                path: path.clone(),
                source,
            })?;
            output.flush().map_err(|source| BaError::WriteSourceFile {
                path: path.clone(),
                source,
            })?;
        }

        info!(
            destination = %destination.display(),
            entries = entry_count,
            "unpacked user source archive"
        );
        Ok(())
    }

    fn entry_path(name: &str) -> BaResult<PathBuf> {
        if name.contains('\\') {
            return Err(BaError::UnsafeSourcePath(name.to_string()));
        }
        let path = Path::new(name);
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(BaError::UnsafeSourcePath(name.to_string()));
        }
        if path != Path::new(USER_GENERATED_DIR) && !path.starts_with(USER_GENERATED_PREFIX) {
            return Err(BaError::UnsafeSourcePath(name.to_string()));
        }
        Ok(path.to_path_buf())
    }
}
