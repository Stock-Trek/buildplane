use crate::{
    constants::{PARQUET_SUFFIX, UPDATE_QUANTIZATION, common_trading_pairs},
    error::{MduError, MduResult},
    parquet_writer::writer_properties,
    quantized_time::{Quantization, QuantizedTime, TopOfHour, TopOfMonth},
    s3::{Bucket, ObjectKey, S3},
    subfolders::SubFolders,
    updaters::{binance_updater::BinanceUpdater, exchange_updater::ExchangeUpdater},
};
use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use buildplane_exchange_types::{klines::Kline, urls::TradingMode};
use bytes::Bytes;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use parquet::arrow::ArrowWriter;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use stock_trek_types::cex::{cex_id::CexId, trading_pair::TradingPair};
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{info, warn};

pub struct KlinesUpdater {
    binance: HashMap<TradingMode, Arc<dyn ExchangeUpdater>>,
}

struct ExchangeUpdate {
    cex_id: CexId,
    trading_mode: TradingMode,
    trading_pairs: Vec<TradingPair>,
    schema: SchemaRef,
    end_time: DateTime<Utc>,
    backfill_start: Option<DateTime<Utc>>,
    updater: Arc<dyn ExchangeUpdater>,
}

struct TradingPairUpdate {
    cex_id: CexId,
    trading_mode: TradingMode,
    trading_pair: TradingPair,
    schema: SchemaRef,
    end_time: DateTime<Utc>,
    backfill_start: Option<DateTime<Utc>>,
}

impl KlinesUpdater {
    pub fn try_new() -> MduResult<Self> {
        let mut binance: HashMap<TradingMode, Arc<dyn ExchangeUpdater>> = HashMap::new();
        for trading_mode in [TradingMode::Paper, TradingMode::Real] {
            binance.insert(
                trading_mode,
                Arc::new(BinanceUpdater::try_new(trading_mode)?),
            );
        }
        Ok(Self { binance })
    }

