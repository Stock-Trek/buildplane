use crate::error::{ETError, ETResult};
use buildplane_shared::{env_vars::EnvVarConfig, errors::error::RsSharedError};
use serde::{Deserialize, Serialize};
use std::env::VarError;
use strum::Display;
use url::Url;

pub trait Urls {
    fn default(&self, protocol: Protocol, trading_mode: TradingMode) -> &str;
    fn env_var_or_default(
        &self,
        prefix: &'static str,
        protocol: Protocol,
        trading_mode: TradingMode,
    ) -> ETResult<String> {
        let env_var_name = format!(
            "{}_{}_{}",
            prefix.to_uppercase(),
            protocol.to_string().to_uppercase(),
            trading_mode.to_string().to_uppercase()
        );
        let default = self.default(protocol, trading_mode);
        let value = EnvVarConfig::from_env()
            .env_var(&env_var_name)
            .map_err(|error| {
                let value = match &error {
                    RsSharedError::EnvVar(error) => match error.as_ref() {
                        VarError::NotUnicode(value) => value.to_string_lossy().into_owned(),
                        VarError::NotPresent => default.to_owned(),
                    },
                    _ => default.to_owned(),
                };
                ETError::BadValue {
                    name: env_var_name.clone(),
                    value,
                    reason: error.to_string(),
                }
            })?
            .unwrap_or_else(|| default.to_owned());
        protocol.validate_url(&env_var_name, &value)?;
        Ok(value)
    }
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Websocket,
}

impl Protocol {
    fn allowed_schemes(self) -> &'static [&'static str] {
        match self {
            Self::Http => &["http", "https"],
            Self::Websocket => &["ws", "wss"],
        }
    }

    fn validate_url(self, name: &str, value: &str) -> ETResult<()> {
        let url = Url::parse(value).map_err(|error| ETError::BadValue {
            name: name.into(),
            value: value.into(),
            reason: error.to_string(),
        })?;
        let allowed_schemes = self.allowed_schemes();
        if !allowed_schemes.contains(&url.scheme()) {
            return Err(ETError::BadValue {
                name: name.into(),
                value: value.into(),
                reason: format!(
                    "URL scheme must be one of {}, got {}",
                    allowed_schemes.join(", "),
                    url.scheme()
                ),
            });
        }
        Ok(())
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
