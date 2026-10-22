use crate::errors::BpError;
use serde::{Deserialize, Serialize};
use strum::Display;
use url::Url;

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

    pub fn validate_url(self, name: &str, value: &str) -> Result<(), BpError> {
        let url = Url::parse(value).map_err(|error| BpError::BadValue {
            name: name.into(),
            value: value.into(),
            reason: error.to_string(),
        })?;
        let allowed_schemes = self.allowed_schemes();
        if !allowed_schemes.contains(&url.scheme()) {
            return Err(BpError::BadValue {
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
