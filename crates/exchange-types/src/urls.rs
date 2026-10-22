use buildplane_shared::env_vars::EnvVarConfig;
use buildplane_shared::errors::{BpError, BpResult};
use buildplane_shared::errors::{ConfigError, ExchangeError};
pub use buildplane_shared::protocol::Protocol;
use serde::{Deserialize, Serialize};
use std::env::VarError;
use strum::Display;

pub trait Urls {
    fn default(&self, protocol: Protocol, trading_mode: TradingMode) -> &str;
    fn env_var_or_default(
        &self,
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
        let value =
            EnvVarConfig::from_env()
                .required(&env_var_name)
                .or_else(|error| match error {
                    BpError::Config(ConfigError::EnvVar(VarError::NotPresent)) => {
                        Ok(default.to_owned())
                    }
                    error => {
                        let value = match &error {
                            BpError::Config(ConfigError::EnvVar(VarError::NotUnicode(value))) => {
                                value.to_string_lossy().into_owned()
                            }
                            _ => default.to_owned(),
                        };
                        Err(BpError::Exchange(ExchangeError::BadValue {
                            name: env_var_name.clone(),
                            value,
                            reason: error.to_string(),
                        }))
                    }
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
