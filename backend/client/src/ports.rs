use crate::config::ClientConfig;
use crate::error::ClientError;
use crate::master_data::{CardArtRow, DailyRows, ReadingCardRows, ReadingConfigRows};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait HealthPort: Send + Sync {
    async fn check_d1(&self) -> Result<(), ClientError>;
    async fn check_public_r2(&self) -> Result<(), ClientError>;
}

#[async_trait]
pub trait RateLimitPort: Send + Sync {
    async fn check_limit(&self, key: &str) -> Result<bool, ClientError>;
}

/// Read-only master data (SPEC-002a). There is no write method on purpose: `client` shares
/// the D1 binding with `admin`, and spec §6 requires that `client` has no code path that
/// writes master-data tables. Rows come back unsorted; `crate::master_data` sorts them.
#[async_trait]
pub trait MasterDataPort: Send + Sync {
    async fn reading_cards(&self) -> Result<ReadingCardRows, ClientError>;
    async fn daily_cards(&self) -> Result<DailyRows, ClientError>;
    async fn card_art(&self) -> Result<Vec<CardArtRow>, ClientError>;
    async fn reading_config(&self) -> Result<ReadingConfigRows, ClientError>;
}

pub trait SecretProvider: Send + Sync {
    fn turnstile_secret(&self) -> Result<String, ClientError>;
}

pub trait Clock: Send + Sync {
    fn now_secs(&self) -> u64;
}

#[derive(Clone)]
pub struct ClientPorts {
    pub config: ClientConfig,
    pub health: Arc<dyn HealthPort>,
    pub rate_limiter: Arc<dyn RateLimitPort>,
    pub master_data: Arc<dyn MasterDataPort>,
    pub secrets: Arc<dyn SecretProvider>,
    pub clock: Arc<dyn Clock>,
}
