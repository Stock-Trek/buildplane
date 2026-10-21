use buildplane_exchange_types::{
    binance::urls::BinanceUrls,
    urls::{Protocol, TradingMode, Urls},
};

#[test]
fn missing_override_uses_default_url() {
    let prefix = "BP_URL_TEST_MISSING";
    let env_var = format!("{prefix}_HTTP_REAL").to_uppercase();
    unsafe { std::env::remove_var(&env_var) };

    let url = BinanceUrls
        .env_var_or_default(prefix, Protocol::Http, TradingMode::Real)
        .unwrap();

    assert_eq!(url, "https://api.binance.com/api/v3");
}

#[test]
fn valid_override_is_returned_unchanged() {
    let prefix = "BP_URL_TEST_VALID";
    let env_var = format!("{prefix}_WEBSOCKET_PAPER").to_uppercase();
    let expected = "wss://example.test/ws-api/v3";
    unsafe { std::env::set_var(&env_var, expected) };

    let url = BinanceUrls
        .env_var_or_default(prefix, Protocol::Websocket, TradingMode::Paper)
        .unwrap();

    assert_eq!(url, expected);
}

#[test]
fn invalid_override_is_rejected() {
    let prefix = "BP_URL_TEST_INVALID";
    let env_var = format!("{prefix}_HTTP_REAL").to_uppercase();
    unsafe { std::env::set_var(&env_var, "not a url") };

    let result = BinanceUrls.env_var_or_default(prefix, Protocol::Http, TradingMode::Real);

    assert!(result.is_err());
}

#[test]
fn override_with_wrong_scheme_is_rejected() {
    let prefix = "BP_URL_TEST_SCHEME";
    let env_var = format!("{prefix}_WEBSOCKET_REAL").to_uppercase();
    unsafe { std::env::set_var(&env_var, "https://example.test/ws-api/v3") };

    let result = BinanceUrls.env_var_or_default(prefix, Protocol::Websocket, TradingMode::Real);

    assert!(result.is_err());
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
    let env_var = format!("{prefix}_HTTP_REAL").to_uppercase();
    unsafe { std::env::remove_var(&env_var) };

    let result = InvalidDefaultUrls.env_var_or_default(prefix, Protocol::Http, TradingMode::Real);

    assert!(result.is_err());
}
