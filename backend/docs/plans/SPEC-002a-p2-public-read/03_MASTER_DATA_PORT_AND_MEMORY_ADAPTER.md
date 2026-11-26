---
model: sonnet
effort: medium
reason: "Complete code supplied; touches ports, both adapters and a new test support module."
---

# Task 03: `MasterDataPort`, memory adapter and D1 wiring

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Depends on tasks 01 and 02. All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Create `client/tests/support/mod.rs` (harness: `block_on`, `body_json`, `tiny_fixture`, `tiny_store`)
- [ ] Create `client/tests/spec_002a_scenarios.rs` (port tests; task 04 and 05 append to it)
- [ ] Modify `client/src/ports.rs` (add `MasterDataPort`, field `ClientPorts.master_data`)
- [ ] Modify `client/src/adapters/memory.rs` (fixture, read counter, port impl)
- [ ] Modify `client/src/adapters/worker_env.rs` (port impl on D1 binding `DB`)
- [ ] Red, implement, green, standards check

The port has read methods only. There is no write method, so no code path in `client` can write master-data tables (spec §6). The memory adapter reuses the existing `d1_healthy` switch as the D1-failure switch, so one switch simulates a D1 outage for `/health` and the master-data GETs alike. `master_data_reads` counts port calls; task 04 uses it to prove that a `405` never touches D1.

## 2. Harness

