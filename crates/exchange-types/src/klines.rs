use crate::{
    error::ETResult,
    new_types::Milliseconds,
    request::{ETHttpRequest, ETWebsocketRequest},
    response::{ETHttpResponse, ETWebsocketResponse},
};
use rust_decimal::Decimal;

pub trait KlinesHttpRequest<Response>: ETHttpRequest<Response = Response>
where
    Response: ETHttpResponse + KlinesResponse,
{
}

pub trait KlinesWebsocketRequest<Response>: ETWebsocketRequest<Response = Response>
where
    Response: ETWebsocketResponse + KlinesResponse,
{
}

pub trait KlinesResponse {
    fn klines(&self) -> ETResult<Vec<Kline>>;
}

pub struct Kline {
    pub open_time: Milliseconds,
    pub close_time: Milliseconds,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub trades: u32,
}
