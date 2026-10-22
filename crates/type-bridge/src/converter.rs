use crate::{
    TBExchangeInfo, asset_ids_to_tickers::AssetIdsToTickers, asset_limits::TBAssetLimits,
    floats_to_decimals::FloatsToDecimals, semantic_checker::SemanticChecker,
    ticker_mapping::TickerMapping, trading_precision::TradingPrecision,
};
use buildplane_exchange_types::{
    exchange::ETExchange,
    request::{ETHttpRequest, ETWebsocketRequest},
    response::{ETHttpResponse, ETWebsocketResponse},
    ticker::Ticker,
};
use buildplane_shared::errors::BpResult;
use rust_decimal::Decimal;
use stock_trek_types::cex::{
    asset_id::AssetId, capability::CexCapability, order_request::OrderRequest,
    order_response::OrderResponse, preferences::CexPreferences, trading_pair::TradingPair,
};

pub trait Converter {
    type Exchange: ETExchange;
    type AssetLimitsRequest: Sized
        + ETHttpRequest<Exchange = Self::Exchange, Response = Self::AssetLimitsResponse>
        + ETWebsocketRequest<Exchange = Self::Exchange, Response = Self::AssetLimitsResponse>;
    type AssetLimitsResponse: Sized + ETHttpResponse + ETWebsocketResponse;
    type ExchangeInfoRequest: Sized
        + ETHttpRequest<Exchange = Self::Exchange, Response = Self::ExchangeInfoResponse>
        + ETWebsocketRequest<Exchange = Self::Exchange, Response = Self::ExchangeInfoResponse>;
    type ExchangeInfoResponse: Sized + ETHttpResponse + ETWebsocketResponse;
    type OrderRequest: Sized
        + ETHttpRequest<Exchange = Self::Exchange, Response = Self::OrderResponse>
        + ETWebsocketRequest<Exchange = Self::Exchange, Response = Self::OrderResponse>;
    type OrderResponse: Sized + ETHttpResponse + ETWebsocketResponse;

    fn capabilities(&self) -> &Vec<CexCapability>;
    fn ticker_mapping(&self) -> &TickerMapping;
    fn asset_limits_request(
        &self,
        trading_pair: &TradingPair,
        cex_preferences: &CexPreferences,
    ) -> BpResult<Self::AssetLimitsRequest>;
    fn exchange_info_request(&self) -> BpResult<Self::ExchangeInfoRequest>;
    fn order_request(
        &self,
        order_request: OrderRequest<AssetId, f64>,
        trading_precision: TradingPrecision,
        cex_preferences: &CexPreferences,
    ) -> BpResult<Self::OrderRequest>;
    fn asset_limits_response(&self, response: Self::AssetLimitsResponse)
    -> BpResult<TBAssetLimits>;
    fn exchange_info_response(
        &self,
        response: Self::ExchangeInfoResponse,
    ) -> BpResult<TBExchangeInfo>;
    fn order_response(&self, response: Self::OrderResponse) -> BpResult<OrderResponse>;
    fn try_convert_precise(
        &self,
        order_request: OrderRequest<AssetId, f64>,
        cex_preferences: &CexPreferences,
        trading_precision: TradingPrecision,
    ) -> BpResult<OrderRequest<Ticker, Decimal>> {
        let order_request_with_precision =
            FloatsToDecimals::new(trading_precision, cex_preferences.rounding)
                .floats_to_decimals(order_request)?;
        SemanticChecker.consistent_or_err(&order_request_with_precision, self.capabilities())?;
        AssetIdsToTickers::asset_ids_to_tickers(order_request_with_precision, self.ticker_mapping())
    }
}
