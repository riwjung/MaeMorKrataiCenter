mod seed_support;
mod support;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use backend_client::adapters::memory::MemoryClientStore;
use backend_client::build_router;
use backend_client::config::Environment;
use seed_support::{
    apply_a1, assert_a1_reading_config, assert_art_consistent, assert_card_art_matches_seed,
    assert_cards_match_seed, assert_clean, assert_daily_matches_seed,
    assert_reading_config_matches_seed, seed_fixture, SEED_FILES,
};
use serde_json::Value;
use std::sync::Arc;
use support::{block_on, body_text};
use tower::ServiceExt;

fn seeded_store() -> Arc<MemoryClientStore> {
    let store = MemoryClientStore::new();
    *store.master_data.lock().unwrap() = seed_fixture();
    store
}

fn app(store: &Arc<MemoryClientStore>) -> Router {
    build_router(store.into_ports(Environment::Staging))
}

/// Returns (status, Cache-Control, body text).
async fn get(app: &Router, path: &str) -> (StatusCode, Option<String>, String) {
    let res = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let cache = res
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|v| v.to_str().unwrap().to_string());
    (status, cache, body_text(res).await)
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn scenario_1_cards_equal_seed_in_file_order_with_public_cache() {
    block_on(async {
        let (status, cache, body) = get(&app(&seeded_store()), "/v1/cards").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("public, max-age=600"));
        assert_cards_match_seed(&parse(&body));
    });
}

#[test]
fn scenario_2_daily_cards_equal_seed_with_computed_missing_fields_and_no_provenance() {
    block_on(async {
        let (status, cache, body) = get(&app(&seeded_store()), "/v1/daily-cards").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("public, max-age=600"));
        assert_daily_matches_seed(&parse(&body));
        assert_clean("/v1/daily-cards", &body);
    });
}

#[test]
fn scenario_3_card_art_equals_seed_and_cards_without_art_are_null() {
    block_on(async {
        let app = app(&seeded_store());
        let (status, cache, art) = get(&app, "/v1/card-art").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("public, max-age=600"));
        let art = parse(&art);
        assert_card_art_matches_seed(&art);
        let (_, _, cards) = get(&app, "/v1/cards").await;
        assert_art_consistent(&parse(&cards), &art);
    });
}

#[test]
fn scenario_4_reading_config_equals_seed_and_daily_keeps_daily_order() {
    block_on(async {
        let app = app(&seeded_store());
        let (status, cache, config) = get(&app, "/v1/reading-config").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cache.as_deref(), Some("public, max-age=600"));
        assert_reading_config_matches_seed(&parse(&config));
        let (_, _, daily) = get(&app, "/v1/daily-cards").await;
        let daily = parse(&daily);
        let keys: Vec<&str> = daily["love_statuses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["key"].as_str().unwrap())
            .collect();
        assert_eq!(
            keys,
            [
                "single",
                "crush",
                "talking",
                "partnered",
                "ex_partner",
                "complicated"
            ]
        );
    });
}

#[test]
fn scenario_a1_deleted_categories_leave_uncategorized_items_in_order() {
    block_on(async {
        let store = seeded_store();
        apply_a1(&mut store.master_data.lock().unwrap());
        let (status, _, config) = get(&app(&store), "/v1/reading-config").await;
        assert_eq!(status, StatusCode::OK);
        assert_a1_reading_config(&parse(&config));
    });
}

#[test]
fn seed_files_and_every_response_body_are_free_of_provenance() {
    block_on(async {
        for (name, text) in SEED_FILES {
            assert_clean(name, text);
        }
        let app = app(&seeded_store());
        for path in [
            "/v1/cards",
            "/v1/daily-cards",
            "/v1/card-art",
            "/v1/reading-config",
        ] {
            let (status, _, body) = get(&app, path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert_clean(path, &body);
        }
    });
}
