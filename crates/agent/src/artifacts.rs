use crate::{s3::SignedPost, source::Source};
use buildplane_shared::errors::{BpError, BpResult};
use std::{
    fs as std_fs,
    io::{self, Cursor, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::TempDir;
use tokio::{fs, process::Command};
use tracing::info;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const WASM_MANIFEST: &str = "crates/runner/Cargo.toml";
const WASM_TARGET: &str = "wasm32-wasip1";
const WASM_ARTIFACT_FILE: &str = "runner.wasm";
const CWASM_ARTIFACT_FILE: &str = "runner.cwasm";
const SOURCE_TARGET_DIR: &str = "target";
const USER_GENERATED_DIR: &str = "crates/runner/src/user_generated";
const USER_GENERATED_ARCHIVE: &str = "sources.zip";
const WORKSPACE_ENTRIES: &[&str] = &["Cargo.toml", "Cargo.lock", "crates", ".cargo"];
const CARGO: &str = "cargo";
const WASMTIME: &str = "wasmtime";

#[derive(Clone)]
pub struct Artifacts {
    signed_post: SignedPost,
    source_get_url: String,
    source_dir: PathBuf,
    build_cache_dir: PathBuf,
    build_dir: Arc<TempDir>,
}

impl Artifacts {
    pub fn new(
        signed_post: SignedPost,
        source_get_url: String,
        source_dir: PathBuf,
        build_cache_dir: PathBuf,
    ) -> BpResult<Self> {
        let build_dir = tempfile::tempdir().map_err(|source| {
            BpError::io(
                "create build directory",
                "temporary build directory",
                source,
            )
        })?;
        Ok(Self {
            signed_post,
            source_get_url,
            source_dir,
            build_cache_dir,
            build_dir: Arc::new(build_dir),
        })
    }

    pub async fn compile(&self) -> BpResult<PathBuf> {
        self.prepare_workspace()?;
        self.unpack_source().await?;
        let target_dir = self.prepare_target_dir().await?;
        let wasm = self.build_wasm(target_dir.path()).await?;
        self.compile_wasm(&wasm).await
    }

    fn prepare_workspace(&self) -> BpResult<()> {
        let workspace = self.build_dir.path();
        if !self.source_dir.join("Cargo.toml").is_file() {
            return Err(BpError::BuildConfig(format!(
                "build workspace source {} does not contain Cargo.toml",
                self.source_dir.display()
            )));
        }

        std_fs::create_dir_all(workspace).map_err(|source| {
            BpError::io(
                "prepare build workspace",
                workspace.display().to_string(),
                source,
            )
        })?;
        for entry in WORKSPACE_ENTRIES {
            let from = self.source_dir.join(entry);
            if !from.exists() {
                continue;
            }
            let to = workspace.join(entry);
            copy_path(&from, &to).map_err(|source| {
                BpError::io(
                    "copy build workspace entry",
                    from.display().to_string(),
                    source,
                )
            })?;
        }
        Ok(())
    }

    async fn unpack_source(&self) -> BpResult<()> {
        let source = Source::download(&self.source_get_url).await?;
        source.extract(self.build_dir.path())
    }

    async fn build_wasm(&self, target_dir: &Path) -> BpResult<PathBuf> {
        let workspace = self.build_dir.path();
        let manifest = workspace.join(WASM_MANIFEST);
        let module = target_dir
            .join(WASM_TARGET)
            .join("release")
            .join(WASM_ARTIFACT_FILE);
        let status = Command::new(CARGO)
            .arg("build")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target")
            .arg(WASM_TARGET)
            .arg("--target-dir")
            .arg(target_dir)
            .arg("--release")
            .arg("--lib")
            .arg("--locked")
            .arg("--offline")
            .current_dir(workspace)
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|source| {
                BpError::io("compile artifact", module.display().to_string(), source)
            })?;

        if !status.success() {
            return Err(BpError::command_failed("compile artifact", module, status));
        }
        info!(
            manifest = %manifest.display(),
            target = WASM_TARGET,
            target_dir = %target_dir.display(),
            "built wasm artifact"
        );
        Ok(module)
    }

    /// Create a pristine target directory for this invocation.
    ///
    /// The prebuilt target directory baked into the image is copied into a
    /// fresh temporary directory every time, so cargo never sees artifacts
    /// produced by user code from an earlier invocation.
    async fn prepare_target_dir(&self) -> BpResult<TempDir> {
        let cache_root = self.build_cache_dir.clone();
        std_fs::create_dir_all(&cache_root).map_err(|source| {
            BpError::io(
                "prepare build cache",
                cache_root.display().to_string(),
                source,
            )
        })?;
        let target_dir = tempfile::Builder::new()
            .prefix("buildplane-target-")
            .tempdir_in(&cache_root)
            .map_err(|source| {
                BpError::io(
                    "prepare build cache",
                    cache_root.display().to_string(),
                    source,
                )
            })?;

        let source = self.source_dir.join(SOURCE_TARGET_DIR);
        if source.is_dir() {
            let from = source;
            let to = target_dir.path().to_path_buf();
            tokio::task::spawn_blocking(move || copy_path(&from, &to))
                .await
                .map_err(|source| {
                    BpError::BuildConfig(format!("failed to prepare build cache: {source}"))
                })?
                .map_err(|source| {
                    BpError::io(
                        "copy build cache",
                        target_dir.path().display().to_string(),
                        source,
                    )
                })?;
        }

        Ok(target_dir)
    }

    async fn compile_wasm(&self, module: &Path) -> BpResult<PathBuf> {
        let output = self.build_dir.path().join(CWASM_ARTIFACT_FILE);
        let status = Command::new(WASMTIME)
            .arg("compile")
            .arg("-C")
            .arg("cache=no")
            .arg(module)
            .arg("-o")
            .arg(&output)
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|source| {
                BpError::io("compile artifact", module.display().to_string(), source)
            })?;

        if !status.success() {
            return Err(BpError::command_failed(
                "compile artifact",
                module.to_path_buf(),
                status,
            ));
        }
        info!(module = %module.display(), output = %output.display(), "compiled wasm artifact");
        Ok(output)
    }

    pub async fn publish(&self, cwasm: &Path) -> BpResult<()> {
        self.signed_post.upload(cwasm).await?;
        let root = self.build_dir.path().join(USER_GENERATED_DIR);
        let files = Self::user_generated_files(&root).await?;
        let archive = Self::source_archive(&root, &files)?;
        let archive_path = self.build_dir.path().join(USER_GENERATED_ARCHIVE);
        fs::write(&archive_path, archive).await.map_err(|source| {
            BpError::io(
                "write source archive",
                archive_path.display().to_string(),
                source,
            )
        })?;
        self.signed_post
            .upload_named(&archive_path, Path::new(USER_GENERATED_ARCHIVE))
            .await?;
        info!(files = files.len(), "published build artifacts");
        Ok(())
    }

    fn source_archive(root: &Path, files: &[PathBuf]) -> BpResult<Vec<u8>> {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for file in files {
            let relative = file.strip_prefix(root).map_err(|_| {
                BpError::BuildConfig(format!(
                    "artifact path {} is outside {USER_GENERATED_DIR}",
                    file.display()
                ))
            })?;
            let name = relative
                .to_str()
                .ok_or_else(|| {
                    BpError::BuildConfig(format!(
                        "artifact path {} is not valid UTF-8",
                        file.display()
                    ))
                })?
                .replace('\\', "/");
            archive.start_file(name, options).map_err(BpError::Zip)?;
            let contents = std_fs::read(file)
                .map_err(|source| BpError::io("read artifact", file.display().to_string(), source))?;
            archive.write_all(&contents).map_err(|source| {
                BpError::io("write source archive", file.display().to_string(), source)
            })?;
        }
        Ok(archive.finish().map_err(BpError::Zip)?.into_inner())
    }

    async fn user_generated_files(root: &Path) -> BpResult<Vec<PathBuf>> {
        let root =
            fs::canonicalize(root)
                .await
                .map_err(|source| {
                    BpError::io(
                        "read artifact directory",
                        root.display().to_string(),
                        source,
                    )
                })?;
        let mut files = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            let mut entries = fs::read_dir(&directory).await.map_err(|source| {
                BpError::io(
                    "read artifact directory",
                    directory.display().to_string(),
                    source,
                )
            })?;
            while let Some(entry) = entries.next_entry().await.map_err(|source| {
                BpError::io(
                    "read artifact directory",
                    directory.display().to_string(),
                    source,
                )
            })? {
                let path = entry.path();
                let file_type = entry.file_type().await.map_err(|source| {
                    BpError::io(
                        "read artifact directory",
                        directory.display().to_string(),
                        source,
                    )
                })?;
                let resolved = fs::canonicalize(&path).await.map_err(|source| {
                    BpError::io("read artifact", path.display().to_string(), source)
                })?;
                if !resolved.starts_with(&root) {
                    return Err(BpError::BuildConfig(format!(
                        "artifact path {} resolves outside {USER_GENERATED_DIR}",
                        path.display()
                    )));
                }

                let target = fs::metadata(&resolved).await.map_err(|source| {
                    BpError::io("read artifact", path.display().to_string(), source)
                })?;
                if target.is_dir() {
                    if file_type.is_symlink() {
                        return Err(BpError::BuildConfig(format!(
                            "artifact path {} is a symlinked directory",
                            path.display()
                        )));
                    }
                    pending.push(path);
                } else if target.is_file() {
                    files.push(path);
                } else {
                    return Err(BpError::BuildConfig(format!(
                        "artifact path {} is not a regular file or directory",
                        path.display()
                    )));
                }
            }
        }
        files.sort();
        Ok(files)
    }
}

