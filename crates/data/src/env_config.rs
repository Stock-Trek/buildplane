use crate::s3::Bucket;
use buildplane_shared::env_vars::EnvVarConfig;
use buildplane_shared::errors::BpResult;
use chrono::{DateTime, Utc};

const DEFAULT_LAMBDA_FUNCTION_NAME: &str = "buildplane-data";

pub struct EnvConfig {
    pub s3_bucket: Bucket,
    pub lambda_function_name: String,
    pub backfill_start: Option<DateTime<Utc>>,
}

impl EnvConfig {
    pub fn from_shared(config: &EnvVarConfig) -> BpResult<Self> {
        Ok(Self {
            s3_bucket: Bucket::new(config.s3_bucket()?),
            lambda_function_name: config.lambda_function_name(DEFAULT_LAMBDA_FUNCTION_NAME)?,
            backfill_start: config.backfill_start()?,
        })
    }
}
