use crate::config::{ClientConfig, Environment};
use crate::error::ClientError;
use crate::ports::{ClientPorts, Clock, HealthPort, RateLimitPort, SecretProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct MemoryClientStore {
    pub d1_healthy: AtomicBool,
    pub public_r2_healthy: AtomicBool,
    pub rate_counts: Mutex<HashMap<String, u32>>,
    pub rate_limit_max: AtomicU64,
    pub now_secs: AtomicU64,
    pub turnstile_secret: String,
}

impl MemoryClientStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            d1_healthy: AtomicBool::new(true),
            public_r2_healthy: AtomicBool::new(true),
            rate_counts: Mutex::new(HashMap::new()),
            rate_limit_max: AtomicU64::new(60),
            now_secs: AtomicU64::new(1_700_000_000),
            turnstile_secret: "1x0000000000000000000000000000000AA".to_string(),
        })
    }

    pub fn into_ports(self: &Arc<Self>, env: Environment) -> ClientPorts {
        ClientPorts {
            config: ClientConfig {
                environment: env,
                allowed_origins: vec!["https://staging.maemorkratai.com".to_string()],
                max_body_bytes: 32_768,
                turnstile_verify_url: "https://challenges.cloudflare.com/turnstile/v0/siteverify"
                    .to_string(),
                jwks_url: "https://admin-api-staging.maemorkratai.com/.well-known/jwks.json"
                    .to_string(),
                jwt_issuer: "https://admin-api-staging.maemorkratai.com".to_string(),
                jwt_audience: "maemorkratai-client".to_string(),
            },
            health: self.clone(),
            rate_limiter: self.clone(),
            secrets: self.clone(),
            clock: self.clone(),
        }
    }
}

#[async_trait]
impl HealthPort for MemoryClientStore {
    async fn check_d1(&self) -> Result<(), ClientError> {
        if self.d1_healthy.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(ClientError::service_unavailable("simulated D1 outage"))
        }
    }

    async fn check_public_r2(&self) -> Result<(), ClientError> {
        if self.public_r2_healthy.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(ClientError::service_unavailable(
                "simulated PUBLIC_ASSETS outage",
            ))
        }
    }
}

#[async_trait]
impl RateLimitPort for MemoryClientStore {
    async fn check_limit(&self, key: &str) -> Result<bool, ClientError> {
        let max = self.rate_limit_max.load(Ordering::SeqCst) as u32;
        let mut counts = self.rate_counts.lock().unwrap();
        let count = counts.entry(key.to_string()).or_insert(0);
        *count += 1;
        Ok(*count <= max)
    }
}

impl SecretProvider for MemoryClientStore {
    fn turnstile_secret(&self) -> Result<String, ClientError> {
        Ok(self.turnstile_secret.clone())
    }
}

impl Clock for MemoryClientStore {
    fn now_secs(&self) -> u64 {
        self.now_secs.load(Ordering::SeqCst)
    }
}
