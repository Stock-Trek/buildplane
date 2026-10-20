use serde::{Deserialize, Serialize};
use strum::Display;

pub trait Urls {
    fn default(&self, protocol: Protocol, trading_mode: TradingMode) -> &str;
    fn env_var_or_default(
        &self,
        prefix: &'static str,
        protocol: Protocol,
        trading_mode: TradingMode,
    ) -> String {
        let env_var_name = format!(
            "{}_{}_{}",
            prefix.to_uppercase(),
            protocol.to_string().to_uppercase(),
            trading_mode.to_string().to_uppercase()
        );
        std::env::var(env_var_name).unwrap_or_else(|_| self.default(protocol, trading_mode).into())
    }
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
