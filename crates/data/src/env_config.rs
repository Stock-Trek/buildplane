use crate::{
    error::{MduError, MduResult},
    s3::Bucket,
};
use chrono::{DateTime, NaiveDate, Utc};
use std::env;

const S3_BUCKET_ENV_VAR: &str = "S3_BUCKET";
const AWS_LAMBDA_FUNCTION_NAME_ENV_VAR: &str = "AWS_LAMBDA_FUNCTION_NAME";
const BACKFILL_START_ENV_VAR: &str = "BACKFILL_START";

pub struct EnvConfig {
    pub s3_bucket: Bucket,
    pub lambda_function_name: String,
    pub backfill_start: Option<DateTime<Utc>>,
}

impl EnvConfig {
    pub fn from_env() -> MduResult<Self> {
        Ok(Self {
            s3_bucket: Bucket::new(Self::required_env_var(S3_BUCKET_ENV_VAR)?),
            lambda_function_name: Self::required_env_var(AWS_LAMBDA_FUNCTION_NAME_ENV_VAR)?,
            backfill_start: Self::optional_timestamp_env_var(BACKFILL_START_ENV_VAR)?,
        })
    }

    fn required_env_var(name: &'static str) -> MduResult<String> {
        env::var(name)
            .map_err(|_| MduError::MissingEnvVar { name })
            .and_then(|value| {
                let value = value.trim();
                if value.is_empty() {
                    Err(MduError::EmptyEnvVar { name })
                } else {
                    Ok(value.to_string())
                }
            })
    }

    fn optional_env_var(name: &'static str) -> MduResult<Option<String>> {
        match env::var(name) {
            Ok(value) => {
                let value = value.trim();
                if value.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(value.to_string()))
                }
            }
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => Err(MduError::InvalidEnvVar { name }),
        }
    }

    fn optional_timestamp_env_var(name: &'static str) -> MduResult<Option<DateTime<Utc>>> {
        let Some(value) = Self::optional_env_var(name)? else {
            return Ok(None);
        };
        Self::parse_timestamp(name, value.trim()).map(Some)
    }

    fn parse_timestamp(name: &'static str, value: &str) -> MduResult<DateTime<Utc>> {
        if let Ok(date_time) = DateTime::parse_from_rfc3339(value) {
            return Ok(date_time.with_timezone(&Utc));
        }
        if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            let Some(date_time) = date.and_hms_opt(0, 0, 0) else {
                return Err(MduError::InvalidTimestamp {
                    name,
                    value: value.to_string(),
                });
            };
            return Ok(date_time.and_utc());
        }
        Err(MduError::InvalidTimestamp {
            name,
            value: value.to_string(),
        })
    }
}
