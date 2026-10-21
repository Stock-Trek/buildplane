use crate::{
    constants::{PARQUET_SUFFIX, common_trading_pairs},
    error::{MduError, MduResult},
    s3::ObjectKey,
};
use buildplane_exchange_types::urls::TradingMode;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::{Map, Value};
use stock_trek_types::cex::{cex_id::CexId, trading_pair::TradingPair};

/// The operation requested from the data Lambda.
///
/// The canonical wire format is an internally tagged object, for example:
///
/// ```json
/// { "mode": "backfill", "pair": "paper/binance/bitcoin/tetherusd",
///   "start": "2024-01-01T00:00:00Z", "end": "2024-01-02T00:00:00Z",
///   "force": true }
/// ```
///
/// An empty payload (and the scheduled EventBridge payload, which has no mode)
/// defaults to [`Job::Update`] so existing deployments keep their behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Job {
    #[serde(
        alias = "scheduled",
        alias = "scheduled_update",
        alias = "scheduled-update",
        alias = "update_all",
        alias = "update-all"
    )]
    #[default]
    Update,
    Backfill(BackfillJob),
    Compact(CompactJob),
}

impl Job {
    /// Parse a Lambda payload, accepting `mode`, `job`, or `action` as the tag.
    ///
    /// A payload without a mode is treated as the scheduled update so existing
    /// EventBridge invocations remain valid.
    pub fn from_value(value: Value) -> MduResult<Self> {
        let job = Self::parse_value(value)?;
        job.validate()?;
        Ok(job)
    }

    fn parse_value(value: Value) -> MduResult<Self> {
        match value {
            Value::Null => Ok(Self::Update),
            Value::String(mode) => Self::from_mode(&mode, Map::new()),
            Value::Object(mut object) => {
                let tag_key = ["mode", "job", "action"]
                    .into_iter()
                    .find(|key| object.contains_key(*key));
                let Some(tag_key) = tag_key else {
                    return Ok(Self::Update);
                };
                let tag = object.remove(tag_key).unwrap_or(Value::Null);
                match tag {
                    Value::String(mode) => Self::from_mode(&mode, object),
                    Value::Object(nested) => {
                        if nested.len() != 1 {
                            return Err(MduError::InvalidJob {
                                message: "a nested job mode must contain exactly one entry"
                                    .to_string(),
                            });
                        }
                        let (mode, payload) = nested.into_iter().next().unwrap();
                        match payload {
                            Value::Object(payload) => {
                                object.extend(payload);
                                Self::from_mode(&mode, object)
                            }
                            Value::Null => Self::from_mode(&mode, object),
                            _ => Err(MduError::InvalidJob {
                                message: format!(
                                    "nested mode {mode} must contain an object payload"
                                ),
                            }),
                        }
                    }
                    other => Err(MduError::InvalidJob {
                        message: format!("job mode must be a string, got {other}"),
                    }),
                }
            }
            other => Err(MduError::InvalidJob {
                message: format!("job payload must be an object, got {other}"),
            }),
        }
    }

    pub fn mode(&self) -> &'static str {
        match self {
            Self::Update => "update",
            Self::Backfill(_) => "backfill",
            Self::Compact(_) => "compact",
        }
    }

    /// Validate the job before any S3 or exchange work starts.
    pub fn validate(&self) -> MduResult<()> {
        match self {
            Self::Update => Ok(()),
            Self::Backfill(backfill) => backfill.validate(),
            Self::Compact(compact) => compact.prefix().map(|_| ()),
        }
    }

    fn from_mode(mode: &str, object: Map<String, Value>) -> MduResult<Self> {
        let mode = mode.trim().to_ascii_lowercase().replace('-', "_");
        let payload = Value::Object(object);
        match mode.as_str() {
            "update" | "scheduled" | "scheduled_update" | "update_all" => Ok(Self::Update),
            "backfill" => serde_json::from_value(payload)
                .map(Self::Backfill)
                .map_err(|error| MduError::InvalidJob {
                    message: format!("invalid backfill job: {error}"),
                }),
            "compact" => serde_json::from_value(payload)
                .map(Self::Compact)
                .map_err(|error| MduError::InvalidJob {
                    message: format!("invalid compact job: {error}"),
                }),
            other => Err(MduError::InvalidJob {
                message: format!("unknown job mode {other}; expected update, backfill, or compact"),
            }),
        }
    }
}

