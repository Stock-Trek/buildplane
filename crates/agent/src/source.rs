//! Download and unpack the user's algorithm sources.
//!
//! The orchestrator uploads a zip archive of the repository's
//! `src/algorithms` directory and passes the pre-signed download URL in the
//! build request as `signed_get_url`. Archive paths are rooted at
//! `crates/runner/src/user_generated/` so extracting the archive over the
//! working directory replaces the checked in algorithm stub with the user's
//! source.

use buildplane_errors::errors::IoError;
use buildplane_errors::errors::{BpError, BpResult};
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
    pub async fn download(url: &str) -> BpResult<Self> {
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .map_err(|source| BpError::http_request("download user source archive", url, source))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(BpError::http_rejected(
                "download user source archive",
                url,
                status.as_u16(),
                body,
            ));
        }
        let bytes = response.bytes().await.map_err(|source| {
            BpError::http_request("read user source archive response", url, source)
        })?;
        info!(bytes = bytes.len(), "downloaded user source archive");
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }

    pub fn extract(&self, destination: &Path) -> BpResult<Vec<PathBuf>> {
        let mut archive = ZipArchive::new(Cursor::new(&self.bytes))
            .map_err(|error| BpError::Io(IoError::Zip(error)))?;

        // Validate every path before writing anything, so a malformed archive
        // cannot leave a partially extracted source tree behind.
        let entry_count = archive.len();
        let mut entries = Vec::with_capacity(entry_count);
        for index in 0..entry_count {
            let entry = archive
                .by_index(index)
                .map_err(|error| BpError::Io(IoError::Zip(error)))?;
            let name = entry
                .name()
                .map_err(|error| BpError::Io(IoError::Zip(error)))?;
            entries.push(Self::entry_path(&name)?);
        }

        let mut files = Vec::new();
        for (index, relative) in entries.into_iter().enumerate() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| BpError::Io(IoError::Zip(error)))?;
            let path = destination.join(relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&path).map_err(|source| {
                    BpError::io(
                        "create source directory",
                        path.display().to_string(),
                        source,
                    )
                })?;
                continue;
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| {
                    BpError::io(
                        "create source directory",
                        parent.display().to_string(),
                        source,
                    )
                })?;
            }
            let mut output = std::fs::File::create(&path).map_err(|source| {
                BpError::io("write source file", path.display().to_string(), source)
            })?;
            std::io::copy(&mut entry, &mut output).map_err(|source| {
                BpError::io("write source file", path.display().to_string(), source)
            })?;
            output.flush().map_err(|source| {
                BpError::io("write source file", path.display().to_string(), source)
            })?;
            files.push(path);
        }

        info!(
            destination = %destination.display(),
            entries = entry_count,
            "unpacked user source archive"
        );
        Ok(files)
    }

    fn entry_path(name: &str) -> BpResult<PathBuf> {
        if name.contains('\\') {
            return Err(BpError::invalid_path(name));
        }
        let path = Path::new(name);
        if path.as_os_str().is_empty()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(BpError::invalid_path(name));
        }
        if path != Path::new(USER_GENERATED_DIR) && !path.starts_with(USER_GENERATED_PREFIX) {
            return Err(BpError::invalid_path(name));
        }
        Ok(path.to_path_buf())
    }
}
