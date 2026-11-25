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
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub public_message: String,
    pub internal_detail: Option<String>,
    pub clear_refresh_cookie: Option<String>,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            public_message: message.into(),
            internal_detail: None,
            clear_refresh_cookie: None,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            public_message: message.into(),
            internal_detail: None,
            clear_refresh_cookie: None,
        }
    }

    pub fn unauthorized_and_clear_cookie(
        message: impl Into<String>,
        set_cookie_header: String,
    ) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            public_message: message.into(),
            internal_detail: None,
            clear_refresh_cookie: Some(set_cookie_header),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "FORBIDDEN",
            public_message: message.into(),
            internal_detail: None,
            clear_refresh_cookie: None,
        }
    }

    pub fn payload_too_large() -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            code: "PAYLOAD_TOO_LARGE",
            public_message: "Request body exceeds maximum allowed size.".to_string(),
            internal_detail: None,
            clear_refresh_cookie: None,
        }
    }

    pub fn rate_limited() -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            code: "RATE_LIMITED",
            public_message: "Too many requests. Try again later.".to_string(),
            internal_detail: None,
            clear_refresh_cookie: None,
        }
    }

    pub fn service_unavailable(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "SERVICE_UNAVAILABLE",
            public_message: "Required service dependency is unavailable.".to_string(),
            internal_detail: Some(detail.into()),
            clear_refresh_cookie: None,
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL_ERROR",
            public_message: "An unexpected internal error occurred.".to_string(),
            internal_detail: Some(detail.into()),
            clear_refresh_cookie: None,
        }
    }

    pub fn into_response_with_request_id(self, request_id: &str) -> Response {
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
        if let Some(cookie) = self.clear_refresh_cookie {
            if let Ok(val) = HeaderValue::from_str(&cookie) {
                headers.append(header::SET_COOKIE, val);
            }
        }
        response
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        self.into_response_with_request_id("unassigned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_error_never_exposes_internal_detail() {
        let err = AppError::internal("SQLITE_CONSTRAINT: users.email leaked detail");
        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!err.public_message.contains("SQLITE"));
    }
}
