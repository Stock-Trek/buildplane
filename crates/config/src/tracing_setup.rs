use crate::env_vars::EnvVarConfig;
use buildplane_errors::{BpError, BpResult};
use tracing_subscriber::{EnvFilter, fmt::Subscriber};

pub struct TracingSetup;

impl TracingSetup {
    pub fn setup(config: &EnvVarConfig) -> BpResult<()> {
        let filter = match &config.log_filter {
            Some(directives) => EnvFilter::try_new(directives)
                .map_err(|error| BpError::config_invalid_source(EnvFilter::DEFAULT_ENV, error))?,
            None => EnvFilter::new("info"),
        };
        let subscriber = Subscriber::builder()
            .with_ansi(false)
            .with_env_filter(filter)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
        Ok(())
    }
}
