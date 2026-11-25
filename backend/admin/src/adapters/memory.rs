use crate::auth::jwt::PublicJwk;
use crate::config::{AppConfig, Environment};
use crate::error::AppError;
use crate::ports::{
    AppPorts, AuditEventInput, AuthRepo, Clock, JwksKeyRecord, RateLimitPort, RefreshOutcome,
    SecretStore, SessionRecord, StorageProbe, UserRecord,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct MemoryStore {
    pub users: Mutex<HashMap<String, UserRecord>>,
    pub sessions: Mutex<HashMap<String, SessionRecord>>,
    pub jwks_keys: Mutex<Vec<JwksKeyRecord>>,
    pub audit_events: Mutex<Vec<AuditEventInput>>,
    pub rate_counts: Mutex<HashMap<String, u32>>,
    pub rate_limit_max: AtomicU64,
    pub db_healthy: AtomicBool,
    pub r2_healthy: AtomicBool,
    pub now_secs: AtomicU64,
    pub pepper: String,
    pub kek_b64: String,
}

impl MemoryStore {
    pub fn new(pepper: impl Into<String>, kek_b64: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            db_healthy: AtomicBool::new(true),
            r2_healthy: AtomicBool::new(true),
            rate_limit_max: AtomicU64::new(100),
            now_secs: AtomicU64::new(1_700_000_000),
            pepper: pepper.into(),
            kek_b64: kek_b64.into(),
            ..Default::default()
        })
    }

    pub fn into_ports(self: &Arc<Self>, env: Environment) -> AppPorts {
        AppPorts {
            config: AppConfig {
                environment: env,
                allowed_origins: vec!["https://admin.maemorkratai.test".to_string()],
                jwt_issuer: "https://admin-api.maemorkratai.test".to_string(),
                jwt_audience: "maemorkratai-admin".to_string(),
                access_token_ttl_secs: 900,
                refresh_token_ttl_secs: 604_800,
                cookie_name: "mmk_admin_rt".to_string(),
                max_body_bytes: 65_536,
            },
            repo: self.clone(),
            storage: self.clone(),
            rate_limiter: self.clone(),
            secrets: self.clone(),
            clock: self.clone(),
        }
    }
}

