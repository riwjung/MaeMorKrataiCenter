use crate::auth::jwt::{PublicJwk, Role};
use crate::config::{AppConfig, Environment};
use crate::error::AppError;
use crate::ports::{
    AppPorts, AuditEventInput, AuthRepo, Clock, JwksKeyRecord, RateLimitPort, RefreshOutcome,
    SecretStore, SessionRecord, StorageProbe, UserRecord,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use worker::d1::D1Type;
use worker::send::{SendFuture, SendWrapper};
use worker::Env;

#[derive(Clone)]
pub struct WorkerEnvAdapter {
    env: SendWrapper<Env>,
}

impl WorkerEnvAdapter {
    pub fn new(env: Env) -> Self {
        Self {
            env: SendWrapper::new(env),
        }
    }

    fn db(&self) -> Result<worker::D1Database, AppError> {
        self.env
            .d1("DB")
            .map_err(|e| AppError::service_unavailable(format!("D1 binding error: {e}")))
    }

    pub fn load_config(&self) -> Result<AppConfig, AppError> {
        let env_str = self
            .env
            .var("ENVIRONMENT")
            .map(|v| v.to_string())
            .map_err(|_| AppError::internal("missing ENVIRONMENT var"))?;
        let environment = Environment::parse(&env_str)?;

        let origins_raw = self
            .env
            .var("ALLOWED_ORIGINS")
            .map(|v| v.to_string())
            .map_err(|_| AppError::internal("missing ALLOWED_ORIGINS var"))?;
        let allowed_origins = AppConfig::parse_origins(&origins_raw)?;

        let jwt_issuer = self
            .env
            .var("JWT_ISSUER")
            .map(|v| v.to_string())
            .map_err(|_| AppError::internal("missing JWT_ISSUER var"))?;
        let jwt_audience = self
            .env
            .var("JWT_AUDIENCE")
            .map(|v| v.to_string())
            .map_err(|_| AppError::internal("missing JWT_AUDIENCE var"))?;

        let access_token_ttl_secs = self
            .env
            .var("ACCESS_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.to_string().parse::<u64>().ok())
            .unwrap_or(900);
        let refresh_token_ttl_secs = self
            .env
            .var("REFRESH_TOKEN_TTL_SECS")
            .ok()
            .and_then(|v| v.to_string().parse::<u64>().ok())
            .unwrap_or(604_800);
        let cookie_name = self
            .env
            .var("COOKIE_NAME")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "mmk_admin_rt".to_string());
        let max_body_bytes = self
            .env
            .var("MAX_BODY_BYTES")
            .ok()
            .and_then(|v| v.to_string().parse::<usize>().ok())
            .unwrap_or(65_536);

        Ok(AppConfig {
            environment,
            allowed_origins,
            jwt_issuer,
            jwt_audience,
            access_token_ttl_secs,
            refresh_token_ttl_secs,
            cookie_name,
            max_body_bytes,
        })
    }

    pub fn into_ports(self) -> Result<AppPorts, AppError> {
        let config = self.load_config()?;
        let shared = Arc::new(self);
        Ok(AppPorts {
            config,
            repo: shared.clone(),
            storage: shared.clone(),
            rate_limiter: shared.clone(),
            secrets: shared.clone(),
            clock: shared,
        })
    }
}

#[derive(Deserialize)]
struct DbUserRow {
    id: String,
    email: String,
    password_hash: String,
    role: String,
    is_active: i64,
}

#[derive(Deserialize)]
struct DbSessionRow {
    id: String,
    user_id: String,
    family_id: String,
    expires_at: i64,
    revoked_at: Option<i64>,
    replaced_by_session_id: Option<String>,
}

#[derive(Deserialize)]
struct DbKeyRow {
    kid: String,
    alg: String,
    public_jwk_json: String,
    encrypted_private_key: String,
    status: String,
    created_at: i64,
}

#[derive(Deserialize)]
struct DbPublicJwkRow {
    public_jwk_json: String,
}

fn to_user_record(row: DbUserRow) -> Result<UserRecord, AppError> {
    Ok(UserRecord {
        id: row.id,
        email: row.email,
        password_hash: row.password_hash,
        role: Role::parse(&row.role)?,
        is_active: row.is_active == 1,
    })
}

#[async_trait]
impl AuthRepo for WorkerEnvAdapter {
    async fn health_check_db(&self) -> Result<(), AppError> {
        let db = self.db()?;
        let stmt = db.prepare("SELECT 1 AS ok");
        SendFuture::new(stmt.first::<serde_json::Value>(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;
        Ok(())
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, AppError> {
        let db = self.db()?;
        let stmt = db
            .prepare("SELECT id, email, password_hash, role, is_active FROM users WHERE email = ?1 LIMIT 1")
            .bind_refs(&[D1Type::Text(email)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        let row: Option<DbUserRow> = SendFuture::new(stmt.first(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;
        row.map(to_user_record).transpose()
    }

    async fn find_user_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, AppError> {
        let db = self.db()?;
        let stmt = db
            .prepare(
                "SELECT id, email, password_hash, role, is_active FROM users WHERE id = ?1 LIMIT 1",
            )
            .bind_refs(&[D1Type::Text(user_id)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        let row: Option<DbUserRow> = SendFuture::new(stmt.first(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;
        row.map(to_user_record).transpose()
    }

    async fn insert_session(&self, session: SessionRecord) -> Result<(), AppError> {
        let db = self.db()?;
        let stmt = db
            .prepare(
                "INSERT INTO sessions (id, user_id, token_hash, family_id, expires_at, revoked_at, replaced_by_session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6)",
            )
            .bind_refs(&[
                D1Type::Text(&session.id),
                D1Type::Text(&session.user_id),
                D1Type::Text(&session.token_hash),
                D1Type::Text(&session.family_id),
                D1Type::Real(session.expires_at as f64),
                D1Type::Real(session.created_at as f64),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        SendFuture::new(stmt.run())
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 insert session failed: {e}")))?;
        Ok(())
    }

    async fn rotate_refresh_session(
        &self,
        token_hash: &str,
        new_session: SessionRecord,
        now_secs: u64,
    ) -> Result<RefreshOutcome, AppError> {
        let db = self.db()?;
        let find_stmt = db
            .prepare(
                "SELECT id, user_id, family_id, expires_at, revoked_at, replaced_by_session_id FROM sessions WHERE token_hash = ?1 LIMIT 1",
            )
            .bind_refs(&[D1Type::Text(token_hash)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        let found: Option<DbSessionRow> = SendFuture::new(find_stmt.first(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;

        let Some(existing) = found else {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        };

        if existing.revoked_at.is_some() || existing.replaced_by_session_id.is_some() {
            let revoke_family = db
                .prepare("UPDATE sessions SET revoked_at = COALESCE(revoked_at, ?1) WHERE family_id = ?2")
                .bind_refs(&[
                    D1Type::Real(now_secs as f64),
                    D1Type::Text(&existing.family_id),
                ])
                .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
            SendFuture::new(revoke_family.run()).await.map_err(|e| {
                AppError::service_unavailable(format!("D1 family revoke failed: {e}"))
            })?;
            return Ok(RefreshOutcome::Replayed {
                user_id: existing.user_id,
                family_id: existing.family_id,
            });
        }

        if (existing.expires_at as u64) <= now_secs {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        }

        let Some(user) = self.find_user_by_id(&existing.user_id).await? else {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        };

        let update_old = db
            .prepare(
                "UPDATE sessions SET revoked_at = ?1, replaced_by_session_id = ?2 WHERE id = ?3 AND revoked_at IS NULL AND replaced_by_session_id IS NULL",
            )
            .bind_refs(&[
                D1Type::Real(now_secs as f64),
                D1Type::Text(&new_session.id),
                D1Type::Text(&existing.id),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;

        let insert_new = db
            .prepare(
                "INSERT INTO sessions (id, user_id, token_hash, family_id, expires_at, revoked_at, replaced_by_session_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6)",
            )
            .bind_refs(&[
                D1Type::Text(&new_session.id),
                // The caller leaves user_id and family_id empty; both come from the session being rotated.
                D1Type::Text(&existing.user_id),
                D1Type::Text(&new_session.token_hash),
                D1Type::Text(&existing.family_id),
                D1Type::Real(new_session.expires_at as f64),
                D1Type::Real(now_secs as f64),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;

        let results = SendFuture::new(db.batch(vec![update_old, insert_new]))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 batch rotation failed: {e}")))?;
        let updated_changes = results
            .first()
            .and_then(|r| r.meta().ok().flatten())
            .and_then(|m| m.changes)
            .unwrap_or(0);
        if updated_changes == 0 {
            return Ok(RefreshOutcome::Replayed {
                user_id: existing.user_id,
                family_id: existing.family_id,
            });
        }

        Ok(RefreshOutcome::Rotated {
            user,
            family_id: existing.family_id,
            new_session_id: new_session.id,
        })
    }

    async fn revoke_session_by_token_hash(
        &self,
        token_hash: &str,
        now_secs: u64,
    ) -> Result<Option<String>, AppError> {
        let db = self.db()?;
        let find_stmt = db
            .prepare("SELECT id, user_id, family_id, expires_at, revoked_at, replaced_by_session_id FROM sessions WHERE token_hash = ?1 LIMIT 1")
            .bind_refs(&[D1Type::Text(token_hash)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        let found: Option<DbSessionRow> = SendFuture::new(find_stmt.first(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;
        let Some(row) = found else {
            return Ok(None);
        };
        let revoke = db
            .prepare("UPDATE sessions SET revoked_at = COALESCE(revoked_at, ?1) WHERE id = ?2")
            .bind_refs(&[D1Type::Real(now_secs as f64), D1Type::Text(&row.id)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        SendFuture::new(revoke.run())
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 revoke failed: {e}")))?;
        Ok(Some(row.user_id))
    }

    async fn revoke_user_sessions(&self, user_id: &str, now_secs: u64) -> Result<(), AppError> {
        let db = self.db()?;
        let stmt = db
            .prepare("UPDATE sessions SET revoked_at = COALESCE(revoked_at, ?1) WHERE user_id = ?2")
            .bind_refs(&[D1Type::Real(now_secs as f64), D1Type::Text(user_id)])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        SendFuture::new(stmt.run()).await.map_err(|e| {
            AppError::service_unavailable(format!("D1 revoke user sessions failed: {e}"))
        })?;
        Ok(())
    }

    async fn get_active_signing_key(&self) -> Result<Option<JwksKeyRecord>, AppError> {
        let db = self.db()?;
        let stmt = db.prepare(
            "SELECT kid, alg, public_jwk_json, encrypted_private_key, status, created_at FROM jwks_keys WHERE status = 'active' ORDER BY created_at DESC LIMIT 1",
        );
        let row: Option<DbKeyRow> = SendFuture::new(stmt.first(None))
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 query error: {e}")))?;
        Ok(row.map(|r| JwksKeyRecord {
            kid: r.kid,
            alg: r.alg,
            public_jwk_json: r.public_jwk_json,
            encrypted_private_key: r.encrypted_private_key,
            status: r.status,
            created_at: r.created_at.max(0) as u64,
        }))
    }

    async fn list_public_jwks(&self) -> Result<Vec<PublicJwk>, AppError> {
        let db = self.db()?;
        let stmt = db.prepare(
            "SELECT public_jwk_json FROM jwks_keys WHERE status IN ('active', 'retiring') ORDER BY created_at DESC",
        );
        let result = SendFuture::new(stmt.all())
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 jwks query failed: {e}")))?;
        let rows: Vec<DbPublicJwkRow> = result
            .results()
            .map_err(|e| AppError::internal(format!("D1 results decode error: {e}")))?;
        rows.into_iter()
            .map(|r| {
                serde_json::from_str::<PublicJwk>(&r.public_jwk_json)
                    .map_err(|_| AppError::internal("corrupt JWK JSON in D1"))
            })
            .collect()
    }

    async fn rotate_signing_key(
        &self,
        new_key: JwksKeyRecord,
        audit: AuditEventInput,
    ) -> Result<(), AppError> {
        let db = self.db()?;
        let retire_stmt =
            db.prepare("UPDATE jwks_keys SET status = 'retiring' WHERE status = 'active'");
        let insert_stmt = db
            .prepare(
                "INSERT INTO jwks_keys (kid, alg, public_jwk_json, encrypted_private_key, status, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5, NULL)",
            )
            .bind_refs(&[
                D1Type::Text(&new_key.kid),
                D1Type::Text(&new_key.alg),
                D1Type::Text(&new_key.public_jwk_json),
                D1Type::Text(&new_key.encrypted_private_key),
                D1Type::Real(new_key.created_at as f64),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;

        let actor_arg = match audit.actor_user_id.as_deref() {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        };
        let target_arg = match audit.target_id.as_deref() {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        };
        let audit_stmt = db
            .prepare(
                "INSERT INTO audit_events (id, actor_user_id, event_type, target_id, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind_refs(&[
                D1Type::Text(&audit.id),
                actor_arg,
                D1Type::Text(&audit.event_type),
                target_arg,
                D1Type::Text(&audit.metadata_json),
                D1Type::Real(audit.created_at as f64),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;

        SendFuture::new(db.batch(vec![retire_stmt, insert_stmt, audit_stmt]))
            .await
            .map_err(|e| {
                AppError::service_unavailable(format!("D1 key rotation batch failed: {e}"))
            })?;
        Ok(())
    }

    async fn insert_audit_event(&self, audit: AuditEventInput) -> Result<(), AppError> {
        let db = self.db()?;
        let actor_arg = match audit.actor_user_id.as_deref() {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        };
        let target_arg = match audit.target_id.as_deref() {
            Some(v) => D1Type::Text(v),
            None => D1Type::Null,
        };
        let stmt = db
            .prepare(
                "INSERT INTO audit_events (id, actor_user_id, event_type, target_id, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind_refs(&[
                D1Type::Text(&audit.id),
                actor_arg,
                D1Type::Text(&audit.event_type),
                target_arg,
                D1Type::Text(&audit.metadata_json),
                D1Type::Real(audit.created_at as f64),
            ])
            .map_err(|e| AppError::internal(format!("D1 bind error: {e}")))?;
        SendFuture::new(stmt.run())
            .await
            .map_err(|e| AppError::service_unavailable(format!("D1 audit insert failed: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl StorageProbe for WorkerEnvAdapter {
    async fn check_r2_bindings(&self) -> Result<(), AppError> {
        let public_bucket = SendWrapper::new(self.env.bucket("PUBLIC_ASSETS").map_err(|e| {
            AppError::service_unavailable(format!("PUBLIC_ASSETS unavailable: {e}"))
        })?);
        let private_bucket = SendWrapper::new(self.env.bucket("PRIVATE_STORAGE").map_err(|e| {
            AppError::service_unavailable(format!("PRIVATE_STORAGE unavailable: {e}"))
        })?);
        SendFuture::new(public_bucket.head("__health_probe__"))
            .await
            .map_err(|e| {
                AppError::service_unavailable(format!("PUBLIC_ASSETS probe error: {e}"))
            })?;
        SendFuture::new(private_bucket.head("__health_probe__"))
            .await
            .map_err(|e| {
                AppError::service_unavailable(format!("PRIVATE_STORAGE probe error: {e}"))
            })?;
        Ok(())
    }
}

#[async_trait]
impl RateLimitPort for WorkerEnvAdapter {
    async fn check_limit(&self, key: &str) -> Result<bool, AppError> {
        let limiter = self
            .env
            .rate_limiter("AUTH_RATE_LIMITER")
            .map_err(|e| AppError::internal(format!("AUTH_RATE_LIMITER binding missing: {e}")))?;
        let outcome = limiter
            .limit(key.to_string())
            .await
            .map_err(|e| AppError::internal(format!("rate limit call failed: {e}")))?;
        Ok(outcome.success)
    }
}

impl SecretStore for WorkerEnvAdapter {
    fn password_pepper(&self) -> Result<String, AppError> {
        self.env
            .secret("PASSWORD_PEPPER")
            .map(|s| s.to_string())
            .map_err(|_| AppError::internal("missing PASSWORD_PEPPER secret"))
    }

    fn key_encryption_key(&self) -> Result<String, AppError> {
        self.env
            .secret("KEY_ENCRYPTION_KEY")
            .map(|s| s.to_string())
            .map_err(|_| AppError::internal("missing KEY_ENCRYPTION_KEY secret"))
    }
}

impl Clock for WorkerEnvAdapter {
    fn now_secs(&self) -> u64 {
        worker::Date::now().as_millis() / 1000
    }
}
