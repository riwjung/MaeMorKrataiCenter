pub mod adapters;
pub mod config;
pub mod error;
pub mod master_data;
pub mod middleware;
pub mod ports;
pub mod routes;
pub mod security;

use crate::adapters::worker_env::WorkerClientAdapter;
use crate::error::ClientError;
use crate::middleware::security::security_pipeline_middleware;
use crate::ports::ClientPorts;
use crate::routes::health::{fallback_not_found, get_health};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use axum::Router;
use tower_service::Service;
use worker::{event, Context, Env, HttpRequest};

pub fn build_router(ports: ClientPorts) -> Router {
    Router::new()
        .route("/health", get(get_health))
        .fallback(fallback_not_found)
        .layer(from_fn_with_state(
            ports.clone(),
            security_pipeline_middleware,
        ))
        .with_state(ports)
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<axum::body::Body>> {
    let adapter = WorkerClientAdapter::new(env);
    let ports = match adapter.into_ports() {
        Ok(p) => p,
        Err(err) => return Ok(err.into_response_with_request_id("startup")),
    };

    let mut router = build_router(ports);
    let axum_req = req.map(axum::body::Body::new);
    match router.call(axum_req).await {
        Ok(resp) => Ok(resp),
        Err(_) => Ok(ClientError::internal("router dispatch failure")
            .into_response_with_request_id("dispatch")),
    }
}