/// Recursively copy `from` to `to`, preserving regular files, directories and
/// symlinks. The source tree is the trusted, read-only workspace baked into the
/// image; user archives are extracted into the writable copy instead.
fn copy_path(from: &Path, to: &Path) -> io::Result<()> {
    let metadata = std_fs::symlink_metadata(from)?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        let target = std_fs::read_link(from)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(target, to)?;
        #[cfg(not(unix))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!("cannot copy symlink {}", from.display()),
        ));
    } else if file_type.is_dir() {
        std_fs::create_dir_all(to)?;
        for entry in std_fs::read_dir(from)? {
            let entry = entry?;
            copy_path(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else if file_type.is_file() {
        if let Some(parent) = to.parent() {
            std_fs::create_dir_all(parent)?;
        }
        std_fs::copy(from, to)?;
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported workspace entry {}", from.display()),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, time::Duration};

    fn artifacts(source_dir: &Path, build_cache_dir: &Path) -> Artifacts {
        let fields = BTreeMap::from([("key".to_string(), "artifacts/${filename}".to_string())]);
        let signed_post = SignedPost::new(
            "https://example.com/upload".to_string(),
            fields,
            Duration::from_secs(1),
        )
        .unwrap();

        Artifacts::new(
            signed_post,
            "https://example.com/source.zip".to_string(),
            source_dir.to_path_buf(),
            build_cache_dir.to_path_buf(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn target_dir_starts_from_the_prebuilt_cache_for_each_invocation() {
        let source = tempfile::tempdir().unwrap();
        let build_cache = tempfile::tempdir().unwrap();
        let prebuilt = source
            .path()
            .join(SOURCE_TARGET_DIR)
            .join(WASM_TARGET)
            .join("release");
        std_fs::create_dir_all(&prebuilt).unwrap();
        std_fs::write(prebuilt.join(WASM_ARTIFACT_FILE), b"prebuilt").unwrap();

        let artifacts = artifacts(source.path(), build_cache.path());
        let first = artifacts.prepare_target_dir().await.unwrap();
        let artifact = first
            .path()
            .join(WASM_TARGET)
            .join("release")
            .join(WASM_ARTIFACT_FILE);
        assert_eq!(std_fs::read(&artifact).unwrap(), b"prebuilt");

        // Simulate cargo writing output derived from a user's sources.
        std_fs::write(first.path().join("user-output"), b"previous run").unwrap();

        let second = artifacts.prepare_target_dir().await.unwrap();
        assert_ne!(first.path(), second.path());
        let artifact = second
            .path()
            .join(WASM_TARGET)
            .join("release")
            .join(WASM_ARTIFACT_FILE);
        assert_eq!(std_fs::read(artifact).unwrap(), b"prebuilt");
        assert!(!second.path().join("user-output").exists());
    }

    #[test]
    fn source_archive_contains_user_generated_files() {
        let root = tempfile::tempdir().unwrap();
        let algorithm = root.path().join("algorithms/algorithm.rs");
        std_fs::create_dir_all(algorithm.parent().unwrap()).unwrap();
        std_fs::write(&algorithm, b"algorithm").unwrap();
        let manifest = root.path().join("mod.rs");
        std_fs::write(&manifest, b"modules").unwrap();

        let bytes =
            Artifacts::source_archive(root.path(), &[algorithm.clone(), manifest.clone()]).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();

        assert_eq!(archive.len(), 2);
        assert_eq!(
            archive.by_index(0).unwrap().name().unwrap(),
            "algorithms/algorithm.rs"
        );
        assert_eq!(archive.by_index(1).unwrap().name().unwrap(), "mod.rs");
    }
}
