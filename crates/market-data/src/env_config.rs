use crate::s3::Bucket;
use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::BpResult;
use chrono::{DateTime, Utc};
use std::time::Duration;

const DEFAULT_LAMBDA_FUNCTION_NAME: &str = "buildplane-market-data";

#[derive(Clone)]
pub struct EnvConfig {
    pub s3_bucket: Bucket,
    pub lambda_function_name: String,
    pub backfill_start: Option<DateTime<Utc>>,
    pub deadline_buffer: Duration,
}

impl EnvConfig {
    pub fn from_shared(config: &EnvVarConfig) -> BpResult<Self> {
        Ok(Self {
            s3_bucket: Bucket::new(config.s3_bucket()?),
            lambda_function_name: config.lambda_function_name(DEFAULT_LAMBDA_FUNCTION_NAME)?,
            backfill_start: config.backfill_start()?,
            deadline_buffer: config.deadline_buffer()?,
        })
    }
}
