use crate::adapters::d1_master_data;
use crate::config::{ClientConfig, Environment};
use crate::error::ClientError;
use crate::master_data::{CardArtRow, DailyRows, ReadingCardRows, ReadingConfigRows};
use crate::ports::{ClientPorts, Clock, HealthPort, MasterDataPort, RateLimitPort, SecretProvider};
use async_trait::async_trait;
use std::sync::Arc;
use worker::send::{SendFuture, SendWrapper};
use worker::{D1Database, Env};

#[derive(Clone)]
pub struct WorkerClientAdapter {
    env: SendWrapper<Env>,
}

impl WorkerClientAdapter {
    pub fn new(env: Env) -> Self {
        Self {
            env: SendWrapper::new(env),
        }
    }

    pub fn load_config(&self) -> Result<ClientConfig, ClientError> {
        let env_str = self
            .env
            .var("ENVIRONMENT")
            .map(|v| v.to_string())
            .map_err(|_| ClientError::internal("missing ENVIRONMENT var"))?;
        let environment = Environment::parse(&env_str)?;

        let origins_raw = self
            .env
            .var("ALLOWED_ORIGINS")
            .map(|v| v.to_string())
            .map_err(|_| ClientError::internal("missing ALLOWED_ORIGINS var"))?;
        let allowed_origins = ClientConfig::parse_origins(&origins_raw)?;

        let max_body_bytes = self
            .env
            .var("MAX_BODY_BYTES")
            .ok()
            .and_then(|v| v.to_string().parse::<usize>().ok())
            .unwrap_or(32_768);

        let turnstile_verify_url = self
            .env
            .var("TURNSTILE_VERIFY_URL")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| {
                "https://challenges.cloudflare.com/turnstile/v0/siteverify".to_string()
            });
        let jwks_url = self
            .env
            .var("JWKS_URL")
            .map(|v| v.to_string())
            .map_err(|_| ClientError::internal("missing JWKS_URL var"))?;
        let jwt_issuer = self
            .env
            .var("JWT_ISSUER")
            .map(|v| v.to_string())
            .map_err(|_| ClientError::internal("missing JWT_ISSUER var"))?;
        let jwt_audience = self
            .env
            .var("JWT_AUDIENCE")
            .map(|v| v.to_string())
            .map_err(|_| ClientError::internal("missing JWT_AUDIENCE var"))?;

        Ok(ClientConfig {
            environment,
            allowed_origins,
            max_body_bytes,
            turnstile_verify_url,
            jwks_url,
            jwt_issuer,
            jwt_audience,
        })
    }

    pub fn into_ports(self) -> Result<ClientPorts, ClientError> {
        let config = self.load_config()?;
        let shared = Arc::new(self);
        Ok(ClientPorts {
            config,
            health: shared.clone(),
            rate_limiter: shared.clone(),
            master_data: shared.clone(),
            secrets: shared.clone(),
            clock: shared,
        })
    }

    fn master_db(&self) -> Result<D1Database, ClientError> {
        self.env
            .d1("DB")
            .map_err(|e| ClientError::service_unavailable(format!("D1 binding error: {e}")))
    }
}

#[async_trait]
impl HealthPort for WorkerClientAdapter {
    async fn check_d1(&self) -> Result<(), ClientError> {
        let db = self
            .env
            .d1("DB")
            .map_err(|e| ClientError::service_unavailable(format!("D1 binding error: {e}")))?;
        let stmt = db.prepare("SELECT 1 AS ok");
        SendFuture::new(stmt.first::<serde_json::Value>(None))
            .await
            .map_err(|e| ClientError::service_unavailable(format!("D1 query error: {e}")))?;
        Ok(())
    }

    async fn check_public_r2(&self) -> Result<(), ClientError> {
        let bucket = SendWrapper::new(self.env.bucket("PUBLIC_ASSETS").map_err(|e| {
            ClientError::service_unavailable(format!("PUBLIC_ASSETS binding error: {e}"))
        })?);
        SendFuture::new(bucket.head("__health_probe__"))
            .await
            .map_err(|e| {
                ClientError::service_unavailable(format!("PUBLIC_ASSETS probe error: {e}"))
            })?;
        Ok(())
    }
}

#[async_trait]
impl MasterDataPort for WorkerClientAdapter {
    async fn reading_cards(&self) -> Result<ReadingCardRows, ClientError> {
        let sets =
            d1_master_data::query_row_sets(self.master_db()?, d1_master_data::READING_CARDS_BATCH)
                .await?;
        d1_master_data::reading_cards_from_sets(sets)
    }

    async fn daily_cards(&self) -> Result<DailyRows, ClientError> {
        let sets =
            d1_master_data::query_row_sets(self.master_db()?, d1_master_data::DAILY_CARDS_BATCH)
                .await?;
        d1_master_data::daily_from_sets(sets)
    }

    async fn card_art(&self) -> Result<Vec<CardArtRow>, ClientError> {
        let sets =
            d1_master_data::query_row_sets(self.master_db()?, d1_master_data::CARD_ART_BATCH)
                .await?;
        d1_master_data::card_art_from_sets(sets)
    }

    async fn reading_config(&self) -> Result<ReadingConfigRows, ClientError> {
        let sets =
            d1_master_data::query_row_sets(self.master_db()?, d1_master_data::READING_CONFIG_BATCH)
                .await?;
        d1_master_data::reading_config_from_sets(sets)
    }
}

#[async_trait]
impl RateLimitPort for WorkerClientAdapter {
    async fn check_limit(&self, key: &str) -> Result<bool, ClientError> {
        let limiter = self
            .env
            .rate_limiter("PUBLIC_RATE_LIMITER")
            .map_err(|e| ClientError::internal(format!("PUBLIC_RATE_LIMITER missing: {e}")))?;
        let outcome = limiter
            .limit(key.to_string())
            .await
            .map_err(|e| ClientError::internal(format!("rate limit call failed: {e}")))?;
        Ok(outcome.success)
    }
}

impl SecretProvider for WorkerClientAdapter {
    fn turnstile_secret(&self) -> Result<String, ClientError> {
        self.env
            .secret("TURNSTILE_SECRET_KEY")
            .map(|s| s.to_string())
            .map_err(|_| ClientError::service_unavailable("TURNSTILE_SECRET_KEY missing"))
    }
}

impl Clock for WorkerClientAdapter {
    fn now_secs(&self) -> u64 {
        worker::Date::now().as_millis() / 1000
    }
}
