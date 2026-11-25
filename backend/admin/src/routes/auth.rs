use crate::auth::envelope::{decrypt_private_key, encrypt_private_key};
use crate::auth::jwt::{generate_es256_keypair, sign_es256_jwt, AccessClaims, Role};
use crate::auth::password::verify_password;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::middleware::auth::{RequireAdmin, RequireOwner};
use crate::middleware::security::{RequestId, ValidatedJson};
use crate::ports::{AppPorts, AuditEventInput, JwksKeyRecord, RefreshOutcome, SessionRecord};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub role: &'static str,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub id: String,
    pub email: String,
    pub role: &'static str,
    pub is_active: bool,
}

#[derive(Serialize)]
pub struct RotateKeyResponse {
    pub kid: String,
    pub alg: String,
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct LogoutResponse {
    pub status: &'static str,
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn format_refresh_cookie(config: &AppConfig, raw_token: &str) -> String {
    if config.environment.is_secure_cookie() {
        format!(
            "{}={}; HttpOnly; Path=/v1/auth; SameSite=None; Secure; Max-Age={}",
            config.cookie_name, raw_token, config.refresh_token_ttl_secs
        )
    } else {
        format!(
            "{}={}; HttpOnly; Path=/v1/auth; SameSite=Lax; Max-Age={}",
            config.cookie_name, raw_token, config.refresh_token_ttl_secs
        )
    }
}

fn format_clear_refresh_cookie(config: &AppConfig) -> String {
    if config.environment.is_secure_cookie() {
        format!(
            "{}=; HttpOnly; Path=/v1/auth; SameSite=None; Secure; Max-Age=0",
            config.cookie_name
        )
    } else {
        format!(
            "{}=; HttpOnly; Path=/v1/auth; SameSite=Lax; Max-Age=0",
            config.cookie_name
        )
    }
}

fn extract_cookie_value(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in raw.split(';') {
        let trimmed = pair.trim();
        if let Some((name, value)) = trimmed.split_once('=') {
            if name.trim() == cookie_name && !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

fn verify_csrf_and_origin(headers: &HeaderMap, config: &AppConfig) -> Result<(), AppError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .ok_or_else(|| AppError::forbidden("Missing Origin header."))?;

    if !config.is_origin_allowed(origin) {
        return Err(AppError::forbidden("Origin not allowed."));
    }

    let csrf = headers
        .get("x-csrf-protection")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .unwrap_or("");
    if csrf != "1" {
        return Err(AppError::forbidden("Missing or invalid CSRF header."));
    }

    Ok(())
}

fn rate_limit_key(headers: &HeaderMap, scope: &str) -> String {
    let ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .unwrap_or("unknown");
    format!("{scope}:{ip}")
}

async fn ensure_active_signing_key(
    ports: &AppPorts,
    actor_user_id: Option<&str>,
    now_secs: u64,
) -> Result<(String, zeroize::Zeroizing<Vec<u8>>), AppError> {
    let kek = ports.secrets.key_encryption_key()?;
    if let Some(record) = ports.repo.get_active_signing_key().await? {
        let pkcs8 = decrypt_private_key(
            &kek,
            &record.kid,
            &record.alg,
            &record.encrypted_private_key,
        )?;
        return Ok((record.kid, pkcs8));
    }

    let generated = generate_es256_keypair()?;
    let encrypted = encrypt_private_key(
        &kek,
        &generated.kid,
        &generated.alg,
        &generated.pkcs8_private_der,
    )?;
    let public_jwk_json = serde_json::to_string(&generated.public_jwk)
        .map_err(|_| AppError::internal("failed to serialize public JWK"))?;

    let key_record = JwksKeyRecord {
        kid: generated.kid.clone(),
        alg: generated.alg.clone(),
        public_jwk_json,
        encrypted_private_key: encrypted,
        status: "active".to_string(),
        created_at: now_secs,
    };
    let audit = AuditEventInput {
        id: Uuid::new_v4().to_string(),
        actor_user_id: actor_user_id.map(ToString::to_string),
        event_type: "jwks_initial_key_created".to_string(),
        target_id: Some(generated.kid.clone()),
        metadata_json: serde_json::json!({ "alg": generated.alg }).to_string(),
        created_at: now_secs,
    };
    ports.repo.rotate_signing_key(key_record, audit).await?;
    Ok((generated.kid, generated.pkcs8_private_der))
}

#[axum::debug_handler]
pub async fn post_login(
    State(ports): State<AppPorts>,
    Extension(req_id): Extension<RequestId>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<LoginRequest>,
) -> Response {
    match handle_login(&ports, &headers, payload).await {
        Ok(res) => res,
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

async fn handle_login(
    ports: &AppPorts,
    headers: &HeaderMap,
    payload: LoginRequest,
) -> Result<Response, AppError> {
    if !ports
        .rate_limiter
        .check_limit(&rate_limit_key(headers, "login"))
        .await?
    {
        return Err(AppError::rate_limited());
    }

    let email = payload.email.trim().to_ascii_lowercase();
    if email.is_empty() || payload.password.is_empty() {
        return Err(AppError::unauthorized("Invalid credentials."));
    }

    let Some(user) = ports.repo.find_user_by_email(&email).await? else {
        return Err(AppError::unauthorized("Invalid credentials."));
    };

    let pepper = ports.secrets.password_pepper()?;
    let valid = verify_password(&payload.password, &user.password_hash, &pepper).await?;
    if !valid {
        return Err(AppError::unauthorized("Invalid credentials."));
    }

    let now = ports.clock.now_secs();
    if !user.is_active {
        let _ = ports.repo.revoke_user_sessions(&user.id, now).await;
        return Err(AppError::forbidden("Account is inactive."));
    }
    if user.role < Role::Admin {
        return Err(AppError::forbidden("Insufficient role privileges."));
    }

    let (kid, pkcs8_der) = ensure_active_signing_key(ports, Some(&user.id), now).await?;
    let claims = AccessClaims {
        iss: ports.config.jwt_issuer.clone(),
        aud: ports.config.jwt_audience.clone(),
        sub: user.id.clone(),
        role: user.role,
        jti: Uuid::new_v4().to_string(),
        iat: now,
        exp: now + ports.config.access_token_ttl_secs,
        nbf: None,
    };
    let access_token = sign_es256_jwt(&kid, &pkcs8_der, &claims)?;

    let raw_refresh = generate_refresh_token();
    let session = SessionRecord {
        id: Uuid::new_v4().to_string(),
        user_id: user.id.clone(),
        token_hash: sha256_hex(&raw_refresh),
        family_id: Uuid::new_v4().to_string(),
        expires_at: now + ports.config.refresh_token_ttl_secs,
        revoked_at: None,
        replaced_by_session_id: None,
        created_at: now,
    };
    ports.repo.insert_session(session).await?;

    let _ = ports
        .repo
        .insert_audit_event(AuditEventInput {
            id: Uuid::new_v4().to_string(),
            actor_user_id: Some(user.id.clone()),
            event_type: "auth_login_succeeded".to_string(),
            target_id: Some(user.id.clone()),
            metadata_json: serde_json::json!({ "role": user.role.as_str() }).to_string(),
            created_at: now,
        })
        .await;

    let mut res = (
        StatusCode::OK,
        Json(TokenResponse {
            access_token,
            token_type: "Bearer",
            expires_in: ports.config.access_token_ttl_secs,
            role: user.role.as_str(),
        }),
    )
        .into_response();

    let cookie = format_refresh_cookie(&ports.config, &raw_refresh);
    if let Ok(val) = HeaderValue::from_str(&cookie) {
        res.headers_mut().append(header::SET_COOKIE, val);
    }
    Ok(res)
}

#[axum::debug_handler]
pub async fn post_refresh(
    State(ports): State<AppPorts>,
    Extension(req_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    match handle_refresh(&ports, &headers).await {
        Ok(res) => res,
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

async fn handle_refresh(ports: &AppPorts, headers: &HeaderMap) -> Result<Response, AppError> {
    verify_csrf_and_origin(headers, &ports.config)?;

    if !ports
        .rate_limiter
        .check_limit(&rate_limit_key(headers, "refresh"))
        .await?
    {
        return Err(AppError::rate_limited());
    }

    let Some(raw_token) = extract_cookie_value(headers, &ports.config.cookie_name) else {
        return Err(AppError::unauthorized("Missing refresh session."));
    };

    let now = ports.clock.now_secs();
    let next_raw_refresh = generate_refresh_token();
    let new_session = SessionRecord {
        id: Uuid::new_v4().to_string(),
        user_id: String::new(),
        token_hash: sha256_hex(&next_raw_refresh),
        family_id: String::new(),
        expires_at: now + ports.config.refresh_token_ttl_secs,
        revoked_at: None,
        replaced_by_session_id: None,
        created_at: now,
    };

    let token_hash = sha256_hex(&raw_token);
    let outcome = ports
        .repo
        .rotate_refresh_session(&token_hash, new_session, now)
        .await?;

    match outcome {
        RefreshOutcome::Rotated { user, .. } => {
            if !user.is_active {
                let _ = ports.repo.revoke_user_sessions(&user.id, now).await;
                return Err(AppError::forbidden("Account is inactive."));
            }
            if user.role < Role::Admin {
                return Err(AppError::forbidden("Insufficient role privileges."));
            }

            let (kid, pkcs8_der) = ensure_active_signing_key(ports, Some(&user.id), now).await?;
            let claims = AccessClaims {
                iss: ports.config.jwt_issuer.clone(),
                aud: ports.config.jwt_audience.clone(),
                sub: user.id.clone(),
                role: user.role,
                jti: Uuid::new_v4().to_string(),
                iat: now,
                exp: now + ports.config.access_token_ttl_secs,
                nbf: None,
            };
            let access_token = sign_es256_jwt(&kid, &pkcs8_der, &claims)?;

            let mut res = (
                StatusCode::OK,
                Json(TokenResponse {
                    access_token,
                    token_type: "Bearer",
                    expires_in: ports.config.access_token_ttl_secs,
                    role: user.role.as_str(),
                }),
            )
                .into_response();

            let cookie = format_refresh_cookie(&ports.config, &next_raw_refresh);
            if let Ok(val) = HeaderValue::from_str(&cookie) {
                res.headers_mut().append(header::SET_COOKIE, val);
            }
            Ok(res)
        }
        RefreshOutcome::Replayed { user_id, family_id } => {
            let _ = ports
                .repo
                .insert_audit_event(AuditEventInput {
                    id: Uuid::new_v4().to_string(),
                    actor_user_id: Some(user_id),
                    event_type: "refresh_token_replay_detected".to_string(),
                    target_id: Some(family_id),
                    metadata_json: serde_json::json!({ "action": "family_revoked" }).to_string(),
                    created_at: now,
                })
                .await;
            Err(AppError::unauthorized_and_clear_cookie(
                "Refresh session invalidated.",
                format_clear_refresh_cookie(&ports.config),
            ))
        }
        RefreshOutcome::NotFoundOrExpired => Err(AppError::unauthorized_and_clear_cookie(
            "Invalid or expired refresh session.",
            format_clear_refresh_cookie(&ports.config),
        )),
    }
}

#[axum::debug_handler]
pub async fn post_logout(
    State(ports): State<AppPorts>,
    Extension(req_id): Extension<RequestId>,
    headers: HeaderMap,
) -> Response {
    match handle_logout(&ports, &headers).await {
        Ok(res) => res,
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

async fn handle_logout(ports: &AppPorts, headers: &HeaderMap) -> Result<Response, AppError> {
    verify_csrf_and_origin(headers, &ports.config)?;

    let now = ports.clock.now_secs();
    if let Some(raw_token) = extract_cookie_value(headers, &ports.config.cookie_name) {
        let token_hash = sha256_hex(&raw_token);
        if let Some(user_id) = ports
            .repo
            .revoke_session_by_token_hash(&token_hash, now)
            .await?
        {
            let _ = ports
                .repo
                .insert_audit_event(AuditEventInput {
                    id: Uuid::new_v4().to_string(),
                    actor_user_id: Some(user_id),
                    event_type: "auth_logout".to_string(),
                    target_id: None,
                    metadata_json: "{}".to_string(),
                    created_at: now,
                })
                .await;
        }
    }

    let mut res = (
        StatusCode::OK,
        Json(LogoutResponse {
            status: "logged_out",
        }),
    )
        .into_response();
    let clear_cookie = format_clear_refresh_cookie(&ports.config);
    if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
        res.headers_mut().append(header::SET_COOKIE, val);
    }
    Ok(res)
}

#[axum::debug_handler(state = AppPorts)]
pub async fn get_me(RequireAdmin(caller): RequireAdmin) -> Response {
    (
        StatusCode::OK,
        Json(MeResponse {
            id: caller.user.id,
            email: caller.user.email,
            role: caller.user.role.as_str(),
            is_active: caller.user.is_active,
        }),
    )
        .into_response()
}

#[axum::debug_handler]
pub async fn post_rotate_keys(
    State(ports): State<AppPorts>,
    Extension(req_id): Extension<RequestId>,
    RequireOwner(caller): RequireOwner,
) -> Response {
    match handle_rotate_keys(&ports, &caller.user.id).await {
        Ok(res) => res,
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

async fn handle_rotate_keys(ports: &AppPorts, actor_user_id: &str) -> Result<Response, AppError> {
    let now = ports.clock.now_secs();
    let kek = ports.secrets.key_encryption_key()?;
    let generated = generate_es256_keypair()?;
    let encrypted = encrypt_private_key(
        &kek,
        &generated.kid,
        &generated.alg,
        &generated.pkcs8_private_der,
    )?;
    let public_jwk_json = serde_json::to_string(&generated.public_jwk)
        .map_err(|_| AppError::internal("failed to serialize public JWK"))?;

    let key_record = JwksKeyRecord {
        kid: generated.kid.clone(),
        alg: generated.alg.clone(),
        public_jwk_json,
        encrypted_private_key: encrypted,
        status: "active".to_string(),
        created_at: now,
    };
    let audit = AuditEventInput {
        id: Uuid::new_v4().to_string(),
        actor_user_id: Some(actor_user_id.to_string()),
        event_type: "jwks_key_rotated".to_string(),
        target_id: Some(generated.kid.clone()),
        metadata_json: serde_json::json!({ "alg": generated.alg }).to_string(),
        created_at: now,
    };

    ports.repo.rotate_signing_key(key_record, audit).await?;

    Ok((
        StatusCode::OK,
        Json(RotateKeyResponse {
            kid: generated.kid,
            alg: generated.alg,
            status: "active",
        }),
    )
        .into_response())
}