#[async_trait]
impl AuthRepo for MemoryStore {
    async fn health_check_db(&self) -> Result<(), AppError> {
        if self.db_healthy.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AppError::service_unavailable("simulated D1 outage"))
        }
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, AppError> {
        let guard = self.users.lock().unwrap();
        Ok(guard.values().find(|u| u.email == email).cloned())
    }

    async fn find_user_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, AppError> {
        let guard = self.users.lock().unwrap();
        Ok(guard.get(user_id).cloned())
    }

    async fn insert_session(&self, session: SessionRecord) -> Result<(), AppError> {
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id.clone(), session);
        Ok(())
    }

    async fn rotate_refresh_session(
        &self,
        token_hash: &str,
        new_session: SessionRecord,
        now_secs: u64,
    ) -> Result<RefreshOutcome, AppError> {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(existing_id) = sessions
            .values()
            .find(|s| s.token_hash == token_hash)
            .map(|s| s.id.clone())
        else {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        };

        let existing = sessions.get(&existing_id).unwrap().clone();
        if existing.revoked_at.is_some() || existing.replaced_by_session_id.is_some() {
            for sess in sessions.values_mut() {
                if sess.family_id == existing.family_id && sess.revoked_at.is_none() {
                    sess.revoked_at = Some(now_secs);
                }
            }
            return Ok(RefreshOutcome::Replayed {
                user_id: existing.user_id,
                family_id: existing.family_id,
            });
        }

        if existing.expires_at <= now_secs {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        }

        let user = self.users.lock().unwrap().get(&existing.user_id).cloned();
        let Some(user) = user else {
            return Ok(RefreshOutcome::NotFoundOrExpired);
        };

        if let Some(entry) = sessions.get_mut(&existing_id) {
            entry.revoked_at = Some(now_secs);
            entry.replaced_by_session_id = Some(new_session.id.clone());
        }
        let mut inserted = new_session;
        inserted.user_id = existing.user_id.clone();
        inserted.family_id = existing.family_id.clone();
        let new_id = inserted.id.clone();
        sessions.insert(new_id.clone(), inserted);

        Ok(RefreshOutcome::Rotated {
            user,
            family_id: existing.family_id,
            new_session_id: new_id,
        })
    }

    async fn revoke_session_by_token_hash(
        &self,
        token_hash: &str,
        now_secs: u64,
    ) -> Result<Option<String>, AppError> {
        let mut sessions = self.sessions.lock().unwrap();
        for sess in sessions.values_mut() {
            if sess.token_hash == token_hash {
                if sess.revoked_at.is_none() {
                    sess.revoked_at = Some(now_secs);
                }
                return Ok(Some(sess.user_id.clone()));
            }
        }
        Ok(None)
    }

    async fn revoke_user_sessions(&self, user_id: &str, now_secs: u64) -> Result<(), AppError> {
        let mut sessions = self.sessions.lock().unwrap();
        for sess in sessions.values_mut() {
            if sess.user_id == user_id && sess.revoked_at.is_none() {
                sess.revoked_at = Some(now_secs);
            }
        }
        Ok(())
    }

    async fn get_active_signing_key(&self) -> Result<Option<JwksKeyRecord>, AppError> {
        let keys = self.jwks_keys.lock().unwrap();
        Ok(keys
            .iter()
            .filter(|k| k.status == "active")
            .max_by_key(|k| k.created_at)
            .cloned())
    }

    async fn list_public_jwks(&self) -> Result<Vec<PublicJwk>, AppError> {
        let keys = self.jwks_keys.lock().unwrap();
        keys.iter()
            .filter(|k| k.status == "active" || k.status == "retiring")
            .map(|k| {
                serde_json::from_str::<PublicJwk>(&k.public_jwk_json)
                    .map_err(|_| AppError::internal("corrupt JWK JSON"))
            })
            .collect()
    }

    async fn rotate_signing_key(
        &self,
        new_key: JwksKeyRecord,
        audit: AuditEventInput,
    ) -> Result<(), AppError> {
        let mut keys = self.jwks_keys.lock().unwrap();
        for k in keys.iter_mut() {
            if k.status == "active" {
                k.status = "retiring".to_string();
            }
        }
        keys.push(new_key);
        self.audit_events.lock().unwrap().push(audit);
        Ok(())
    }

    async fn insert_audit_event(&self, audit: AuditEventInput) -> Result<(), AppError> {
        self.audit_events.lock().unwrap().push(audit);
        Ok(())
    }
}

#[async_trait]
impl StorageProbe for MemoryStore {
    async fn check_r2_bindings(&self) -> Result<(), AppError> {
        if self.r2_healthy.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AppError::service_unavailable("simulated R2 outage"))
        }
    }
}

#[async_trait]
impl RateLimitPort for MemoryStore {
    async fn check_limit(&self, key: &str) -> Result<bool, AppError> {
        let max = self.rate_limit_max.load(Ordering::SeqCst) as u32;
        let mut counts = self.rate_counts.lock().unwrap();
        let entry = counts.entry(key.to_string()).or_insert(0);
        *entry += 1;
        Ok(*entry <= max)
    }
}

impl SecretStore for MemoryStore {
    fn password_pepper(&self) -> Result<String, AppError> {
        Ok(self.pepper.clone())
    }

    fn key_encryption_key(&self) -> Result<String, AppError> {
        Ok(self.kek_b64.clone())
    }
}

impl Clock for MemoryStore {
    fn now_secs(&self) -> u64 {
        self.now_secs.load(Ordering::SeqCst)
    }
}
