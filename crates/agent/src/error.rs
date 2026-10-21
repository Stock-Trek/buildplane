use lambda_runtime::Diagnostic;
use std::{path::PathBuf, process::ExitStatus, time::Duration};

pub type BaResult<T> = Result<T, BaError>;

#[derive(Debug, thiserror::Error)]
pub enum BaError {
    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("failed to install tracing subscriber")]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),

    #[error("build timed out after {0:?}")]
    Timeout(Duration),

    #[error("failed to download user source archive")]
    DownloadSource(#[source] reqwest::Error),

    #[error("download of user source archive was rejected with status {status}: {body}")]
    SourceDownloadRejected { status: u16, body: String },

    #[error("failed to read user source archive response")]
    ReadSourceResponse(#[source] reqwest::Error),

    #[error("failed to read user source archive")]
    ReadSourceArchive(#[source] zip::result::ZipError),

    #[error("user source archive contains unsafe path {0}")]
    UnsafeSourcePath(String),

    #[error("failed to create source directory {path}")]
    CreateSourceDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write source file {path}")]
    WriteSourceFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read artifact {path}")]
    ReadArtifact {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read artifact directory {path}")]
    ReadArtifactDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to build the upload request for {path}")]
    BuildUploadRequest {
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },

    #[error("failed to upload artifact {path}")]
    UploadArtifact {
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },

    #[error("upload of {path} was rejected with status {status}: {body}")]
    UploadRejected {
        path: PathBuf,
        status: u16,
        body: String,
    },

    #[error("failed to create temporary build directory")]
    CreateBuildDirectory(#[source] std::io::Error),

    #[error("failed to prepare the build workspace at {path}")]
    PrepareWorkspace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to copy build workspace entry {path}")]
    CopyWorkspaceEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to copy the prebuilt build cache to {path}")]
    CopyBuildCache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to prepare the build cache at {path}")]
    PrepareBuildCache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to compile artifact {path}")]
    CompileArtifact {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("compilation of artifact {path} exited with {status}")]
    CompileArtifactFailed { path: PathBuf, status: ExitStatus },
}

impl From<BaError> for Diagnostic {
    fn from(value: BaError) -> Self {
        Diagnostic {
            error_type: "BuildAgent".to_string(),
            error_message: value.to_string(),
        }
    }
}
