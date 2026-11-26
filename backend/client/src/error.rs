use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub request_id: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ClientError {
    pub status: StatusCode,
    pub code: &'static str,
    pub public_message: String,
    pub internal_detail: Option<String>,
}

impl ClientError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            public_message: message.into(),
            internal_detail: None,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            public_message: message.into(),
            internal_detail: None,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "FORBIDDEN",
            public_message: message.into(),
            internal_detail: None,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            public_message: "Resource not found.".to_string(),
            internal_detail: None,
        }
    }

    pub fn method_not_allowed() -> Self {
        Self {
            status: StatusCode::METHOD_NOT_ALLOWED,
            code: "METHOD_NOT_ALLOWED",
            public_message: "Method not allowed on this path.".to_string(),
            internal_detail: None,
        }
    }

    pub fn payload_too_large() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "PAYLOAD_TOO_LARGE",
            public_message: "Request body exceeds maximum allowed size.".to_string(),
            internal_detail: None,
        }
    }

    pub fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "RATE_LIMITED",
            public_message: "Too many requests. Try again later.".to_string(),
            internal_detail: None,
        }
    }

    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "SERVICE_UNAVAILABLE",
            public_message: "Required service dependency is unavailable.".to_string(),
            internal_detail: Some(detail.into()),
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            public_message: "An unexpected internal error occurred.".to_string(),
            internal_detail: Some(detail.into()),
        }
    }

    pub fn into_response_with_request_id(self, request_id: &str) -> Response {
        if self.status.is_server_error() {
            if let Some(detail) = &self.internal_detail {
                log_server_error(request_id, self.code, detail);
            }
        }
        let mut response = (
            self.status,
            Json(ErrorEnvelope {
                request_id: request_id.to_string(),
                code: self.code.to_string(),
                message: self.public_message,
            }),
        )
            .into_response();

        let headers = response.headers_mut();
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        if let Ok(val) = HeaderValue::from_str(request_id) {
            headers.insert("x-request-id", val);
        }
        response
    }
}

impl IntoResponse for ClientError {
    fn into_response(self) -> Response {
        self.into_response_with_request_id("unassigned")
    }
}

/// Server-side log for 5xx errors. The detail stays out of the response body;
/// it only goes to the Worker log (`wrangler tail` / dashboard) with the request id.
#[cfg(target_arch = "wasm32")]
fn log_server_error(request_id: &str, code: &str, detail: &str) {
    worker::console_error!("request_id={request_id} code={code} detail={detail}");
}

#[cfg(not(target_arch = "wasm32"))]
fn log_server_error(_request_id: &str, _code: &str, _detail: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_never_leaks_internal_sql_or_stack() {
        let err = ClientError::service_unavailable("D1_ERROR: no such table cards");
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(!err.public_message.contains("D1_ERROR"));
        assert!(!err.public_message.contains("cards"));
    }
}
