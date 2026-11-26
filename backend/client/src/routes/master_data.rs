//! The four public master-data GETs (SPEC-002a, `docs/api/client/openapi.yaml` v0.2.1).

use crate::master_data::{
    assemble_card_art, assemble_daily_cards, assemble_reading_cards, assemble_reading_config,
};
use crate::middleware::security::RequestId;
use crate::ports::ClientPorts;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

/// D05 TTL; Workers Cache (D13) and browsers both follow it.
pub const PUBLIC_CACHE_CONTROL: &str = "public, max-age=600";

/// Serializes `body`; only a `200` gets the public cache headers, so a serialization failure
/// (axum answers `500`) stays `no-store` through the security middleware.
fn public_json<T: Serialize>(body: &T) -> Response {
    let mut res = Json(body).into_response();
    if res.status() == StatusCode::OK {
        let headers = res.headers_mut();
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_CACHE_CONTROL),
        );
        // Workers Cache keys variants on `Vary`. It is set even when the request has no
        // `Origin`, so a stored no-Origin variant is never served to a browser request.
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    res
}

pub async fn get_cards(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.reading_cards().await {
        Ok(rows) => public_json(&assemble_reading_cards(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_daily_cards(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.daily_cards().await {
        Ok(rows) => public_json(&assemble_daily_cards(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_card_art(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.card_art().await {
        Ok(rows) => public_json(&assemble_card_art(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_reading_config(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.reading_config().await {
        Ok(rows) => public_json(&assemble_reading_config(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}
