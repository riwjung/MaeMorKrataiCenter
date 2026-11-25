use crate::error::ClientError;
use crate::middleware::security::{client_ip_from_headers, RequestId};
use crate::ports::ClientPorts;
use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct TurnstileSiteVerifyRequest {
    pub secret: String,
    pub response: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remoteip: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnstileSiteVerifyResponse {
    pub success: bool,
    #[serde(default, rename = "error-codes")]
    pub error_codes: Vec<String>,
}

#[async_trait]
pub trait TurnstileTransport: Send + Sync {
    async fn siteverify(
        &self,
        verify_url: &str,
        payload: TurnstileSiteVerifyRequest,
    ) -> Result<TurnstileSiteVerifyResponse, ClientError>;
}

pub struct WorkerTurnstileTransport;

#[cfg(target_arch = "wasm32")]
#[async_trait]
impl TurnstileTransport for WorkerTurnstileTransport {
    async fn siteverify(
        &self,
        verify_url: &str,
        payload: TurnstileSiteVerifyRequest,
    ) -> Result<TurnstileSiteVerifyResponse, ClientError> {
        use wasm_bindgen::JsValue;
        use worker::send::SendFuture;

        let body_json = serde_json::to_string(&payload)
            .map_err(|_| ClientError::internal("failed to serialize Turnstile payload"))?;
        let headers = worker::Headers::new();
        headers
            .set("content-type", "application/json")
            .map_err(|_| ClientError::internal("failed to set Turnstile header"))?;

        let mut init = worker::RequestInit::new();
        init.with_method(worker::Method::Post)
            .with_headers(headers)
            .with_body(Some(JsValue::from_str(&body_json)));

        let req = worker::Request::new_with_init(verify_url, &init)
            .map_err(|_| ClientError::service_unavailable("Turnstile request build failed"))?;
        let mut resp = SendFuture::new(worker::Fetch::Request(req).send())
            .await
            .map_err(|_| ClientError::service_unavailable("Turnstile verification unreachable"))?;

        if resp.status_code() != 200 {
            return Err(ClientError::service_unavailable(
                "Turnstile verification returned non-200 status",
            ));
        }

        let text = SendFuture::new(resp.text())
            .await
            .map_err(|_| ClientError::service_unavailable("Turnstile response read failed"))?;
        serde_json::from_str::<TurnstileSiteVerifyResponse>(&text)
            .map_err(|_| ClientError::service_unavailable("Turnstile response decode failed"))
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl TurnstileTransport for WorkerTurnstileTransport {
    async fn siteverify(
        &self,
        _verify_url: &str,
        _payload: TurnstileSiteVerifyRequest,
    ) -> Result<TurnstileSiteVerifyResponse, ClientError> {
        Err(ClientError::service_unavailable(
            "WorkerTurnstileTransport requires wasm32 runtime",
        ))
    }
}

#[derive(Clone)]
pub struct TurnstileGuard {
    transport: Arc<dyn TurnstileTransport>,
}

impl TurnstileGuard {
    pub fn new(transport: Arc<dyn TurnstileTransport>) -> Self {
        Self { transport }
    }

    pub async fn verify_token(
        &self,
        ports: &ClientPorts,
        token: Option<&str>,
        remoteip: Option<String>,
    ) -> Result<(), ClientError> {
        let Some(clean_token) = token.map(str::trim).filter(|t| !t.is_empty()) else {
            return Err(ClientError::forbidden(
                "Turnstile verification token required.",
            ));
        };

        let secret = ports.secrets.turnstile_secret()?;
        let response = self
            .transport
            .siteverify(
                &ports.config.turnstile_verify_url,
                TurnstileSiteVerifyRequest {
                    secret,
                    response: clean_token.to_string(),
                    remoteip,
                },
            )
            .await?;

        if !response.success {
            return Err(ClientError::forbidden("Turnstile verification failed."));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedTurnstile;

#[derive(Clone)]
pub struct TurnstileRouterState {
    pub ports: ClientPorts,
    pub guard: TurnstileGuard,
}

impl FromRequestParts<TurnstileRouterState> for VerifiedTurnstile {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &TurnstileRouterState,
    ) -> Result<Self, Self::Rejection> {
        let req_id = parts
            .extensions
            .get::<RequestId>()
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "unassigned".to_string());

        let token = parts
            .headers
            .get("cf-turnstile-response")
            .and_then(|v| v.to_str().ok());
        let ip = client_ip_from_headers(&parts.headers);
        let remoteip = (ip != "unknown").then_some(ip);

        state
            .guard
            .verify_token(&state.ports, token, remoteip)
            .await
            .map(|()| Self)
            .map_err(|err| err.into_response_with_request_id(&req_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::memory::MemoryClientStore;
    use crate::config::Environment;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn block_on<F: Future>(fut: F) -> F::Output {
        fn noop(_: *const ()) {}
        fn clone(ptr: *const ()) -> RawWaker {
            RawWaker::new(ptr, &VTABLE)
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
        let mut cx = Context::from_waker(&waker);
        let mut pinned = pin!(fut);
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => val,
            Poll::Pending => panic!("fixture transport must resolve synchronously"),
        }
    }

    enum FixtureOutcome {
        Pass,
        Reject,
        Outage,
    }

    struct FixtureTransport(FixtureOutcome);

    #[async_trait]
    impl TurnstileTransport for FixtureTransport {
        async fn siteverify(
            &self,
            _verify_url: &str,
            _payload: TurnstileSiteVerifyRequest,
        ) -> Result<TurnstileSiteVerifyResponse, ClientError> {
            match self.0 {
                FixtureOutcome::Pass => Ok(TurnstileSiteVerifyResponse {
                    success: true,
                    error_codes: vec![],
                }),
                FixtureOutcome::Reject => Ok(TurnstileSiteVerifyResponse {
                    success: false,
                    error_codes: vec!["invalid-input-response".to_string()],
                }),
                FixtureOutcome::Outage => Err(ClientError::service_unavailable("upstream timeout")),
            }
        }
    }

    #[test]
    fn turnstile_guard_fails_closed_on_missing_invalid_and_outage() {
        block_on(async {
            let ports = MemoryClientStore::new().into_ports(Environment::Staging);

            let pass_guard = TurnstileGuard::new(Arc::new(FixtureTransport(FixtureOutcome::Pass)));
            assert!(pass_guard
                .verify_token(&ports, Some("valid-tok"), None)
                .await
                .is_ok());

            // Missing token -> 403
            let missing_err = pass_guard
                .verify_token(&ports, None, None)
                .await
                .unwrap_err();
            assert_eq!(missing_err.status, axum::http::StatusCode::FORBIDDEN);

            // Invalid token -> 403
            let reject_guard =
                TurnstileGuard::new(Arc::new(FixtureTransport(FixtureOutcome::Reject)));
            let reject_err = reject_guard
                .verify_token(&ports, Some("bad-tok"), None)
                .await
                .unwrap_err();
            assert_eq!(reject_err.status, axum::http::StatusCode::FORBIDDEN);

            // Upstream outage -> 503 (never bypassed)
            let outage_guard =
                TurnstileGuard::new(Arc::new(FixtureTransport(FixtureOutcome::Outage)));
            let outage_err = outage_guard
                .verify_token(&ports, Some("valid-tok"), None)
                .await
                .unwrap_err();
            assert_eq!(
                outage_err.status,
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            );
        });
    }
}
