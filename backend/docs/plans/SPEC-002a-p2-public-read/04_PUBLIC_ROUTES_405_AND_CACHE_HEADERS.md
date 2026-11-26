---
model: sonnet
effort: medium
reason: "Complete code and tests supplied; router, one route module, error constructor, fallback."
---

# Task 04: Four public GETs, 405/404 fallback, cache headers

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Depends on tasks 01 to 03. All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Replace `client/tests/spec_002a_scenarios.rs` with the full content in §3 (keeps the 3 task 03 tests, adds 9)
- [ ] Create `client/src/routes/master_data.rs`
- [ ] Modify `client/src/routes/mod.rs`, `client/src/routes/health.rs`, `client/src/error.rs`, `client/src/lib.rs`
- [ ] Red, implement, green, standards check

Behavior (battleground record `docs/battlegrounds/SPEC-002a-p2-public-read/04-method-fallback-and-cache-headers/debate.md`):
- `GET /v1/cards`, `/v1/daily-cards`, `/v1/card-art`, `/v1/reading-config` return `200` with `Cache-Control: public, max-age=600` (D05) and `Vary: Origin` always, also without an `Origin` request header. Workers Cache honors `Vary` (D13); without it, a response cached for a request with no `Origin` would be served to browsers that need `Access-Control-Allow-Origin`.
- `security_pipeline_middleware` already keeps a `Cache-Control` set by a handler and forces `no-store` otherwise, so the middleware does not change. The handler sets the public header on status `200` only.
- Every error (`503` D1, `429`, `404`, `405`, `500`) comes from `ClientError::into_response_with_request_id`, which sets `no-store`.
- `Router::method_not_allowed_fallback` returns `405` + envelope (`METHOD_NOT_ALLOWED`) for any non-GET/HEAD method on a known path, including `/health` (D12). axum 0.8.9 adds `Allow: GET,HEAD` from the route's method router after the handler (`routing/route.rs`, `set_allow_header`). An unknown path keeps the existing `404` fallback for any method. `OPTIONS` is answered by the middleware before routing (SPEC-001 CORS behavior unchanged).
- A POST/PUT/PATCH/DELETE carrying a disallowed `Origin` is still rejected with `403` by the middleware before routing (D04); this task does not change that.

SPEC-001 impact (estimate risk 7): no test in `client/tests/spec_001_scenarios.rs` asserts axum's bare `405`. `POST /health` appears twice there, and both requests are stopped by the middleware before routing (`413` for the oversized body, `403` for the disallowed origin). They pass unchanged.

## 2. Harness
Uses `client/tests/support/mod.rs` from task 03 (`block_on`, `body_json`, `body_text`, `tiny_store`). No additions.

## 3. Tests

