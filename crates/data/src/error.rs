use arrow_schema::ArrowError;
use chrono::{DateTime, Utc};
use parquet::errors::ParquetError;
use std::error::Error as StdError;
use thiserror::Error;

pub type MduResult<T> = Result<T, MduError>;

#[derive(Debug, Error)]
pub enum MduError {
    #[error("Arrow error: {0}")]
    Arrow(#[from] ArrowError),
    #[error("Parquet error: {0}")]
    Parquet(#[from] ParquetError),
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
    #[error("Configuration error: {0}")]
    Config(#[from] buildplane_shared::errors::error::RsSharedError),
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
}

impl MduError {
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

    pub fn is_skippable(&self) -> bool {
        matches!(
            self,
            Self::MissingSymbol { .. }
                | Self::UnsupportedExchange { .. }
                | Self::UnsupportedTradingMode { .. }
        )
    }

    pub fn multiple(failures: impl IntoIterator<Item = MduError>) -> Self {
        let failures = failures.into_iter().collect::<Vec<_>>();
        let count = failures.len();
        let failures = failures
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        Self::Multiple { count, failures }
    }
}