/// A targeted backfill for one trading pair and time range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackfillJob {
    #[serde(alias = "trading_pair", alias = "pair_path")]
    pub pair: TradingPairPath,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub start: DateTime<Utc>,
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub end: DateTime<Utc>,
    /// Re-fetch and overwrite windows that already have data.
    ///
    /// Compacted periods are still skipped; rebuilding a compacted roll-up is
    /// a separate compaction concern.
    #[serde(default, alias = "overwrite", alias = "repair")]
    pub force: bool,
}

impl BackfillJob {
    pub fn new(
        pair: impl Into<TradingPairPath>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        force: bool,
    ) -> Self {
        Self {
            pair: pair.into(),
            start,
            end,
            force,
        }
    }

    pub fn validate(&self) -> MduResult<()> {
        if self.start >= self.end {
            return Err(MduError::InvalidBackfillRange {
                start: self.start,
                end: self.end,
            });
        }
        self.pair.resolve().map(|_| ())
    }

    pub fn resolve(&self) -> MduResult<ResolvedTradingPair> {
        self.pair.resolve()
    }
}

/// A targeted compaction for one prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactJob {
    #[serde(alias = "prefix", alias = "path")]
    pub target: String,
}

impl CompactJob {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }

    /// Normalize the target into the prefix expected by the compactor.
    pub fn prefix(&self) -> MduResult<ObjectKey> {
        let target = self.target.trim();
        if target.is_empty() {
            return Err(MduError::InvalidJob {
                message: "compact target cannot be empty".to_string(),
            });
        }
        let prefix = if target.ends_with('/') {
            target.to_string()
        } else if target.ends_with(PARQUET_SUFFIX) {
            let (prefix, _) = target
                .rsplit_once('/')
                .ok_or_else(|| MduError::InvalidJob {
                    message: format!("compact file target {target} has no parent prefix"),
                })?;
            format!("{prefix}/")
        } else {
            format!("{target}/")
        };
        Ok(ObjectKey::new(prefix))
    }
}

/// A trading-pair path such as `paper/binance/bitcoin/tetherusd`.
///
/// It can be supplied as a string or as an object with `trading_mode`, `cex_id`,
/// `base`, and `quote` fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TradingPairPath(String);

impl TradingPairPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn resolve(&self) -> MduResult<ResolvedTradingPair> {
        let normalized = self.0.trim().trim_matches('/').replace([':', '\\'], "/");
        if normalized.is_empty() {
            return Err(MduError::InvalidTradingPairPath {
                pair: self.0.clone(),
            });
        }
        for (cex_id, trading_mode, trading_pairs) in common_trading_pairs() {
            for trading_pair in trading_pairs {
                let candidate = format!(
                    "{trading_mode}/{cex_id}/{}/{}",
                    trading_pair.base, trading_pair.quote
                );
                if candidate.eq_ignore_ascii_case(&normalized) {
                    return Ok(ResolvedTradingPair {
                        cex_id,
                        trading_mode,
                        trading_pair,
                    });
                }
            }
        }
        Err(MduError::UnsupportedTradingPair {
            pair: self.0.clone(),
        })
    }
}

impl From<&str> for TradingPairPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl From<String> for TradingPairPath {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

impl Serialize for TradingPairPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for TradingPairPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum TradingPairPathRepr {
            Path(String),
            Parts {
                #[serde(alias = "mode", alias = "tradingMode")]
                trading_mode: String,
                #[serde(
                    alias = "exchange",
                    alias = "cex",
                    alias = "cexId",
                    alias = "exchange_id"
                )]
                cex_id: String,
                #[serde(alias = "base_asset", alias = "baseAsset")]
                base: String,
                #[serde(alias = "quote_asset", alias = "quoteAsset")]
                quote: String,
            },
        }

        match TradingPairPathRepr::deserialize(deserializer)? {
            TradingPairPathRepr::Path(path) => Ok(Self::new(path)),
            TradingPairPathRepr::Parts {
                trading_mode,
                cex_id,
                base,
                quote,
            } => Ok(Self::new(format!("{trading_mode}/{cex_id}/{base}/{quote}"))),
        }
    }
}

/// A configured trading pair resolved from a [`TradingPairPath`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTradingPair {
    pub cex_id: CexId,
    pub trading_mode: TradingMode,
    pub trading_pair: TradingPair,
}