### File: `client/tests/spec_002a_scenarios.rs` (full content, replaces the task 03 version)
```rust
mod support;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use backend_client::adapters::memory::MemoryClientStore;
use backend_client::build_router;
use backend_client::config::Environment;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use support::{block_on, body_json, body_text, tiny_store};
use tower::ServiceExt;

const MASTER_DATA_PATHS: [&str; 4] = [
    "/v1/cards",
    "/v1/daily-cards",
    "/v1/card-art",
    "/v1/reading-config",
];
const KNOWN_PATHS: [&str; 5] = [
    "/health",
    "/v1/cards",
    "/v1/daily-cards",
    "/v1/card-art",
    "/v1/reading-config",
];
const WRITE_METHODS: [Method; 4] = [Method::POST, Method::PUT, Method::PATCH, Method::DELETE];
const ALLOWED_ORIGIN: &str = "https://staging.maemorkratai.com";
const PUBLIC_CACHE: &str = "public, max-age=600";

fn app(store: &Arc<MemoryClientStore>) -> Router {
    build_router(store.into_ports(Environment::Staging))
}

async fn send(app: &Router, method: Method, path: &str, origin: Option<&str>) -> Response {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(origin) = origin {
        req = req.header(header::ORIGIN, origin);
    }
    app.clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn header_value(res: &Response, name: header::HeaderName) -> Option<String> {
    res.headers()
        .get(name)
        .map(|v| v.to_str().unwrap().to_string())
}

fn object_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

fn sorted(keys: &[&str]) -> Vec<String> {
    let mut keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    keys.sort();
    keys
}

async fn assert_error(res: Response, status: StatusCode, code: &str) {
    assert_eq!(res.status(), status);
    assert_eq!(header_value(&res, header::CACHE_CONTROL).as_deref(), Some("no-store"));
    assert_eq!(header_value(&res, header::X_CONTENT_TYPE_OPTIONS).as_deref(), Some("nosniff"));
    let body = body_json(res).await;
    assert_eq!(body["code"], code);
    assert!(!body["request_id"].as_str().unwrap().is_empty());
    assert!(body["message"].as_str().is_some());
    assert_eq!(object_keys(&body), sorted(&["request_id", "code", "message"]));
}

// ---- Port (task 03) ---------------------------------------------------------------------

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

// ---- HTTP (task 04) ---------------------------------------------------------------------

#[test]
fn successful_gets_are_publicly_cacheable_and_vary_on_origin_with_or_without_origin() {
    block_on(async {
        let store = tiny_store();
        let app = app(&store);
        for path in MASTER_DATA_PATHS {
            let plain = send(&app, Method::GET, path, None).await;
            assert_eq!(plain.status(), StatusCode::OK, "{path}");
            assert_eq!(header_value(&plain, header::CACHE_CONTROL).as_deref(), Some(PUBLIC_CACHE), "{path}");
            assert_eq!(header_value(&plain, header::VARY).as_deref(), Some("Origin"), "{path}");
            assert_eq!(header_value(&plain, header::ACCESS_CONTROL_ALLOW_ORIGIN), None, "{path}");
            assert_eq!(header_value(&plain, header::X_CONTENT_TYPE_OPTIONS).as_deref(), Some("nosniff"), "{path}");
            assert!(header_value(&plain, header::HeaderName::from_static("x-request-id")).is_some(), "{path}");
            assert_eq!(
                header_value(&plain, header::CONTENT_TYPE).as_deref(),
                Some("application/json"),
                "{path}"
            );

            let allowed = send(&app, Method::GET, path, Some(ALLOWED_ORIGIN)).await;
            assert_eq!(allowed.status(), StatusCode::OK, "{path}");
            assert_eq!(header_value(&allowed, header::CACHE_CONTROL).as_deref(), Some(PUBLIC_CACHE), "{path}");
            assert_eq!(header_value(&allowed, header::VARY).as_deref(), Some("Origin"), "{path}");
            assert_eq!(
                header_value(&allowed, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
                Some(ALLOWED_ORIGIN),
                "{path}"
            );

            let foreign = send(&app, Method::GET, path, Some("https://evil.example.com")).await;
            assert_eq!(foreign.status(), StatusCode::OK, "{path}");
            assert_eq!(header_value(&foreign, header::VARY).as_deref(), Some("Origin"), "{path}");
            assert_eq!(header_value(&foreign, header::ACCESS_CONTROL_ALLOW_ORIGIN), None, "{path}");
        }
    });
}

#[test]
fn scenario_5_d1_unavailable_returns_503_no_store_on_every_endpoint() {
    block_on(async {
        let store = tiny_store();
        store.d1_healthy.store(false, Ordering::SeqCst);
        let app = app(&store);
        for path in MASTER_DATA_PATHS {
            let res = send(&app, Method::GET, path, Some(ALLOWED_ORIGIN)).await;
            let body_has_detail = {
                assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
                assert_eq!(header_value(&res, header::CACHE_CONTROL).as_deref(), Some("no-store"), "{path}");
                let text = body_text(res).await;
                text.contains("simulated") || text.contains("D1")
            };
            assert!(!body_has_detail, "{path} leaked internal detail");
            let res = send(&app, Method::GET, path, None).await;
            assert_error(res, StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE").await;
        }
    });
}

#[test]
fn daily_cards_without_meta_row_is_503_no_store() {
    block_on(async {
        let store = tiny_store();
        store.master_data.lock().unwrap().daily_meta = None;
        let res = send(&app(&store), Method::GET, "/v1/daily-cards", None).await;
        assert_error(res, StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE").await;
    });
}

#[test]
fn scenario_6_write_methods_on_known_paths_return_405_with_allow_and_never_touch_d1() {
    block_on(async {
        let store = tiny_store();
        let before = store.master_data.lock().unwrap().clone();
        let app = app(&store);
        for path in KNOWN_PATHS {
            for method in WRITE_METHODS {
                let res = send(&app, method.clone(), path, None).await;
                assert_eq!(header_value(&res, header::ALLOW).as_deref(), Some("GET,HEAD"), "{method} {path}");
                assert_error(res, StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED").await;
            }
        }
        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 0);
        assert_eq!(*store.master_data.lock().unwrap(), before);
    });
}

#[test]
fn post_to_master_data_path_from_allowed_origin_gets_405_with_cors() {
    block_on(async {
        let store = tiny_store();
        let res = send(&app(&store), Method::POST, "/v1/cards", Some(ALLOWED_ORIGIN)).await;
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            header_value(&res, header::ACCESS_CONTROL_ALLOW_ORIGIN).as_deref(),
            Some(ALLOWED_ORIGIN)
        );
        assert_eq!(header_value(&res, header::ALLOW).as_deref(), Some("GET,HEAD"));
        assert_error(res, StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED").await;
        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn unknown_paths_return_404_for_every_method() {
    block_on(async {
        let store = tiny_store();
        let app = app(&store);
        let paths = ["/", "/v1", "/v1/unknown", "/v1/cards/", "/v1/cards/the-fool", "/V1/CARDS"];
        let methods = [Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE];
        for path in paths {
            for method in methods.clone() {
                let res = send(&app, method.clone(), path, None).await;
                assert_eq!(header_value(&res, header::ALLOW), None, "{method} {path}");
                assert_error(res, StatusCode::NOT_FOUND, "NOT_FOUND").await;
            }
        }
        assert_eq!(store.master_data_reads.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn rate_limited_master_data_request_is_429_no_store() {
    block_on(async {
        let store = tiny_store();
        store.rate_limit_max.store(1, Ordering::SeqCst);
        let app = app(&store);
        let first = send(&app, Method::GET, "/v1/cards", None).await;
        assert_eq!(first.status(), StatusCode::OK);
        let second = send(&app, Method::GET, "/v1/cards", None).await;
        assert_error(second, StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED").await;
    });
}

#[test]
fn health_stays_no_store_and_is_never_publicly_cacheable() {
    block_on(async {
        let store = tiny_store();
        let res = send(&app(&store), Method::GET, "/health", None).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header_value(&res, header::CACHE_CONTROL).as_deref(), Some("no-store"));
    });
}

#[test]
fn tiny_fixture_bodies_follow_the_contract() {
    block_on(async {
        let store = tiny_store();
        let app = app(&store);

        let cards = body_json(send(&app, Method::GET, "/v1/cards", None).await).await;
        assert_eq!(cards.as_array().unwrap().len(), 2);
        assert_eq!(cards[0]["id"], "the-fool");
        assert_eq!(cards[1]["id"], "six-of-wands");
        assert_eq!(cards[1]["art"], Value::Null);
        assert_eq!(
            cards[1]["meanings"],
            json!({"single": "m-single", "complicated": null, "ex_partner": null})
        );
        for card in cards.as_array().unwrap() {
            assert_eq!(object_keys(card), sorted(&["id", "name", "keyword", "art", "meanings", "details"]));
        }

        let daily = body_json(send(&app, Method::GET, "/v1/daily-cards", None).await).await;
        assert_eq!(daily["auto_fallback_to_general_or_spread"], json!(true));
        assert_eq!(
            daily["love_statuses"],
            json!([
                {"key": "single", "source_label": "คนโสด"},
                {"key": "ex_partner", "source_label": "คนรักเก่า"},
                {"key": "complicated", "source_label": "ความสัมพันธ์ซับซ้อน"}
            ])
        );
        assert_eq!(daily["cards"][0]["card_id"], "the-fool");
        assert_eq!(daily["cards"][0]["approved_for_web"], json!(false));
        assert_eq!(daily["cards"][0]["missing_fields"], json!(["content.love_by_status.complicated"]));
        assert_eq!(daily["cards"][0]["content"]["love_by_status"]["complicated"], Value::Null);
        assert_eq!(daily["cards"][1]["missing_fields"], json!([]));
        assert_eq!(daily["cards"][0]["variants"], json!({"summary_theme": "summary", "overview_theme": null}));

        let art = body_json(send(&app, Method::GET, "/v1/card-art", None).await).await;
        assert_eq!(
            art,
            json!({"cards": [{
                "id": "the-fool",
                "art": "/cards/v3/the-fool-e11e1ee168a4.webp",
                "width": 637,
                "height": 1024,
                "renderedWidth": 600,
                "renderedHeight": 965,
                "sha256": "a".repeat(64),
                "bytes": 51234
            }]})
        );

        let config = body_json(send(&app, Method::GET, "/v1/reading-config", None).await).await;
        assert_eq!(
            config["situations"]["love"],
            json!([
                {"key": "single", "label": "คนโสด"},
                {"key": "complicated", "label": "ซับซ้อน"},
                {"key": "ex_partner", "label": "เลิกกัน"}
            ])
        );
        assert_eq!(
            config["situations"]["work"],
            json!([
                {"key": "current_job", "label": "งานปัจจุบัน"},
                {"key": "job_seeking", "label": "มองหางานใหม่"}
            ])
        );
        assert_eq!(config["suggestions"]["love"], json!([{"id": "love-1", "text": "q-love"}]));
        assert_eq!(config["categories"][0]["key"], "love");
        assert_eq!(config["positions"][0], json!({"key": "past", "label": "อดีต"}));
        assert_eq!(
            config["love_statuses"][2],
            json!({"key": "ex_partner", "daily_label": "คนรักเก่า", "reading_label": "เลิกกัน"})
        );
        assert_eq!(config["uncategorized_situations"], json!([]));
        assert_eq!(config["uncategorized_suggestions"], json!([]));
    });
}
```

