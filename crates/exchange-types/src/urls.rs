use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::errors::{BpError, BpResult};
use buildplane_errors::errors::{ConfigError, ExchangeError};
pub use buildplane_types::protocol::Protocol;
use serde::{Deserialize, Serialize};
use strum::Display;

pub trait Urls {
    fn default(&self, protocol: Protocol, trading_mode: TradingMode) -> &str;
    fn env_var_or_default(
        &self,
        prefix: &'static str,
        protocol: Protocol,
        trading_mode: TradingMode,
    ) -> BpResult<String> {
        self.env_var_or_default_with(&EnvVarConfig::from_env(), prefix, protocol, trading_mode)
    }

    fn env_var_or_default_with(
        &self,
        config: &EnvVarConfig,
        prefix: &'static str,
        protocol: Protocol,
        trading_mode: TradingMode,
    ) -> BpResult<String> {
        let env_var_name = format!(
            "{}_{}_{}",
            prefix.to_uppercase(),
            protocol.to_string().to_uppercase(),
            trading_mode.to_string().to_uppercase()
        );
        let default = self.default(protocol, trading_mode);
        let value = config
            .required(&env_var_name)
            .or_else(|error| match error {
                BpError::Config(ConfigError::Missing { .. }) => Ok(default.to_owned()),
                error => Err(BpError::Exchange(ExchangeError::BadValue {
                    name: env_var_name.clone(),
                    value: config
                        .required_lossy(&env_var_name)
                        .unwrap_or_else(|_| default.to_owned()),
                    reason: error.to_string(),
                })),
            })?;
        protocol.validate_url(&env_var_name, &value)?;
        Ok(value)
    }
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TradingMode {
    Real,
    Paper,
}

pub struct LocalhostUrls;

impl Urls for LocalhostUrls {
    fn default(&self, protocol: Protocol, _trading_mode: TradingMode) -> &str {
        match protocol {
            Protocol::Http => "http://localhost",
            Protocol::Websocket => "ws://localhost",
        }
    }
}
