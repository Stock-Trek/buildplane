use crate::{
    error::{BaError, BaResult},
    s3::SignedPost,
    source::Source,
};
use std::{
    fs as std_fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::TempDir;
use tokio::{fs, process::Command, sync::Mutex};
use tracing::info;

const WASM_MANIFEST: &str = "crates/runner/Cargo.toml";
const WASM_TARGET: &str = "wasm32-wasip1";
const WASM_ARTIFACT_FILE: &str = "runner.wasm";
const CWASM_ARTIFACT_FILE: &str = "runner.cwasm";
const SOURCE_TARGET_DIR: &str = "target";
const USER_GENERATED_DIR: &str = "crates/runner/src/user_generated";
const WORKSPACE_ENTRIES: &[&str] = &["Cargo.toml", "Cargo.lock", "crates", ".cargo"];
const CACHE_READY_FILE: &str = ".buildplane-cache-ready";
const CARGO: &str = "cargo";
const WASMTIME: &str = "wasmtime";

static BUILD_CACHE_LOCK: Mutex<()> = Mutex::const_new(());

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
    ) -> BaResult<Self> {
        let build_dir = tempfile::tempdir().map_err(BaError::CreateBuildDirectory)?;
        Ok(Self {
            signed_post,
            source_get_url,
            source_dir,
            build_cache_dir,
            build_dir: Arc::new(build_dir),
        })
    }

    pub async fn compile(&self) -> BaResult<PathBuf> {
        self.prepare_workspace()?;
        self.unpack_source().await?;
        let wasm = self.build_wasm().await?;
        self.compile_wasm(&wasm).await
    }

    fn prepare_workspace(&self) -> BaResult<()> {
        let workspace = self.build_dir.path();
        if !self.source_dir.join("Cargo.toml").is_file() {
            return Err(BaError::Config(format!(
                "build workspace source {} does not contain Cargo.toml",
                self.source_dir.display()
            )));
        }

        std_fs::create_dir_all(workspace).map_err(|source| BaError::PrepareWorkspace {
            path: workspace.to_path_buf(),
            source,
        })?;
        for entry in WORKSPACE_ENTRIES {
            let from = self.source_dir.join(entry);
            if !from.exists() {
                continue;
            }
            let to = workspace.join(entry);
            copy_path(&from, &to)
                .map_err(|source| BaError::CopyWorkspaceEntry { path: from, source })?;
        }
        Ok(())
    }

    async fn unpack_source(&self) -> BaResult<()> {
        let source = Source::download(&self.source_get_url).await?;
        source.extract(self.build_dir.path())
    }

    async fn build_wasm(&self) -> BaResult<PathBuf> {
        let target_dir = self.build_target_dir().await?;
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
            .arg(&target_dir)
            .arg("--release")
            .arg("--lib")
            .arg("--locked")
            .arg("--offline")
            .current_dir(workspace)
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|source| BaError::CompileArtifact {
                path: module.clone(),
                source,
            })?;

        if !status.success() {
            return Err(BaError::CompileArtifactFailed {
                path: module,
                status,
            });
        }
        info!(
            manifest = %manifest.display(),
            target = WASM_TARGET,
            target_dir = %target_dir.display(),
            "built wasm artifact"
        );
        Ok(module)
    }

    async fn build_target_dir(&self) -> BaResult<PathBuf> {
        let _guard = BUILD_CACHE_LOCK.lock().await;
        let source = self.source_dir.join(SOURCE_TARGET_DIR);
        let destination = self.build_cache_dir.clone();

        if source.is_dir() {
            // Use a marker so a cache copy interrupted by a timeout or crash is
            // discarded rather than being reused in a half-written state.
            if !destination.join(CACHE_READY_FILE).is_file() {
                let _ = fs::remove_dir_all(&destination).await;
                let from = source.clone();
                let to = destination.clone();
                tokio::task::spawn_blocking(move || copy_path(&from, &to))
                    .await
                    .map_err(|source| {
                        BaError::Config(format!("failed to prepare build cache: {source}"))
                    })?
                    .map_err(|source| BaError::CopyBuildCache {
                        path: destination.clone(),
                        source,
                    })?;
                fs::write(destination.join(CACHE_READY_FILE), b"")
                    .await
                    .map_err(|source| BaError::PrepareBuildCache {
                        path: destination.clone(),
                        source,
                    })?;
            }
        } else if !destination.is_dir() {
            fs::create_dir_all(&destination).await.map_err(|source| {
                BaError::PrepareBuildCache {
                    path: destination.clone(),
                    source,
                }
            })?;
        }

        Ok(destination)
    }

    async fn compile_wasm(&self, module: &Path) -> BaResult<PathBuf> {
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
            .map_err(|source| BaError::CompileArtifact {
                path: module.to_path_buf(),
                source,
            })?;

        if !status.success() {
            return Err(BaError::CompileArtifactFailed {
                path: module.to_path_buf(),
                status,
            });
        }
        info!(module = %module.display(), output = %output.display(), "compiled wasm artifact");
        Ok(output)
    }

    pub async fn publish(&self, cwasm: &Path) -> BaResult<()> {
        self.signed_post.upload(cwasm).await?;
        let root = self.build_dir.path().join(USER_GENERATED_DIR);
        for source in Self::user_generated_files(&root).await? {
            let name = source.strip_prefix(&root).map_err(|_| {
                BaError::Config(format!(
                    "artifact path {} is outside {USER_GENERATED_DIR}",
                    source.display()
                ))
            })?;
            self.signed_post.upload_named(&source, name).await?;
        }
        info!("published build artifacts");
        Ok(())
    }

    async fn user_generated_files(root: &Path) -> BaResult<Vec<PathBuf>> {
        let root =
            fs::canonicalize(root)
                .await
                .map_err(|source| BaError::ReadArtifactDirectory {
                    path: root.to_path_buf(),
                    source,
                })?;
        let mut files = Vec::new();
        let mut pending = vec![root.clone()];
        while let Some(directory) = pending.pop() {
            let mut entries = fs::read_dir(&directory).await.map_err(|source| {
                BaError::ReadArtifactDirectory {
                    path: directory.clone(),
                    source,
                }
            })?;
            while let Some(entry) =
                entries
                    .next_entry()
                    .await
                    .map_err(|source| BaError::ReadArtifactDirectory {
                        path: directory.clone(),
                        source,
                    })?
            {
                let path = entry.path();
                let file_type =
                    entry
                        .file_type()
                        .await
                        .map_err(|source| BaError::ReadArtifactDirectory {
                            path: directory.clone(),
                            source,
                        })?;
                let resolved =
                    fs::canonicalize(&path)
                        .await
                        .map_err(|source| BaError::ReadArtifact {
                            path: path.clone(),
                            source,
                        })?;
                if !resolved.starts_with(&root) {
                    return Err(BaError::Config(format!(
                        "artifact path {} resolves outside {USER_GENERATED_DIR}",
                        path.display()
                    )));
                }

                let target =
                    fs::metadata(&resolved)
                        .await
                        .map_err(|source| BaError::ReadArtifact {
                            path: path.clone(),
                            source,
                        })?;
                if target.is_dir() {
                    if file_type.is_symlink() {
                        return Err(BaError::Config(format!(
                            "artifact path {} is a symlinked directory",
                            path.display()
                        )));
                    }
                    pending.push(path);
                } else if target.is_file() {
                    files.push(path);
                } else {
                    return Err(BaError::Config(format!(
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