## 4. Red
```bash
cargo test -p backend-client --test spec_002a_scenarios
```
Expected: compiles; the 3 port tests pass; the 9 HTTP tests FAIL. For example `successful_gets_...` fails with `assertion left == right failed: /v1/cards` (`left: 404`, `right: 200`), and `scenario_6_...` fails because `POST /health` returns axum's bare `405` with no `x-content-type-options`/envelope (`left: None`, `right: Some("nosniff")`). `unknown_paths_return_404_for_every_method` already passes, because the existing fallback covers it. It stays as a regression guard.

## 5. Implementation

### File: `client/src/error.rs` (exact diff)
After the `not_found` constructor, add:
```rust

    pub fn method_not_allowed() -> Self {
        Self {
            status: StatusCode::METHOD_NOT_ALLOWED,
            code: "METHOD_NOT_ALLOWED",
            public_message: "Method not allowed on this path.".to_string(),
            internal_detail: None,
        }
    }
```

### File: `client/src/routes/mod.rs` (full content)
```rust
pub mod health;
pub mod master_data;
```

### File: `client/src/routes/health.rs` (exact diff)
Append at the end of the file:
```rust

/// D12: a known path called with a method it does not serve. axum adds the `Allow` header
/// (for example `GET,HEAD`) from the route's method router after this handler returns.
pub async fn fallback_method_not_allowed(Extension(req_id): Extension<RequestId>) -> Response {
    ClientError::method_not_allowed().into_response_with_request_id(&req_id.0)
}
```

