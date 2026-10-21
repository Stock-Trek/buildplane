use crate::error::{ETError, ETResult};
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
        let value = match std::env::var(&env_var_name) {
            Ok(value) => value,
            Err(VarError::NotPresent) => self.default(protocol, trading_mode).to_owned(),
            Err(VarError::NotUnicode(value)) => {
                return Err(ETError::BadValue {
                    name: env_var_name,
                    value: value.to_string_lossy().into_owned(),
                    reason: "Environment variable is not valid UTF-8".into(),
                });
            }
        };
        validate_url(&env_var_name, &value, protocol)?;
        Ok(value)
    }
}

fn validate_url(name: &str, value: &str, protocol: Protocol) -> ETResult<()> {
    let url = Url::parse(value).map_err(|error| ETError::BadValue {
        name: name.into(),
        value: value.into(),
        reason: error.to_string(),
    })?;
    let expected_schemes = match protocol {
        Protocol::Http => "http, https",
        Protocol::Websocket => "ws, wss",
    };
    let valid_scheme = match protocol {
        Protocol::Http => matches!(url.scheme(), "http" | "https"),
        Protocol::Websocket => matches!(url.scheme(), "ws" | "wss"),
    };
    if !valid_scheme {
        return Err(ETError::BadValue {
            name: name.into(),
            value: value.into(),
            reason: format!(
                "URL scheme must be one of {expected_schemes}, got {}",
                url.scheme()
            ),
        });
    }
    Ok(())
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Websocket,
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
