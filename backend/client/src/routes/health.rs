use crate::error::ClientError;
use crate::middleware::security::RequestId;
use crate::ports::ClientPorts;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    d1: &'static str,
    public_r2: &'static str,
}

#[axum::debug_handler]
pub async fn get_health(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    if let Err(err) = ports.health.check_d1().await {
        return err.into_response_with_request_id(&req_id.0);
    }
    if let Err(err) = ports.health.check_public_r2().await {
        return err.into_response_with_request_id(&req_id.0);
    }

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            d1: "ok",
            public_r2: "ok",
        }),
    )
        .into_response()
}

pub async fn fallback_not_found(Extension(req_id): Extension<RequestId>) -> Response {
    ClientError::not_found().into_response_with_request_id(&req_id.0)
}

/// D12: a known path called with a method it does not serve. axum adds the `Allow` header
/// (for example `GET,HEAD`) from the route's method router after this handler returns.
pub async fn fallback_method_not_allowed(Extension(req_id): Extension<RequestId>) -> Response {
    ClientError::method_not_allowed().into_response_with_request_id(&req_id.0)
}