### File: `client/src/routes/master_data.rs` (new, full content)
```rust
//! The four public master-data GETs (SPEC-002a, `docs/api/client/openapi.yaml` v0.2.1).

use crate::master_data::{
    assemble_card_art, assemble_daily_cards, assemble_reading_cards, assemble_reading_config,
};
use crate::middleware::security::RequestId;
use crate::ports::ClientPorts;
use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

/// D05 TTL; Workers Cache (D13) and browsers both follow it.
pub const PUBLIC_CACHE_CONTROL: &str = "public, max-age=600";

/// Serializes `body`; only a `200` gets the public cache headers, so a serialization failure
/// (axum answers `500`) stays `no-store` through the security middleware.
fn public_json<T: Serialize>(body: &T) -> Response {
    let mut res = Json(body).into_response();
    if res.status() == StatusCode::OK {
        let headers = res.headers_mut();
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static(PUBLIC_CACHE_CONTROL),
        );
        // Workers Cache keys variants on `Vary`. It is set even when the request has no
        // `Origin`, so a stored no-Origin variant is never served to a browser request.
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    }
    res
}

pub async fn get_cards(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.reading_cards().await {
        Ok(rows) => public_json(&assemble_reading_cards(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_daily_cards(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.daily_cards().await {
        Ok(rows) => public_json(&assemble_daily_cards(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_card_art(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.card_art().await {
        Ok(rows) => public_json(&assemble_card_art(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}

pub async fn get_reading_config(
    State(ports): State<ClientPorts>,
    Extension(req_id): Extension<RequestId>,
) -> Response {
    match ports.master_data.reading_config().await {
        Ok(rows) => public_json(&assemble_reading_config(rows)),
        Err(err) => err.into_response_with_request_id(&req_id.0),
    }
}
```