### File: `client/tests/support/mod.rs` (new, full content)
```rust
// Shared helpers for the SPEC-002a test binaries. Each file under tests/ is compiled as its
// own crate and uses a different subset of these helpers, so unused ones would warn per binary.
#![allow(dead_code)]

use backend_client::adapters::memory::{MasterDataFixture, MemoryClientStore};
use backend_client::master_data::{
    CardArtRow, CardRow, CategoryRow, DailyCardRow, DailyMetaRow, LoveStatusRow, PositionRow,
    SituationRow, SuggestionRow, TextMap,
};
use serde_json::Value;
use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub fn block_on<F: Future>(fut: F) -> F::Output {
    fn noop(_: *const ()) {}
    fn clone(ptr: *const ()) -> RawWaker {
        RawWaker::new(ptr, &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut pinned = pin!(fut);
    loop {
        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Body limit 4 MiB: the largest master-data body is about 359 KB.
pub async fn body_text(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

pub async fn body_json(res: axum::response::Response) -> Value {
    serde_json::from_str(&body_text(res).await).unwrap()
}

fn s(text: &str) -> String {
    text.to_string()
}

fn texts(pairs: &[(&str, Option<&str>)]) -> TextMap {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(str::to_string)))
        .collect()
}

fn love_status(key: &str, daily_label: &str, reading_label: &str, daily: i64, reading: i64) -> LoveStatusRow {
    LoveStatusRow {
        key: s(key),
        daily_label: s(daily_label),
        reading_label: s(reading_label),
        daily_sort_order: daily,
        reading_sort_order: reading,
    }
}

fn daily_card(card_id: &str, name: &str, sort_order: i64, love_by_status: TextMap) -> DailyCardRow {
    DailyCardRow {
        card_id: s(card_id),
        name: s(name),
        arcana: s("major"),
        reading_mode: s("daily_single"),
        time_horizon: s("today"),
        orientation: s("upright_only"),
        content_status: s("assistant_draft_pending_review"),
        approved_for_web: false,
        theme: s("theme"),
        overview: s("overview"),
        work: s("work"),
        money: s("money"),
        rabbit_message: s("rabbit"),
        colors: s("สีเหลือง สีขาว"),
        affirmation: s("affirmation"),
        summary_theme: Some(s("summary")),
        overview_theme: None,
        sort_order,
        love_by_status,
    }
}

/// Small hand-built data set, rows stored in reverse order so the assembly's sorting is exercised.
/// Love statuses: single (daily 0, reading 0), ex_partner (daily 1, reading 2),
/// complicated (daily 2, reading 1). Cards: the-fool (art), six-of-wands (no art).
pub fn tiny_fixture() -> MasterDataFixture {
    MasterDataFixture {
        love_statuses: vec![
            love_status("complicated", "ความสัมพันธ์ซับซ้อน", "ซับซ้อน", 2, 1),
            love_status("ex_partner", "คนรักเก่า", "เลิกกัน", 1, 2),
            love_status("single", "คนโสด", "คนโสด", 0, 0),
        ],
        categories: vec![
            CategoryRow { key: s("work"), label: s("การงาน"), icon: s("☆"), hint: s("งานและอาชีพ"), sort_order: 1 },
            CategoryRow { key: s("love"), label: s("ความรัก"), icon: s("♡"), hint: s("ความรู้สึกและความสัมพันธ์"), sort_order: 0 },
        ],
        situations: vec![
            SituationRow { key: s("job_seeking"), category: Some(s("work")), label: s("มองหางานใหม่"), sort_order: 1 },
            SituationRow { key: s("current_job"), category: Some(s("work")), label: s("งานปัจจุบัน"), sort_order: 0 },
        ],
        suggestions: vec![
            SuggestionRow { id: s("work-1"), category: Some(s("work")), text: s("q-work"), sort_order: 0 },
            SuggestionRow { id: s("love-1"), category: Some(s("love")), text: s("q-love"), sort_order: 0 },
        ],
        positions: vec![
            PositionRow { key: s("future"), label: s("อนาคต"), sort_order: 2 },
            PositionRow { key: s("present"), label: s("ปัจจุบัน"), sort_order: 1 },
            PositionRow { key: s("past"), label: s("อดีต"), sort_order: 0 },
        ],
        cards: vec![
            CardRow {
                id: s("six-of-wands"),
                name: s("Six of Wands"),
                keyword: s("ชัยชนะ"),
                art: None,
                sort_order: 1,
                meanings: texts(&[("single", Some("m-single"))]),
                details: texts(&[("single", Some("d-single"))]),
            },
            CardRow {
                id: s("the-fool"),
                name: s("The Fool"),
                keyword: s("การเริ่มต้น"),
                art: Some(s("/cards/v3/the-fool-e11e1ee168a4.webp")),
                sort_order: 0,
                meanings: texts(&[
                    ("single", Some("m1")),
                    ("ex_partner", Some("m2")),
                    ("complicated", Some("m3")),
                ]),
                details: texts(&[
                    ("single", Some("d1")),
                    ("ex_partner", Some("d2")),
                    ("complicated", Some("d3")),
                ]),
            },
        ],
        card_art: vec![CardArtRow {
            id: s("the-fool"),
            art: s("/cards/v3/the-fool-e11e1ee168a4.webp"),
            width: 637,
            height: 1024,
            rendered_width: 600,
            rendered_height: 965,
            sha256: "a".repeat(64),
            bytes: 51_234,
            sort_order: 0,
        }],
        daily_meta: Some(DailyMetaRow {
            schema_version: 1,
            version: s("daily-v1-draft-2026-11-21"),
            updated: s("2026-11-21"),
            status: s("draft_pending_owner_review"),
            scope: s("daily"),
            auto_fallback_to_general_or_spread: true,
        }),
        daily_cards: vec![
            daily_card(
                "six-of-wands",
                "Six of Wands",
                1,
                texts(&[("single", Some("l1")), ("ex_partner", Some("l2")), ("complicated", Some("l3"))]),
            ),
            daily_card(
                "the-fool",
                "The Fool",
                0,
                texts(&[("single", Some("l1")), ("ex_partner", Some("l2")), ("complicated", None)]),
            ),
        ],
    }
}

pub fn tiny_store() -> Arc<MemoryClientStore> {
    let store = MemoryClientStore::new();
    *store.master_data.lock().unwrap() = tiny_fixture();
    store
}
```

## 3. Tests

