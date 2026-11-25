pub mod adapters;
pub mod auth;
pub mod config;
pub mod error;
pub mod middleware;
pub mod ports;
pub mod routes;

use crate::adapters::worker_env::WorkerEnvAdapter;
use crate::error::AppError;
use crate::middleware::security::request_id_and_headers_middleware;
use crate::ports::AppPorts;
use crate::routes::auth::{get_me, post_login, post_logout, post_refresh, post_rotate_keys};
use crate::routes::health::{fallback_not_found, get_health};
use crate::routes::jwks::get_jwks;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use tower_service::Service;
use worker::{event, Context, Env, HttpRequest};

pub fn build_router(ports: AppPorts) -> Router {
    Router::new()
        .route("/health", get(get_health))
        .route("/.well-known/jwks.json", get(get_jwks))
        .route("/v1/auth/login", post(post_login))
        .route("/v1/auth/refresh", post(post_refresh))
        .route("/v1/auth/logout", post(post_logout))
        .route("/v1/me", get(get_me))
        .route("/v1/admin/auth/keys/rotate", post(post_rotate_keys))
        .fallback(fallback_not_found)
        .layer(from_fn_with_state(
            ports.clone(),
            request_id_and_headers_middleware,
        ))
        .with_state(ports)
}

#[event(fetch)]
async fn fetch(
    req: HttpRequest,
    env: Env,
    _ctx: Context,
) -> worker::Result<http::Response<axum::body::Body>> {
    let adapter = WorkerEnvAdapter::new(env);
    let ports = match adapter.into_ports() {
        Ok(p) => p,
        Err(err) => return Ok(err.into_response_with_request_id("startup")),
    };

    let mut router = build_router(ports);
    let axum_req = req.map(axum::body::Body::new);
    match router.call(axum_req).await {
        Ok(resp) => Ok(resp),
        Err(_) => {
            Ok(AppError::internal("router dispatch failure")
                .into_response_with_request_id("dispatch"))
        }
    }
}
