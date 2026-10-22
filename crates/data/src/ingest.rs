use crate::{
    parquet_writer::writer_properties,
    plan::Window,
    s3::{Bucket, ObjectKey, S3},
    updaters::exchange_updater::ExchangeUpdater,
};
use arrow_array::{ArrayRef, Float64Array, Int64Array, RecordBatch};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use buildplane_exchange_types::klines::Kline;
use buildplane_shared::errors::{BpError, BpResult};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use std::sync::Arc;
use stock_trek_types::cex::trading_pair::TradingPair;
use tracing::warn;

pub struct Ingest;

pub struct WindowIngest<'a> {
    pub trading_pair: &'a TradingPair,
    pub window: Window,
    pub candles_base_path: &'a str,
    pub schema: &'a SchemaRef,
}

impl Ingest {
    pub async fn ingest_window(
        transfer_client: &TransferClient,
        bucket: &Bucket,
        updater: &dyn ExchangeUpdater,
        ingest: WindowIngest<'_>,
    ) -> BpResult<()> {
        let WindowIngest {
            trading_pair,
            window,
            candles_base_path,
            schema,
        } = ingest;
        let candles = updater
            .klines(trading_pair, window.start, window.end)
            .await?;
        if !Self::candles_are_complete(&candles, window.start, window.end) {
            warn!(
                "incomplete klines for {:?} between {} and {}: expected {} candles, received {}; skipping upload",
                trading_pair,
                window.start,
                window.end,
                Self::expected_candles(window.start, window.end),
                candles.len()
            );
            return Ok(());
        }
        let candles_bytes = Self::candles_to_bytes(&candles, schema)?;
        let candles_file_path =
            ObjectKey::new(format!("{}/{}", candles_base_path, window.file_path()));
        S3::upload_bytes(transfer_client, bucket, &candles_file_path, candles_bytes).await
    }

    pub fn schema() -> SchemaRef {
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

    fn candle_field_to_f64<T>(field: &'static str, value: T) -> BpResult<f64>
    where
        T: TryInto<f64>,
        T::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        value
            .try_into()
            .map_err(|source| BpError::candle_conversion(field, source))
    }

    fn candles_to_bytes(candles: &[Kline], schema: &SchemaRef) -> BpResult<Bytes> {
        let open = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("open", k.open))
            .collect::<BpResult<Vec<_>>>()?;
        let high = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("high", k.high))
            .collect::<BpResult<Vec<_>>>()?;
        let low = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("low", k.low))
            .collect::<BpResult<Vec<_>>>()?;
        let close = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("close", k.close))
            .collect::<BpResult<Vec<_>>>()?;
        let volume = candles
            .iter()
            .map(|k| Self::candle_field_to_f64("volume", k.volume))
            .collect::<BpResult<Vec<_>>>()?;
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
        Ok(Bytes::from(buffer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn expected_candles_are_one_per_minute() {
        assert_eq!(
            Ingest::expected_candles(utc(2024, 1, 1, 0, 0), utc(2024, 1, 1, 0, 6)),
            6
        );
    }

    #[test]
    fn empty_window_expects_no_candles() {
        assert_eq!(
            Ingest::expected_candles(utc(2024, 1, 1, 0, 6), utc(2024, 1, 1, 0, 0)),
            0
        );
    }
}
