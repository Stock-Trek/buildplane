use buildplane_config::env_vars::EnvVarConfig;
use buildplane_errors::{BpError, ConfigError};
use buildplane_exchange_types::{
    binance::urls::BinanceUrls,
    urls::{Protocol, TradingMode, Urls},
};

#[test]
fn missing_override_uses_default_url() {
    let prefix = "BP_URL_TEST_MISSING";
    let config = EnvVarConfig::from_map(Vec::<(String, String)>::new()).unwrap();

    let url = BinanceUrls
        .env_var_or_default_with(&config, prefix, Protocol::Http, TradingMode::Real)
        .unwrap();

    assert_eq!(url, "https://api.binance.com/api/v3");
}

#[test]
fn valid_override_is_returned_unchanged() {
    let prefix = "BP_URL_TEST_VALID";
    let env_var = format!("{prefix}_WEBSOCKET_PAPER").to_uppercase();
    let expected = "wss://example.test/ws-api/v3";
    let config = EnvVarConfig::from_map([(&env_var, expected)]).unwrap();

    let url = BinanceUrls
        .env_var_or_default_with(&config, prefix, Protocol::Websocket, TradingMode::Paper)
        .unwrap();

    assert_eq!(url, expected);
}

#[test]
fn invalid_override_is_rejected() {
    let prefix = "BP_URL_TEST_INVALID";
    let env_var = format!("{prefix}_HTTP_REAL").to_uppercase();
    let result = EnvVarConfig::from_map([(&env_var, "not a url")]);

    assert!(result.is_err());
}

#[test]
fn override_with_wrong_scheme_is_rejected() {
    let prefix = "BP_URL_TEST_SCHEME";
    let env_var = format!("{prefix}_WEBSOCKET_REAL").to_uppercase();
    let result = EnvVarConfig::from_map([(&env_var, "https://example.test/ws-api/v3")]);

    assert!(result.is_err());
}

#[test]
#[cfg(unix)]
fn non_unicode_override_is_rejected_by_constructor() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let prefix = "BP_URL_TEST_NOT_UNICODE";
    let env_var = format!("{prefix}_HTTP_REAL").to_uppercase();
    let configured = OsString::from_vec(b"https://configured.example/\xff".to_vec());
    let error = EnvVarConfig::from_map([(OsString::from(&env_var), configured)]).unwrap_err();

    assert!(matches!(
        &error,
        BpError::Config(ConfigError::NotUnicode { name }) if name == &env_var
    ));
    assert!(!error.to_string().contains("configured.example"));
}

struct InvalidDefaultUrls;

impl Urls for InvalidDefaultUrls {
    fn default(&self, _protocol: Protocol, _trading_mode: TradingMode) -> &str {
        "not a url"
    }
}

#[test]
fn invalid_default_url_is_rejected() {
    let prefix = "BP_URL_TEST_BAD_DEFAULT";
    let config = EnvVarConfig::from_map(Vec::<(String, String)>::new()).unwrap();

    let result = InvalidDefaultUrls.env_var_or_default_with(
        &config,
        prefix,
        Protocol::Http,
        TradingMode::Real,
    );

    assert!(result.is_err());
}
