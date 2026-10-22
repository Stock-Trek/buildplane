use crate::{
    errors::retryability::Retryability,
    new_types::{Nanoseconds, UsageCount},
    protocol::Protocol,
    sqs_message::SqsMessage,
};
use aws_credential_types::provider::error::CredentialsError;
use aws_sdk_lambda::{
    error::ProvideErrorMetadata, operation::invoke::InvokeError as SdkInvokeError,
};
use aws_sigv4::{
    http_request::SigningError, sign::v4::signing_params::BuildError as SigningParamsError,
};
use chrono::{DateTime, Utc};
use http::header::{InvalidHeaderName, InvalidHeaderValue, ToStrError};
use lambda_runtime::Diagnostic;
use std::{error::Error as StdError, path::PathBuf, process::ExitStatus, time::Duration};

const HTTP_FORBIDDEN: u16 = 403;
const HTTP_REQUEST_TIMEOUT: u16 = 408;
const HTTP_TOO_MANY_REQUESTS: u16 = 429;
const HTTP_INTERNAL_SERVER_ERROR: u16 = 500;

pub type BpResult<T> = Result<T, BpError>;

/// The single error type used across buildplane.
///
/// Variants are grouped by the kind of failure rather than by the crate that
/// originally raised them, so the same category covers every caller that can
/// encounter it.
#[derive(Debug, thiserror::Error, strum::AsRefStr, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum BpError {
    // Configuration and validation.
    #[error("{name} is missing")]
    ConfigMissing { name: String },
    #[error("{name} must not be empty")]
    ConfigEmpty { name: String },
    #[error("{name} is not valid UTF-8")]
    ConfigNotUnicode { name: String },
    #[error("{name} is invalid: {message}")]
    ConfigInvalid {
        name: String,
        message: String,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    },
    #[error("{host:?} is not a hostname, IP address, or host:port")]
    HostInvalid { host: String },
    #[error("host {host:?} is not in the allowed GitHub hosts list")]
    HostNotAllowed { host: String },
    #[error("GitHub repository id {repo_id:?} is not numeric: {source}")]
    InvalidMessageGitHubRepositoryId {
        repo_id: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("invalid path: {value:?}")]
    InvalidMessageInvalidPath { value: String },
    #[error("SQS record is missing a message id; cannot report a batch item failure")]
    InvalidMessageMissingMessageId,
    #[error("SQS record is missing a body")]
    InvalidMessageMissingBody,
    #[error("Missing {0}")]
    TypeBridgeMissing(String),
    #[error("Bad response from exchange. {0}")]
    BadResponse(String),
    #[error("Poisoned Lock")]
    PoisonedLock,
    #[error("System Time was before epoch")]
    SystemTimeBeforeEpoch,
    #[error("Unsupported {name}: {value}")]
    Unsupported { name: String, value: String },
    #[error("Invalid value for {name}: {value}")]
    InvalidValue { name: String, value: String },

    // Environment and process startup.
    #[error("EnvVar: {0}")]
    EnvVar(#[from] std::env::VarError),
    #[error("failed to install tracing subscriber")]
    Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),

    // Build-agent work.
    #[error("invalid configuration: {0}")]
    BuildConfig(String),
    #[error("build timed out after {0:?}")]
    BuildTimeout(Duration),
    #[error("failed to download user source archive")]
    BuildDownloadSource(#[source] reqwest::Error),
    #[error("download of user source archive was rejected with status {status}: {body}")]
    BuildSourceDownloadRejected { status: u16, body: String },
    #[error("failed to read user source archive response")]
    BuildReadSourceResponse(#[source] reqwest::Error),
    #[error("failed to read user source archive")]
    BuildReadSourceArchive(#[source] zip::result::ZipError),
    #[error("user source archive contains unsafe path {0}")]
    BuildUnsafeSourcePath(String),
    #[error("failed to create source directory {path}")]
    BuildCreateSourceDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write source file {path}")]
    BuildWriteSourceFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read artifact {path}")]
    BuildReadArtifact {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read artifact directory {path}")]
    BuildReadArtifactDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create user source archive")]
    BuildCreateSourceArchive(#[source] zip::result::ZipError),
    #[error("failed to write user source archive {path}")]
    BuildWriteSourceArchive {
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
    BuildUploadArtifact {
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },
    #[error("upload of {path} was rejected with status {status}: {body}")]
    BuildUploadRejected {
        path: PathBuf,
        status: u16,
        body: String,
    },
    #[error("failed to create temporary build directory")]
    BuildCreateBuildDirectory(#[source] std::io::Error),
    #[error("failed to prepare the build workspace at {path}")]
    BuildPrepareWorkspace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to copy build workspace entry {path}")]
    BuildCopyWorkspaceEntry {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to copy the prebuilt build cache to {path}")]
    BuildCopyBuildCache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to prepare the build cache at {path}")]
    BuildPrepareBuildCache {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to compile artifact {path}")]
    BuildCompileArtifact {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("compilation of artifact {path} exited with {status}")]
    BuildCompileArtifactFailed { path: PathBuf, status: ExitStatus },

    // Data ingestion, planning, and processing.
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow_schema::ArrowError),
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),
    #[error("I/O error while {operation} at {path}: {source}")]
    Io {
        operation: &'static str,
        path: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("S3 {operation} failed for {target}: {source}")]
    S3 {
        operation: &'static str,
        target: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("Failed to build S3 {operation} request for {target}: {source}")]
    S3Request {
        operation: &'static str,
        target: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("S3 delete objects failed for {target}: {failures}")]
    S3DeleteObjects { target: String, failures: String },
    #[error("Exchange request failed while trying to {operation}: {source}")]
    Exchange {
        operation: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("Background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
    #[error("Concurrency semaphore was closed while waiting for a permit")]
    SemaphoreClosed,
    #[error("Unsupported exchange: {exchange}")]
    UnsupportedExchange { exchange: String },
    #[error("Unsupported trading mode: {trading_mode}")]
    UnsupportedTradingMode { trading_mode: String },
    #[error("Cannot find a symbol for trading pair {trading_pair}")]
    MissingSymbol { trading_pair: String },
    #[error("Unknown {exchange} submission outcome while fetching klines")]
    UnknownSubmissionOutcome { exchange: &'static str },
    #[error("Invalid data job: {message}")]
    InvalidJob { message: String },
    #[error("Invalid trading pair path {pair}; expected mode/cex/base/quote")]
    InvalidTradingPairPath { pair: String },
    #[error("Trading pair {pair} is not configured for data collection")]
    UnsupportedTradingPair { pair: String },
    #[error("Backfill end time {end} must be after start time {start}")]
    InvalidBackfillRange {
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    },
    #[error("Failed to acquire lock {key}: another invocation is already running")]
    LockContention { key: String },
    #[error("Lock {key} is missing an ETag and cannot be released safely")]
    MissingLockETag { key: String },
    #[error("Month {month} must be between 1 and 12")]
    InvalidMonth { month: u32 },
    #[error("Day {day} is not valid for this month")]
    InvalidDay { day: u32 },
    #[error("Hour {hour} must be between 0 and 23")]
    InvalidHour { hour: u32 },
    #[error("Failed to parse year and month from path {path}")]
    InvalidYearMonth { path: String },
    #[error("Cannot get subfolders from path with too many delimiters {path}")]
    TooManyPathSegments { path: String },
    #[error("Date time out of range while trying to {operation}")]
    DateTimeOutOfRange { operation: &'static str },
    #[error("Failed to convert candle {field} to f64: {source}")]
    CandleConversion {
        field: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("{count} operation(s) failed: {failures}")]
    Multiple { count: usize, failures: String },

    // Archives and object storage.
    #[error("failed to write source archive: {0}")]
    ArchiveIo(#[from] std::io::Error),
    #[error("failed to create source archive: {0}")]
    ArchiveZip(#[from] zip::result::ZipError),
    #[error("failed to load AWS credentials: {0}")]
    Credentials(#[from] CredentialsError),
    #[error("invalid S3 endpoint URL: {source}")]
    InvalidEndpoint {
        #[source]
        source: url::ParseError,
    },
    #[error("S3 endpoint {endpoint:?} does not contain a host")]
    InvalidEndpointHost { endpoint: String },
    #[error("missing credentials")]
    MissingCredentials,
    #[error(
        "object {key:?} is {size} bytes, exceeding the maximum object size of {max_bytes} bytes"
    )]
    ObjectTooLarge {
        key: String,
        size: usize,
        max_bytes: u64,
    },
    #[error("failed to upload object to S3: {0}")]
    Upload(#[source] reqwest::Error),
    #[error("S3 {operation} failed for {target}: {source}")]
    S3PresignOperation {
        operation: &'static str,
        target: String,
        retryable: bool,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("S3 rejected upload for key {key:?} with status {status}: {body}")]
    StorageUploadRejected {
        key: String,
        status: u16,
        body: String,
    },

    // Exchange protocol encoding and decoding.
    #[error("Cannot send request {request_type} on protocol {protocol}")]
    BadProtocol {
        request_type: String,
        protocol: Protocol,
    },
    #[error("Cannot use {name}={value}. {reason}")]
    BadValue {
        name: String,
        value: String,
        reason: String,
    },
    #[error("Crypto key error: {0}")]
    CryptoKey(String),
    #[error("Deserialize response error: {0}")]
    DeserializeResponse(serde_json::Error),
    #[error("Parsing error: {0}")]
    ParseError(String),
    #[error("Serialize request error: {0}")]
    SerializeRequest(serde_json::Error),
    #[error("Value for {0} deserialized as Unknown")]
    UnknownValue(String),

    // Exchange gateway, submission, and rate limiting.
    #[error("Auto resync clock task panicked")]
    AutoResyncClockPanicked,
    #[error("Auto resync clock task is no longer running")]
    AutoResyncClockStopped,
    #[error("A user callback panicked: {0}")]
    CallbackPanicked(String),
    #[error(
        "The server clock has not been synchronised; call sync_clock_http or sync_clock_websocket before sending signed requests"
    )]
    ClockNotSynced,
    #[error(transparent)]
    External(#[from] Box<dyn StdError + Send + Sync + 'static>),
    #[error("HTTP request failed with status {status}")]
    GatewayHttpStatus { status: u16 },
    #[error("Failed to parse HTTP response: {source}")]
    HttpParseError {
        #[source]
        source: Box<BpError>,
    },
    #[error("Failed to parse websocket response: {source}")]
    WebsocketParseError {
        #[source]
        source: Box<BpError>,
    },
    #[error("Rate limiter capacity must be greater than zero")]
    InvalidRateLimitCapacity,
    #[error("Rate limiter interval must be greater than zero")]
    InvalidRateLimitInterval,
    #[error("Clock sync frequency must be at least 1 minute")]
    InvalidSyncFrequency,
    #[error("Server time response did not contain a server time")]
    MissingServerTime,
    #[error("Connector is not connected")]
    NotConnected,
    #[error("{source}")]
    Send {
        failure: SendFailure,
        #[source]
        source: Box<BpError>,
    },
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error(
        "Request rate limit cost {cost} exceeds the maximum capacity {capacity} for interval {interval_nanos}ns"
    )]
    RequestExceedsRateLimit {
        cost: UsageCount,
        capacity: UsageCount,
        interval_nanos: Nanoseconds,
    },
    #[error("Request timed out waiting for a response")]
    TimedOut,
    #[error("Connector was not initialised with a websocket listener")]
    WebsocketListenerMissing,

    // Cloud messaging and invocation.
    #[error("SQS PartialBatch failed to send {} message(s): {}", .failures.len(), Self::format_batch_failures(.failures))]
    SqsPartialBatch { failures: Vec<SqsBatchFailure> },
    #[error(
        "SQS message body is {size} bytes, which exceeds the maximum batch payload of {limit} bytes"
    )]
    SqsMessageTooLarge { size: usize, limit: usize },
    #[error("Lambda function returned {function_error}: {detail}", detail = .detail.as_deref().unwrap_or("no detail"))]
    LambdaFunction {
        function_error: String,
        detail: Option<String>,
    },
    #[error("Lambda invoke returned unexpected status code {status_code}")]
    LambdaStatus { status_code: i32 },
    #[error("Lambda invoke error: {0}")]
    LambdaSdkInvoke(#[from] Box<SdkInvokeError>),

    // GitHub and repository access.
    #[error("failed to build GitHub client: {0}")]
    GitHubBuildClient(#[source] octocrab::Error),
    #[error("no commit found for {ref_name:?} in repository {repo_id}")]
    GitHubCommitNotFound { repo_id: u64, ref_name: String },
    #[error("path {path:?} was not found in repository {repo_id} at {ref_name:?}: {source}")]
    GitHubContentNotFound {
        repo_id: u64,
        path: String,
        ref_name: String,
        #[source]
        source: Box<octocrab::Error>,
    },
    #[error("directory {path:?} was not found in repository {repo_id} at {ref_name:?}")]
    GitHubDirectoryNotFound {
        repo_id: u64,
        path: String,
        ref_name: String,
    },
    #[error("directory {path:?} in repository {repo_id} exceeds the maximum depth of {max_depth}")]
    GitHubDirectoryDepthExceeded {
        repo_id: u64,
        path: String,
        max_depth: usize,
    },
    #[error(
        "directory {directory:?} in repository {repo_id} exceeds the maximum total size of {max_bytes} bytes"
    )]
    GitHubDirectoryTooLarge {
        repo_id: u64,
        directory: String,
        max_bytes: u64,
    },
    #[error("failed to decode {encoding:?} content at {path:?} in repository {repo_id}: {source}")]
    GitHubDecodeContent {
        repo_id: u64,
        path: String,
        encoding: &'static str,
        #[source]
        source: base64::DecodeError,
    },
    #[error("failed to get blob {sha} in repository {repo_id}: {source}")]
    GitHubGetBlob {
        repo_id: u64,
        sha: String,
        #[source]
        source: Box<octocrab::Error>,
    },
    #[error("failed to get commit for {ref_name:?} in repository {repo_id}: {source}")]
    GitHubGetCommit {
        repo_id: u64,
        ref_name: String,
        #[source]
        source: octocrab::Error,
    },
    #[error("failed to get contents at {path:?} in repository {repo_id}: {source}")]
    GitHubGetContent {
        repo_id: u64,
        path: String,
        #[source]
        source: Box<octocrab::Error>,
    },
    #[error("failed to get tree for {ref_name:?} in repository {repo_id}: {source}")]
    GitHubGetTree {
        repo_id: u64,
        ref_name: String,
        #[source]
        source: Box<octocrab::Error>,
    },
    #[error("failed to get repository {repo_id}: {source}")]
    GitHubGetRepository {
        repo_id: u64,
        #[source]
        source: octocrab::Error,
    },
    #[error("failed to create client for installation {installation_id}: {source}")]
    GitHubInstallation {
        installation_id: u64,
        #[source]
        source: octocrab::Error,
    },
    #[error("content path {path:?} in repository {repo_id} is outside directory {directory:?}")]
    GitHubInvalidContentPath {
        repo_id: u64,
        path: String,
        directory: String,
    },
    #[error("failed to list installation repositories: {0}")]
    GitHubListRepositories(#[source] octocrab::Error),
    #[error("content for {path:?} in repository {repo_id} is missing")]
    GitHubMissingContent { repo_id: u64, path: String },
    #[error("repository {repo_id} was not found: {source}")]
    GitHubRepositoryNotFound {
        repo_id: u64,
        #[source]
        source: octocrab::Error,
    },
    #[error("directory listing for repository {repo_id} at {ref_name:?} was truncated by GitHub")]
    GitHubTreeTruncated { repo_id: u64, ref_name: String },
    #[error("directory {directory:?} in repository {repo_id} contains more than {max_files} files")]
    GitHubTooManyFiles {
        repo_id: u64,
        directory: String,
        max_files: usize,
    },
    #[error("unsupported content encoding {encoding:?} at {path:?} in repository {repo_id}")]
    GitHubUnsupportedContentEncoding {
        repo_id: u64,
        path: String,
        encoding: String,
    },
    #[error("unsupported content type {content_type:?} at {path:?} in repository {repo_id}")]
    GitHubUnsupportedContentType {
        repo_id: u64,
        path: String,
        content_type: String,
    },

    // Webhook HTTP handling and secret retrieval.
    #[error("Http error: {0}")]
    Http(#[from] http::Error),
    #[error("Invalid header name: {0}")]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Invalid header value string: {0}")]
    InvalidHeaderValueStr(#[from] ToStrError),
    #[error("Refresh throttled")]
    RefreshThrottled,
    #[error("Secrets unavailable; retry after {retry_after:?}")]
    SecretsUnavailable { retry_after: Duration },
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Signing params error: {0}")]
    SigningParams(#[from] SigningParamsError),
    #[error("Signing error: {0}")]
    Signing(#[from] SigningError),

    // Deadlines and operation timeouts.
    #[error("{operation} timed out")]
    TimeoutOperation { operation: &'static str },
    #[error("deadline reached before operation could start")]
    TimeoutDeadlineExceeded,
}

impl BpError {
    pub fn config_missing(name: impl Into<String>) -> Self {
        Self::ConfigMissing { name: name.into() }
    }

    pub fn config_empty(name: impl Into<String>) -> Self {
        Self::ConfigEmpty { name: name.into() }
    }

    pub fn config_not_unicode(name: impl Into<String>) -> Self {
        Self::ConfigNotUnicode { name: name.into() }
    }

    pub fn config_invalid(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConfigInvalid {
            name: name.into(),
            message: message.into(),
            source: None,
        }
    }

    pub fn config_invalid_source(
        name: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::ConfigInvalid {
            name: name.into(),
            message: source.to_string(),
            source: Some(Box::new(source)),
        }
    }

    pub fn io<E>(operation: &'static str, path: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        Self::Io {
            operation,
            path: path.into(),
            source: source.into(),
        }
    }

    pub fn s3<E>(operation: &'static str, target: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        Self::S3 {
            operation,
            target: target.into(),
            source: source.into(),
        }
    }

    pub fn s3_request<E>(operation: &'static str, target: impl Into<String>, source: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        Self::S3Request {
            operation,
            target: target.into(),
            source: source.into(),
        }
    }

    pub fn exchange<E>(operation: &'static str, source: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        Self::Exchange {
            operation,
            source: source.into(),
        }
    }

    pub fn candle_conversion<E>(field: &'static str, source: E) -> Self
    where
        E: Into<Box<dyn StdError + Send + Sync>>,
    {
        Self::CandleConversion {
            field,
            source: source.into(),
        }
    }

    pub fn external(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::External(Box::new(source))
    }

    pub fn send_not_sent(source: Self) -> Self {
        Self::Send {
            failure: SendFailure::NotSent,
            source: Box::new(source),
        }
    }

    pub fn send_not_sent_external(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Send {
            failure: SendFailure::NotSent,
            source: Box::new(Self::external(source)),
        }
    }

    pub fn send_failed(source: Self) -> Self {
        Self::Send {
            failure: SendFailure::Failed,
            source: Box::new(source),
        }
    }

    pub fn send_unknown(source: Self) -> Self {
        Self::Send {
            failure: SendFailure::Unknown,
            source: Box::new(source),
        }
    }

    pub fn send_unknown_external(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Send {
            failure: SendFailure::Unknown,
            source: Box::new(Self::external(source)),
        }
    }

    pub fn multiple(failures: impl IntoIterator<Item = BpError>) -> Self {
        let failures = failures.into_iter().collect::<Vec<_>>();
        let count = failures.len();
        let failures = failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        Self::Multiple { count, failures }
    }

    pub fn is_skippable(&self) -> bool {
        matches!(
            self,
            Self::MissingSymbol { .. }
                | Self::UnsupportedExchange { .. }
                | Self::UnsupportedTradingMode { .. }
                | Self::LockContention { .. }
        )
    }

    pub fn any_sent(&self, total: usize) -> bool {
        matches!(self, Self::SqsPartialBatch { failures } if failures.len() < total)
    }

    pub fn is_client_error(&self) -> bool {
        matches!(self, Self::HostInvalid { .. } | Self::HostNotAllowed { .. })
    }

    pub fn was_not_sent(&self) -> bool {
        matches!(
            self,
            Self::Send {
                failure: SendFailure::NotSent,
                ..
            }
        )
    }

    pub fn has_unknown_response(&self) -> bool {
        matches!(
            self,
            Self::Send {
                failure: SendFailure::Unknown,
                ..
            }
        )
    }

    pub fn error_type(&self) -> &'static str {
        self.into()
    }

    pub fn is_retryable(&self) -> bool {
        self.retryability() == Retryability::Retryable
    }

    pub fn retryability(&self) -> Retryability {
        match self {
            Self::Io { .. }
            | Self::S3 { .. }
            | Self::S3Request { .. }
            | Self::S3DeleteObjects { .. }
            | Self::Exchange { .. }
            | Self::Arrow(_)
            | Self::Parquet(_)
            | Self::Task(_)
            | Self::ArchiveIo(_)
            | Self::Upload(_)
            | Self::External(_)
            | Self::GatewayHttpStatus { status: 408 | 429 }
            | Self::RateLimited
            | Self::TimedOut
            | Self::TimeoutOperation { .. }
            | Self::TimeoutDeadlineExceeded
            | Self::SecretsUnavailable { .. } => Retryability::Retryable,
            Self::GatewayHttpStatus { status } if *status >= HTTP_INTERNAL_SERVER_ERROR => {
                Retryability::Retryable
            }
            Self::SqsPartialBatch { failures }
                if !failures.is_empty() && failures.iter().all(SqsBatchFailure::is_permanent) =>
            {
                Retryability::Permanent
            }
            Self::SqsPartialBatch { .. } => Retryability::Retryable,
            Self::LambdaSdkInvoke(error) => invoke_retryability(error),
            Self::Credentials(error) => credentials_retryability(error),
            Self::Reqwest(error) => reqwest_retryability(error),
            Self::S3PresignOperation { retryable, .. } => {
                if *retryable {
                    Retryability::Retryable
                } else {
                    Retryability::Permanent
                }
            }
            Self::StorageUploadRejected { status, .. } => http_status_retryability(*status),
            Self::Send { source, .. } => match source.as_ref() {
                Self::TimedOut | Self::External(_) => Retryability::Retryable,
                Self::GatewayHttpStatus { status } => http_status_retryability(*status),
                _ => Retryability::Permanent,
            },
            Self::GitHubGetCommit { source, .. }
            | Self::GitHubGetRepository { source, .. }
            | Self::GitHubInstallation { source, .. }
            | Self::GitHubListRepositories(source) => octocrab_retryability(source),
            Self::GitHubGetBlob { source, .. }
            | Self::GitHubGetContent { source, .. }
            | Self::GitHubGetTree { source, .. } => octocrab_retryability(source),
            Self::Multiple { .. } => Retryability::Permanent,
            _ => Retryability::Permanent,
        }
    }

    fn format_batch_failures(failures: &[SqsBatchFailure]) -> String {
        failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl From<BpError> for Diagnostic {
    fn from(value: BpError) -> Self {
        Diagnostic {
            error_type: value.error_type().into(),
            error_message: value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SendFailure {
    Failed,
    NotSent,
    Unknown,
}

#[derive(Debug)]
pub struct SqsBatchFailure {
    pub source: Option<SqsMessage>,
    pub code: String,
    pub detail: Option<String>,
}

impl SqsBatchFailure {
    pub fn is_permanent(&self) -> bool {
        Retryability::sqs_batch_failure_retryability(&self.code) == Retryability::Permanent
    }
}

impl std::fmt::Display for SqsBatchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "source={:?} code={:?} detail={:?}",
            self.source, self.code, self.detail
        )
    }
}

fn http_status_retryability(status: u16) -> Retryability {
    match status {
        HTTP_REQUEST_TIMEOUT | HTTP_TOO_MANY_REQUESTS => Retryability::Retryable,
        status if status >= HTTP_INTERNAL_SERVER_ERROR => Retryability::Retryable,
        status if (400..500).contains(&status) => Retryability::Permanent,
        _ => Retryability::Retryable,
    }
}

fn credentials_retryability(error: &CredentialsError) -> Retryability {
    match error {
        CredentialsError::ProviderTimedOut(_) | CredentialsError::ProviderError(_) => {
            Retryability::Retryable
        }
        _ => Retryability::Permanent,
    }
}

fn reqwest_retryability(error: &reqwest::Error) -> Retryability {
    match error.status().map(|status| status.as_u16()) {
        Some(status) => http_status_retryability(status),
        None => Retryability::Retryable,
    }
}

fn octocrab_retryability(error: &octocrab::Error) -> Retryability {
    match error {
        octocrab::Error::GitHub { source, .. } => github_retryability(
            source.status_code.as_u16(),
            &source.message,
            source.documentation_url.as_deref(),
        ),
        octocrab::Error::Installation { .. }
        | octocrab::Error::InstallationTokenInvalidAuth { .. }
        | octocrab::Error::InvalidHeaderValue { .. }
        | octocrab::Error::InvalidUtf8 { .. }
        | octocrab::Error::Json { .. }
        | octocrab::Error::JWT { .. }
        | octocrab::Error::Serde { .. }
        | octocrab::Error::SerdeUrlEncoded { .. }
        | octocrab::Error::Uri { .. }
        | octocrab::Error::UriParse { .. } => Retryability::Permanent,
        _ => Retryability::Retryable,
    }
}

fn github_retryability(
    status: u16,
    message: &str,
    documentation_url: Option<&str>,
) -> Retryability {
    match status {
        HTTP_FORBIDDEN if is_rate_limit_error(message, documentation_url) => {
            Retryability::Retryable
        }
        HTTP_REQUEST_TIMEOUT | HTTP_TOO_MANY_REQUESTS => Retryability::Retryable,
        status if status >= HTTP_INTERNAL_SERVER_ERROR => Retryability::Retryable,
        status if (400..500).contains(&status) => Retryability::Permanent,
        _ => Retryability::Retryable,
    }
}

fn is_rate_limit_error(message: &str, documentation_url: Option<&str>) -> bool {
    mentions_rate_limit(message) || documentation_url.is_some_and(mentions_rate_limit)
}

fn mentions_rate_limit(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("rate limit")
        || value.contains("rate-limit")
        || value.contains("rate_limit")
        || value.contains("ratelimit")
}

fn invoke_retryability(error: &SdkInvokeError) -> Retryability {
    match error.code() {
        Some(
            "InvalidParameterValueException"
            | "InvalidRequestContentException"
            | "RecursiveInvocationException"
            | "ResourceConflictException"
            | "ResourceNotFoundException"
            | "UnsupportedMediaTypeException",
        ) => Retryability::Permanent,
        _ => Retryability::Retryable,
    }
}
