use crate::{
    error::ETResult,
    new_types::Milliseconds,
    request::{ETHttpRequest, ETWebsocketRequest},
    response::{ETHttpResponse, ETWebsocketResponse},
};

pub trait ServerTimeHttpRequest<Response>: ETHttpRequest<Response = Response>
where
    Response: ETHttpResponse + ServerTimeResponse,
{
}

pub trait ServerTimeWebsocketRequest<Response>: ETWebsocketRequest<Response = Response>
where
    Response: ETWebsocketResponse + ServerTimeResponse,
{
}

pub trait ServerTimeResponse {
    fn server_time(&self) -> ETResult<Milliseconds>;
}