### File: `client/src/lib.rs` (exact diffs)
1. Replace:
```rust
use crate::routes::health::{fallback_not_found, get_health};
```
with:
```rust
use crate::routes::health::{fallback_method_not_allowed, fallback_not_found, get_health};
use crate::routes::master_data::{get_card_art, get_cards, get_daily_cards, get_reading_config};
```
2. Replace the body of `build_router`:
```rust
pub fn build_router(ports: ClientPorts) -> Router {
    Router::new()
        .route("/health", get(get_health))
        .fallback(fallback_not_found)
        .layer(from_fn_with_state(
            ports.clone(),
            security_pipeline_middleware,
        ))
        .with_state(ports)
}
```
with:
```rust
pub fn build_router(ports: ClientPorts) -> Router {
    Router::new()
        .route("/health", get(get_health))
        .route("/v1/cards", get(get_cards))
        .route("/v1/daily-cards", get(get_daily_cards))
        .route("/v1/card-art", get(get_card_art))
        .route("/v1/reading-config", get(get_reading_config))
        // Must come after every `.route(...)`: it is applied to the routes registered so far.
        .method_not_allowed_fallback(fallback_method_not_allowed)
        .fallback(fallback_not_found)
        .layer(from_fn_with_state(
            ports.clone(),
            security_pipeline_middleware,
        ))
        .with_state(ports)
}
```

> **Execution note 2026-11-26:** red showed 7 failing HTTP tests, not 9. `unknown_paths_return_404_for_every_method` and `health_stays_no_store_...` already pass before the routes exist (regression guards), so 7 is the consistent count. `scenario_6_...` failed on the empty body of axum's bare `405` (no envelope), which is the expected missing-envelope reason.

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
Expected: `spec_002a_scenarios` 12 passed; `spec_001_scenarios` 5 passed unchanged; the lib unit tests pass; fmt, both clippy runs and `make check` are clean.
