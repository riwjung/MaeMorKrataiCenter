use crate::error::ClientError;
use crate::ports::ClientPorts;
use axum::body::Body;
use axum::extract::{FromRequest, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;
use serde::de::DeserializeOwned;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub fn extract_request_id(req: &Request) -> String {
    req.extensions()
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "unassigned".to_string())
}

pub fn client_ip_from_headers(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .unwrap_or("unknown")
        .to_string()
}

pub async fn security_pipeline_middleware(
    State(ports): State<ClientPorts>,
    mut req: Request,
    next: Next,
) -> Response {
    let req_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 64 && s.bytes().all(|b| b.is_ascii_graphic()))
        .map(ToString::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(RequestId(req_id.clone()));

    let origin = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .map(ToString::to_string);
    let method = req.method().clone();
    let origin_allowed = origin
        .as_deref()
        .map(|o| ports.config.is_origin_allowed(o))
        .unwrap_or(false);

    if method == Method::OPTIONS {
        if let Some(ref orig) = origin {
            if origin_allowed {
                let mut res = Response::new(Body::empty());
                *res.status_mut() = StatusCode::NO_CONTENT;
                apply_cors_headers(&mut res, orig);
                apply_security_headers(&mut res, &req_id, false);
                return res;
            }
        }
        let mut res =
            ClientError::forbidden("Origin not allowed.").into_response_with_request_id(&req_id);
        apply_security_headers(&mut res, &req_id, true);
        return res;
    }

    if origin.is_some() && !origin_allowed && is_state_changing(&method) {
        let mut res = ClientError::forbidden("Cross-origin request rejected.")
            .into_response_with_request_id(&req_id);
        apply_security_headers(&mut res, &req_id, true);
        return res;
    }

    let rate_key = format!(
        "global:{}:{}",
        req.uri().path(),
        client_ip_from_headers(req.headers())
    );
    match ports.rate_limiter.check_limit(&rate_key).await {
        Ok(true) => {}
        Ok(false) => {
            let mut res = ClientError::rate_limited().into_response_with_request_id(&req_id);
            if origin_allowed {
                if let Some(ref orig) = origin {
                    apply_cors_headers(&mut res, orig);
                }
            }
            apply_security_headers(&mut res, &req_id, true);
            return res;
        }
        Err(err) => {
            let mut res = err.into_response_with_request_id(&req_id);
            apply_security_headers(&mut res, &req_id, true);
            return res;
        }
    }

    let max_bytes = ports.config.max_body_bytes;
    if let Some(cl_val) = req.headers().get(header::CONTENT_LENGTH) {
        if let Ok(cl_str) = cl_val.to_str() {
            let Ok(len) = cl_str.trim().parse::<usize>() else {
                let mut res = ClientError::bad_request("Invalid Content-Length header.")
                    .into_response_with_request_id(&req_id);
                apply_security_headers(&mut res, &req_id, true);
                return res;
            };
            if len > max_bytes {
                let mut res =
                    ClientError::payload_too_large().into_response_with_request_id(&req_id);
                if origin_allowed {
                    if let Some(ref orig) = origin {
                        apply_cors_headers(&mut res, orig);
                    }
                }
                apply_security_headers(&mut res, &req_id, true);
                return res;
            }
        }
    }

    let (parts, body) = req.into_parts();
    let buffered = match axum::body::to_bytes(body, max_bytes).await {
        Ok(b) => b,
        Err(_) => {
            let mut res = ClientError::payload_too_large().into_response_with_request_id(&req_id);
            if origin_allowed {
                if let Some(ref orig) = origin {
                    apply_cors_headers(&mut res, orig);
                }
            }
            apply_security_headers(&mut res, &req_id, true);
            return res;
        }
    };
    let rebuilt = Request::from_parts(parts, Body::from(buffered));

    let mut res = next.run(rebuilt).await;
    if origin_allowed {
        if let Some(ref orig) = origin {
            apply_cors_headers(&mut res, orig);
        }
    }
    let preserve_cache = res.headers().contains_key(header::CACHE_CONTROL);
    apply_security_headers(&mut res, &req_id, !preserve_cache);
    res
}

fn is_state_changing(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn apply_cors_headers(res: &mut Response, allowed_origin: &str) {
    let headers = res.headers_mut();
    if let Ok(val) = HeaderValue::from_str(allowed_origin) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, val);
    }
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Authorization, Content-Type, CF-Turnstile-Response"),
    );
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
}

fn apply_security_headers(res: &mut Response, request_id: &str, force_no_store: bool) {
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    if force_no_store {
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    if let Ok(val) = HeaderValue::from_str(request_id) {
        headers.insert("x-request-id", val);
    }
}

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let req_id = extract_request_id(&req);
        match Json::<T>::from_request(req, state).await {
            Ok(Json(val)) => Ok(Self(val)),
            Err(_) => Err(ClientError::bad_request("Malformed JSON payload.")
                .into_response_with_request_id(&req_id)),
        }
    }
}