/// Summary of the work performed by a job.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct JobOutcome {
    pub windows_ingested: usize,
    pub compaction_targets: usize,
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if let Ok(date_time) = DateTime::parse_from_rfc3339(&value) {
        return Ok(date_time.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|date_time| date_time.and_utc())
            .ok_or_else(|| de::Error::custom(format!("invalid date {value}")));
    }
    Err(de::Error::custom(format!(
        "expected an RFC 3339 timestamp or YYYY-MM-DD date, got {value}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn utc(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, 0, 0, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn empty_payload_defaults_to_scheduled_update() {
        assert_eq!(Job::from_value(json!({})).unwrap(), Job::Update);
        assert_eq!(Job::from_value(Value::Null).unwrap(), Job::Update);
    }

    #[test]
    fn scheduled_aliases_are_accepted() {
        for mode in ["update", "scheduled", "scheduled-update", "update_all"] {
            assert_eq!(
                Job::from_value(json!({ "mode": mode })).unwrap(),
                Job::Update
            );
        }
    }

    #[test]
    fn backfill_is_parsed_with_force_aliases() {
        let job = Job::from_value(json!({
            "mode": "backfill",
            "pair": "paper/binance/bitcoin/tetherusd",
            "start": "2024-01-01",
            "end": "2024-01-02T00:00:00Z",
            "overwrite": true
        }))
        .unwrap();
        let Job::Backfill(backfill) = job else {
            panic!("expected a backfill job");
        };
        assert_eq!(backfill.pair.as_str(), "paper/binance/bitcoin/tetherusd");
        assert_eq!(backfill.start, utc(2024, 1, 1));
        assert_eq!(backfill.end, utc(2024, 1, 2));
        assert!(backfill.force);
        backfill.validate().unwrap();
    }

    #[test]
    fn trading_pair_path_accepts_structured_parts() {
        let job = Job::from_value(json!({
            "mode": "backfill",
            "pair": {
                "trading_mode": "Paper",
                "exchange": "Binance",
                "base": "Bitcoin",
                "quote": "TetherUSD"
            },
            "start": "2024-01-01T00:00:00Z",
            "end": "2024-01-01T01:00:00Z"
        }))
        .unwrap();
        let Job::Backfill(backfill) = job else {
            panic!("expected a backfill job");
        };
        let resolved = backfill.resolve().unwrap();
        assert_eq!(resolved.trading_mode, TradingMode::Paper);
        assert_eq!(resolved.cex_id, CexId::Binance);
    }

    #[test]
    fn nested_mode_form_is_accepted() {
        let job = Job::from_value(json!({
            "mode": {
                "compact": {
                    "target": "paper/binance/bitcoin/tetherusd/2024/01/01/"
                }
            }
        }))
        .unwrap();
        let Job::Compact(compact) = job else {
            panic!("expected a compact job");
        };
        assert_eq!(
            compact.prefix().unwrap().as_str(),
            "paper/binance/bitcoin/tetherusd/2024/01/01/"
        );
    }

    #[test]
    fn compact_prefix_is_normalized() {
        let compact = CompactJob::new("paper/binance/bitcoin/tetherusd");
        assert_eq!(
            compact.prefix().unwrap().as_str(),
            "paper/binance/bitcoin/tetherusd/"
        );
        let compact = CompactJob::new("paper/binance/bitcoin/tetherusd/2024/01/01/00.parquet");
        assert_eq!(
            compact.prefix().unwrap().as_str(),
            "paper/binance/bitcoin/tetherusd/2024/01/01/"
        );
    }

    #[test]
    fn invalid_modes_and_ranges_are_rejected() {
        assert!(Job::from_value(json!({ "mode": "unknown" })).is_err());
        assert!(
            Job::from_value(json!({
                "mode": "backfill",
                "pair": "paper/binance/bitcoin/tetherusd",
                "start": "2024-01-02",
                "end": "2024-01-01"
            }))
            .is_err()
        );
        assert!(
            Job::from_value(json!({
                "mode": "backfill",
                "pair": "paper/binance/bitcoin/tetherusd",
                "start": "2024-01-01",
                "end": "2024-01-02"
            }))
            .unwrap()
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn unsupported_pair_is_rejected() {
        let pair = TradingPairPath::new("paper/binance/dogecoin/tetherusd");
        assert!(pair.resolve().is_err());
    }
}