### File: `client/tests/spec_002a_scenarios.rs` (new, full content; tasks 04 and 05 append tests)
```rust
mod support;

use axum::http::StatusCode;
use backend_client::adapters::memory::MemoryClientStore;
use backend_client::config::Environment;
use std::sync::atomic::Ordering;
use support::{block_on, tiny_store};

#[test]
fn memory_port_returns_fixture_rows_and_counts_reads() {
    block_on(async {
        let store = tiny_store();
        let ports = store.into_ports(Environment::Staging);
        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 0);

        let cards = ports.master_data.reading_cards().await.unwrap();
        assert_eq!(cards.cards.len(), 2);
        assert_eq!(cards.love_statuses.len(), 3);
        let daily = ports.master_data.daily_cards().await.unwrap();
        assert_eq!(daily.cards.len(), 2);
        assert!(daily.meta.auto_fallback_to_general_or_spread);
        assert_eq!(ports.master_data.card_art().await.unwrap().len(), 1);
        let config = ports.master_data.reading_config().await.unwrap();
        assert_eq!(config.categories.len(), 2);
        assert_eq!(config.positions.len(), 3);

        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 4);
    });
}

#[test]
fn memory_port_reports_d1_outage_as_503_on_every_method() {
    block_on(async {
        let store = tiny_store();
        store.d1_healthy.store(false, Ordering::SeqCst);
        let ports = store.into_ports(Environment::Staging);
        let statuses = [
            ports.master_data.reading_cards().await.unwrap_err().status,
            ports.master_data.daily_cards().await.unwrap_err().status,
            ports.master_data.card_art().await.unwrap_err().status,
            ports.master_data.reading_config().await.unwrap_err().status,
        ];
        assert_eq!(statuses, [StatusCode::SERVICE_UNAVAILABLE; 4]);
    });
}

#[test]
fn memory_port_without_daily_meta_is_503_and_empty_store_reads_empty_lists() {
    block_on(async {
        let store = MemoryClientStore::new();
        let ports = store.into_ports(Environment::Staging);
        let err = ports.master_data.daily_cards().await.unwrap_err();
        assert_eq!(err.status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(ports.master_data.reading_cards().await.unwrap().cards.is_empty());
        assert!(ports.master_data.card_art().await.unwrap().is_empty());
    });
}
```

## 4. Red
```bash
cargo test -p backend-client --test spec_002a_scenarios
```
Expected: FAIL to compile with errors including `unresolved import backend_client::adapters::memory::MasterDataFixture`, `no field master_data on type Arc<MemoryClientStore>` and `no field master_data on type ClientPorts`.

## 5. Implementation

### File: `client/src/ports.rs` (full content)
```rust
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
```

### File: `client/src/adapters/memory.rs` (full content)
```rust
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
```

### File: `client/src/adapters/worker_env.rs` (exact diffs)
1. Replace the import block at the top of the file:
```rust
use crate::config::{ClientConfig, Environment};
use crate::error::ClientError;
use crate::ports::{ClientPorts, Clock, HealthPort, RateLimitPort, SecretProvider};
use async_trait::async_trait;
use std::sync::Arc;
use worker::send::{SendFuture, SendWrapper};
use worker::Env;
```
with:
```rust
use crate::adapters::d1_master_data;
use crate::config::{ClientConfig, Environment};
use crate::error::ClientError;
use crate::master_data::{CardArtRow, DailyRows, ReadingCardRows, ReadingConfigRows};
use crate::ports::{ClientPorts, Clock, HealthPort, MasterDataPort, RateLimitPort, SecretProvider};
use async_trait::async_trait;
use std::sync::Arc;
use worker::send::{SendFuture, SendWrapper};
use worker::{D1Database, Env};
```
2. In `into_ports`, replace:
```rust
            health: shared.clone(),
            rate_limiter: shared.clone(),
            secrets: shared.clone(),
            clock: shared,
```
with:
```rust
            health: shared.clone(),
            rate_limiter: shared.clone(),
            master_data: shared.clone(),
            secrets: shared.clone(),
            clock: shared,
```
3. Inside `impl WorkerClientAdapter { ... }`, after the `into_ports` method, add:
```rust

    fn master_db(&self) -> Result<D1Database, ClientError> {
        self.env
            .d1("DB")
            .map_err(|e| ClientError::service_unavailable(format!("D1 binding error: {e}")))
    }
```
4. After the `impl HealthPort for WorkerClientAdapter { ... }` block, add:
```rust

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
        let sets = d1_master_data::query_row_sets(
            self.master_db()?,
            d1_master_data::READING_CONFIG_BATCH,
        )
        .await?;
        d1_master_data::reading_config_from_sets(sets)
    }
}
```
Nothing else in `worker_env.rs` changes.

## 6. Green + Standards Check
```bash
cargo test -p backend-client --test spec_002a_scenarios
cargo test -p backend-client
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked -- -D warnings
make check
```
Expected: `spec_002a_scenarios` 3 passed; all `backend-client` tests pass, including the unchanged `spec_001_scenarios`; fmt (after `cargo fmt --all` reflows the fixture literals), both clippy runs and `make check` are clean. If the wasm32 build reports that the `MasterDataPort` future is not `Send`, check that `query_row_sets` still wraps its whole body in `SendFuture` (task 02); do not add `?Send` to the trait.
