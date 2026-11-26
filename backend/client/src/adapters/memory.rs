use crate::config::{ClientConfig, Environment};
use crate::error::ClientError;
use crate::master_data::{
    CardArtRow, CardRow, CategoryRow, DailyCardRow, DailyMetaRow, DailyRows, LoveStatusRow,
    PositionRow, ReadingCardRows, ReadingConfigRows, SituationRow, SuggestionRow,
};
use crate::ports::{ClientPorts, Clock, HealthPort, MasterDataPort, RateLimitPort, SecretProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Master-data rows held by the memory adapter, one field per D1 table (`daily_meta` is the
/// single row or `None` when not seeded). Tests edit it directly, like direct SQL fixtures.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MasterDataFixture {
    pub love_statuses: Vec<LoveStatusRow>,
    pub categories: Vec<CategoryRow>,
    pub situations: Vec<SituationRow>,
    pub suggestions: Vec<SuggestionRow>,
    pub positions: Vec<PositionRow>,
    pub cards: Vec<CardRow>,
    pub card_art: Vec<CardArtRow>,
    pub daily_meta: Option<DailyMetaRow>,
    pub daily_cards: Vec<DailyCardRow>,
}

#[derive(Default)]
pub struct MemoryClientStore {
    pub d1_healthy: AtomicBool,
    pub public_r2_healthy: AtomicBool,
    pub rate_counts: Mutex<HashMap<String, u32>>,
    pub rate_limit_max: AtomicU64,
    pub now_secs: AtomicU64,
    pub turnstile_secret: String,
    pub master_data: Mutex<MasterDataFixture>,
    /// Number of `MasterDataPort` calls, including failed ones.
    pub master_data_reads: AtomicU64,
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
            master_data: Mutex::new(MasterDataFixture::default()),
            master_data_reads: AtomicU64::new(0),
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
            master_data: self.clone(),
            secrets: self.clone(),
            clock: self.clone(),
        }
    }

    fn read_master_data(&self) -> Result<MasterDataFixture, ClientError> {
        self.master_data_reads.fetch_add(1, Ordering::SeqCst);
        if !self.d1_healthy.load(Ordering::SeqCst) {
            return Err(ClientError::service_unavailable("simulated D1 outage"));
        }
        Ok(self.master_data.lock().unwrap().clone())
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
impl MasterDataPort for MemoryClientStore {
    async fn reading_cards(&self) -> Result<ReadingCardRows, ClientError> {
        let data = self.read_master_data()?;
        Ok(ReadingCardRows {
            love_statuses: data.love_statuses,
            cards: data.cards,
        })
    }

    async fn daily_cards(&self) -> Result<DailyRows, ClientError> {
        let data = self.read_master_data()?;
        let meta = data.daily_meta.ok_or_else(|| {
            ClientError::service_unavailable("daily_meta row missing; run the seed")
        })?;
        Ok(DailyRows {
            meta,
            love_statuses: data.love_statuses,
            cards: data.daily_cards,
        })
    }

    async fn card_art(&self) -> Result<Vec<CardArtRow>, ClientError> {
        Ok(self.read_master_data()?.card_art)
    }

    async fn reading_config(&self) -> Result<ReadingConfigRows, ClientError> {
        let data = self.read_master_data()?;
        Ok(ReadingConfigRows {
            categories: data.categories,
            situations: data.situations,
            suggestions: data.suggestions,
            positions: data.positions,
            love_statuses: data.love_statuses,
        })
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
