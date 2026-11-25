use crate::auth::jwt::{verify_jwt_with_jwks, Role};
use crate::error::AppError;
use crate::middleware::security::RequestId;
use crate::ports::{AppPorts, UserRecord};
use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use axum::response::Response;

#[derive(Debug, Clone)]
pub struct AuthenticatedCaller {
    pub user: UserRecord,
    pub jti: String,
}

#[derive(Debug, Clone)]
pub struct RequireAdmin(pub AuthenticatedCaller);

#[derive(Debug, Clone)]
pub struct RequireOwner(pub AuthenticatedCaller);

// The error is the extractor's axum rejection (`Response`); boxing it would only be unboxed again in `from_request_parts`.
#[allow(clippy::result_large_err)]
async fn authenticate_from_parts(
    parts: &Parts,
    ports: &AppPorts,
    min_role: Role,
) -> Result<AuthenticatedCaller, Response> {
    let req_id = parts
        .extensions
        .get::<RequestId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "unassigned".to_string());

    let auth_header = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let Some(token) = auth_header.strip_prefix("Bearer ").map(str::trim) else {
        return Err(AppError::unauthorized("Authentication required.")
            .into_response_with_request_id(&req_id));
    };
    if token.is_empty() {
        return Err(AppError::unauthorized("Authentication required.")
            .into_response_with_request_id(&req_id));
    }

    let jwks = match ports.repo.list_public_jwks().await {
        Ok(k) => k,
        Err(err) => return Err(err.into_response_with_request_id(&req_id)),
    };

    let now = ports.clock.now_secs();
    let claims = match verify_jwt_with_jwks(
        token,
        &jwks,
        &ports.config.jwt_issuer,
        &ports.config.jwt_audience,
        now,
    ) {
        Ok(c) => c,
        Err(err) => return Err(err.into_response_with_request_id(&req_id)),
    };

    let user = match ports.repo.find_user_by_id(&claims.sub).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err(
                AppError::unauthorized("Invalid token.").into_response_with_request_id(&req_id)
            );
        }
        Err(err) => return Err(err.into_response_with_request_id(&req_id)),
    };

    if !user.is_active {
        let _ = ports.repo.revoke_user_sessions(&user.id, now).await;
        return Err(
            AppError::forbidden("Account is inactive.").into_response_with_request_id(&req_id)
        );
    }

    if user.role < min_role || claims.role < min_role {
        return Err(AppError::forbidden("Insufficient role privileges.")
            .into_response_with_request_id(&req_id));
    }

    Ok(AuthenticatedCaller {
        user,
        jti: claims.jti,
    })
}

impl FromRequestParts<AppPorts> for RequireAdmin {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppPorts,
    ) -> Result<Self, Self::Rejection> {
        authenticate_from_parts(parts, state, Role::Admin)
            .await
            .map(Self)
    }
}

impl FromRequestParts<AppPorts> for RequireOwner {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppPorts,
    ) -> Result<Self, Self::Rejection> {
        authenticate_from_parts(parts, state, Role::Owner)
            .await
            .map(Self)
    }
}