    pub async fn update(
        &self,
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        backfill_start: Option<DateTime<Utc>>,
    ) -> MduResult<()> {
        info!("update");
        let now = Utc::now();
        let schema = Self::create_schema();
        let end_time = QuantizedTime::timestamp(now, UPDATE_QUANTIZATION)?;
        let mut join_set = JoinSet::new();
        let mut errors = Vec::new();
        for (cex_id, trading_mode, trading_pairs) in common_trading_pairs() {
            let s3_client = s3_client.clone();
            let transfer_client = transfer_client.clone();
            let bucket = bucket.clone();
            let schema = schema.clone();
            let updater = match cex_id {
                CexId::Binance => match self.binance.get(&trading_mode).cloned() {
                    Some(updater) => updater,
                    None => {
                        let error = MduError::UnsupportedTradingMode {
                            trading_mode: format!("{trading_mode:?}"),
                        };
                        warn!(
                            "skipping exchange {:?} in trading mode {:?}: {error}",
                            cex_id, trading_mode
                        );
                        continue;
                    }
                },
                other => {
                    let error = MduError::UnsupportedExchange {
                        exchange: format!("{other:?}"),
                    };
                    warn!("skipping exchange {:?}: {error}", other);
                    continue;
                }
            };
            join_set.spawn(async move {
                let result = Self::update_exchange(
                    &s3_client,
                    &transfer_client,
                    &bucket,
                    ExchangeUpdate {
                        cex_id,
                        trading_mode,
                        trading_pairs,
                        schema,
                        end_time,
                        backfill_start,
                        updater,
                    },
                )
                .await;
                if let Err(error) = &result {
                    warn!(
                        "failed to update exchange {:?} in trading mode {:?}: {error}",
                        cex_id, trading_mode
                    );
                }
                result
            });
        }
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) if error.is_skippable() => {}
                Ok(Err(error)) => errors.push(error),
                Err(error) => errors.push(error.into()),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(MduError::multiple(errors))
        }
    }

    async fn update_exchange(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        update: ExchangeUpdate,
    ) -> MduResult<()> {
        let ExchangeUpdate {
            cex_id,
            trading_mode,
            trading_pairs,
            schema,
            end_time,
            backfill_start,
            updater,
        } = update;
        info!("update exchange {:?} @ end time {}", cex_id, end_time);
        if let Some(backfill_start) = backfill_start {
            info!("backfill start set to {}", backfill_start);
        }
        let semaphore = Arc::new(Semaphore::new(10));
        let mut join_set = JoinSet::new();
        let mut errors = Vec::new();
        for trading_pair in trading_pairs {
            info!("update symbol {:?}", trading_pair);
            let bucket = bucket.clone();
            let schema = schema.clone();
            let semaphore = semaphore.clone();
            let s3_client = s3_client.clone();
            let transfer_client = transfer_client.clone();
            let updater_clone = updater.clone();
            let update = TradingPairUpdate {
                cex_id,
                trading_mode,
                trading_pair,
                schema,
                end_time,
                backfill_start,
            };
            join_set.spawn(async move {
                let result = Self::update_pair(
                    &s3_client,
                    &transfer_client,
                    &bucket,
                    &semaphore,
                    updater_clone.as_ref(),
                    &update,
                )
                .await;
                if let Err(error) = &result {
                    if error.is_skippable() {
                        warn!(
                            "skipping unsupported trading pair {:?}: {error}",
                            update.trading_pair
                        );
                    } else {
                        warn!(
                            "failed to update trading pair {:?}: {error}",
                            update.trading_pair
                        );
                    }
                }
                result
            });
        }
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) if error.is_skippable() => {}
                Ok(Err(error)) => errors.push(error),
                Err(error) => errors.push(error.into()),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(MduError::multiple(errors))
        }
    }

    async fn update_pair(
        s3_client: &S3Client,
        transfer_client: &TransferClient,
        bucket: &Bucket,
        semaphore: &Semaphore,
        updater: &dyn ExchangeUpdater,
        update: &TradingPairUpdate,
    ) -> MduResult<()> {
        let TradingPairUpdate {
            cex_id,
            trading_mode,
            trading_pair,
            schema,
            end_time,
            backfill_start,
        } = update;
        let _permit = semaphore
            .acquire()
            .await
            .map_err(|_| MduError::SemaphoreClosed)?;
        let candles_base_path = format!(
            "{}/{}/{}/{}",
            trading_mode, cex_id, trading_pair.base, trading_pair.quote
        );
        let base_path_prefix = ObjectKey::new(format!("{}/", candles_base_path));
        let existing = S3::list_files(s3_client, bucket, &base_path_prefix)
            .await?
            .into_iter()
            .filter_map(|key| {
                key.as_str()
                    .strip_prefix(base_path_prefix.as_str())
                    .map(str::to_string)
            })
            .collect::<HashSet<_>>();
        for (start_time, window_end_time) in
            Self::missing_windows(&existing, *end_time, *backfill_start)?
        {
            let candles = updater
                .klines(trading_pair, start_time, window_end_time)
                .await?;
            if !Self::candles_are_complete(&candles, start_time, window_end_time) {
                warn!(
                    "incomplete klines for {:?} between {} and {}: expected {} candles, received {}; skipping upload",
                    trading_pair,
                    start_time,
                    window_end_time,
                    Self::expected_candles(start_time, window_end_time),
                    candles.len()
                );
                continue;
            }
            let candles_bytes = Self::candles_to_bytes(&candles, schema)?;
            let candles_relative_file_path = Self::window_file_paths(start_time).0;
            let candles_file_path = ObjectKey::new(format!(
                "{}/{}",
                candles_base_path, candles_relative_file_path
            ));
            S3::upload_bytes(transfer_client, bucket, &candles_file_path, candles_bytes).await?;
        }
        Ok(())
    }

    fn missing_windows(
        existing: &HashSet<String>,
        end_time: DateTime<Utc>,
        backfill_start: Option<DateTime<Utc>>,
    ) -> MduResult<Vec<(DateTime<Utc>, DateTime<Utc>)>> {
        let backfill_start = backfill_start
            .map(|start| QuantizedTime::timestamp(start, UPDATE_QUANTIZATION))
            .transpose()?;
        let existing_start = existing
            .iter()
            .filter_map(|path| Self::file_start_time(path.as_str()))
            .min()
            .map(|start| QuantizedTime::timestamp(start, UPDATE_QUANTIZATION))
            .transpose()?;
        let start = match (backfill_start, existing_start) {
            (Some(backfill_start), Some(existing_start)) => {
                Some(backfill_start.min(existing_start))
            }
            (Some(backfill_start), None) => Some(backfill_start),
            (None, Some(existing_start)) => Some(existing_start),
            (None, None) => None,
        };
        let Some(mut start) = start.filter(|start| *start < end_time) else {
            return Ok(vec![(
                QuantizedTime::previous_top(end_time, UPDATE_QUANTIZATION)?,
                end_time,
            )]);
        };
        let mut windows = Vec::new();
        while start < end_time {
            if let Some(boundary) = Self::compacted_boundary(existing, start)? {
                start = boundary;
                continue;
            }
            let window_end = QuantizedTime::next_top(start, UPDATE_QUANTIZATION)?;
            if window_end > end_time {
                break;
            }
            if !existing.contains(&Self::window_file_paths(start).0) {
                windows.push((start, window_end));
            }
            start = window_end;
        }
        Ok(windows)
    }

    fn compacted_boundary(
        existing: &HashSet<String>,
        start_time: DateTime<Utc>,
    ) -> MduResult<Option<DateTime<Utc>>> {
        let (_, compacted) = Self::window_file_paths(start_time);
        let next_period_start = if existing.contains(&compacted[3]) {
            Self::next_year(start_time)?
        } else if existing.contains(&compacted[2]) {
            Self::next_month(start_time)?
        } else if existing.contains(&compacted[1]) {
            Self::next_day(start_time)?
        } else if existing.contains(&compacted[0]) {
            Self::next_hour(start_time)?
        } else {
            return Ok(None);
        };
        // The compacted period may end between update windows for coarse
        // quantizations, so resume at the first window at or after its end.
        Ok(Some(QuantizedTime::top_at_or_after(
            next_period_start,
            UPDATE_QUANTIZATION,
        )?))
    }

    fn next_hour(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let hour_start = QuantizedTime::timestamp(timestamp, Quantization::Hour(TopOfHour::One))?;
        QuantizedTime::next_top(hour_start, Quantization::Hour(TopOfHour::One))
    }

    fn next_day(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let day_start = QuantizedTime::timestamp(timestamp, Quantization::Day)?;
        QuantizedTime::next_top(day_start, Quantization::Day)
    }

    fn next_month(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let month_start =
            QuantizedTime::timestamp(timestamp, Quantization::Month(TopOfMonth::One))?;
        QuantizedTime::next_top(month_start, Quantization::Month(TopOfMonth::One))
    }

    fn next_year(timestamp: DateTime<Utc>) -> MduResult<DateTime<Utc>> {
        let year_start = QuantizedTime::timestamp(timestamp, Quantization::Year)?;
        QuantizedTime::next_top(year_start, Quantization::Year)
    }

    fn window_file_paths(start_time: DateTime<Utc>) -> (String, [String; 4]) {
        let year = start_time.year();
        let month = start_time.month() as i32;
        let day = start_time.day() as i32;
        let hour = start_time.hour() as i32;
        let minute = start_time.minute() as i32;
        (
            SubFolders::file_path(&[year, month, day, hour, minute], PARQUET_SUFFIX),
            [
                SubFolders::file_path(&[year, month, day, hour], PARQUET_SUFFIX),
                SubFolders::file_path(&[year, month, day], PARQUET_SUFFIX),
                SubFolders::file_path(&[year, month], PARQUET_SUFFIX),
                SubFolders::file_path(&[year], PARQUET_SUFFIX),
            ],
        )
    }

    fn file_start_time(path: &str) -> Option<DateTime<Utc>> {
        let path = path.strip_suffix(PARQUET_SUFFIX)?;
        let mut segments = path.split('/');
        let year = segments.next()?.parse::<i32>().ok()?;
        let month = Self::file_path_segment(segments.next(), 1)?;
        let day = Self::file_path_segment(segments.next(), 1)?;
        let hour = Self::file_path_segment(segments.next(), 0)?;
        let minute = Self::file_path_segment(segments.next(), 0)?;
        if segments.next().is_some() {
            return None;
        }
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
    }

    fn file_path_segment(segment: Option<&str>, default: u32) -> Option<u32> {
        match segment {
            Some(segment) => segment.parse().ok(),
            None => Some(default),
        }
    }

    fn create_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("open_time", DataType::Int64, false),
            Field::new("open", DataType::Float64, false),
            Field::new("high", DataType::Float64, false),
            Field::new("low", DataType::Float64, false),
            Field::new("close", DataType::Float64, false),
            Field::new("volume", DataType::Float64, false),
        ]))
    }

    fn expected_candles(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> usize {
        (end_time - start_time).num_minutes().max(0) as usize
    }

    fn candles_are_complete(
        candles: &[Kline],
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> bool {
        if candles.len() != Self::expected_candles(start_time, end_time) {
            return false;
        }
        candles.iter().enumerate().all(|(index, kline)| {
            let expected_open_time = start_time + chrono::Duration::minutes(index as i64);
            kline.open_time.0 == expected_open_time.timestamp_millis()
        })
    }

    fn candle_field_to_f64<T>(field: &'static str, value: T) -> MduResult<f64>
    where
        T: TryInto<f64>,
        T::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        value
            .try_into()
            .map_err(|source| MduError::candle_conversion(field, source))
    }

    fn candles_to_bytes(candles: &[Kline], schema: &SchemaRef) -> MduResult<Bytes> {
        let open = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("open", k.open))
            .collect::<MduResult<Vec<_>>>()?;
        let high = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("high", k.high))
            .collect::<MduResult<Vec<_>>>()?;
        let low = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("low", k.low))
            .collect::<MduResult<Vec<_>>>()?;
        let close = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("close", k.close))
            .collect::<MduResult<Vec<_>>>()?;
        let volume = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("volume", k.volume))
            .collect::<MduResult<Vec<_>>>()?;
        let columns: Vec<ArrayRef> = vec![
            Arc::new(Int64Array::from_iter(candles.iter().map(|k| k.open_time.0))),
            Arc::new(Float64Array::from(open)),
            Arc::new(Float64Array::from(high)),
            Arc::new(Float64Array::from(low)),
            Arc::new(Float64Array::from(close)),
            Arc::new(Float64Array::from(volume)),
        ];
        let batch = RecordBatch::try_new(schema.clone(), columns)?;
        let mut buffer = Vec::new();
        let mut writer =
            ArrowWriter::try_new(&mut buffer, batch.schema(), Some(writer_properties()))?;
        writer.write(&batch)?;
        writer.close()?;
        let bytes = Bytes::from(buffer);
        Ok(bytes)
    }
}
