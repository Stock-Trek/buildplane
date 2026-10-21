use crate::{
    constants::{UPDATE_QUANTIZATION, common_trading_pairs},
    error::{MduError, MduResult},
    ingest::{Ingest, WindowIngest},
    plan,
    quantized_time::QuantizedTime,
    s3::{Bucket, ObjectKey, S3},
    updaters::{binance_updater::BinanceUpdater, exchange_updater::ExchangeUpdater},
};
use arrow_schema::SchemaRef;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3_transfer_manager::Client as TransferClient;
use buildplane_exchange_types::urls::TradingMode;
use chrono::{DateTime, Utc};
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
        let schema = Ingest::schema();
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
        for window in plan::Planner::plan_windows(&existing, *end_time, *backfill_start)? {
            Ingest::ingest_window(
                transfer_client,
                bucket,
                updater,
                WindowIngest {
                    trading_pair,
                    window,
                    candles_base_path: &candles_base_path,
                    schema,
                },
            )
            .await?;
        }
        Ok(())
    }
}
