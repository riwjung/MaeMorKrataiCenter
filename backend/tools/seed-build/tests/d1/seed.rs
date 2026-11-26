//! The seed on real D1: every seeded row equals `seed/*.json` byte for byte, and scenario 27.

use d1_local::{backend_dir, D1Local, Row};
use seed_build::{build_sql, SeedSources};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;

const EXTRA_SUGGESTION_ID: &str = "7b0f3c1e-2a4d-4e8b-9c6f-1d2e3f4a5b6c";

/// A migrated database loaded with `seed/` the way `make seed-local` loads it.
fn seeded(name: &str) -> D1Local {
    let db = D1Local::migrated(name);
    load_seed(&db);
    db
}

/// Builds the seed SQL from `seed/` with the seed-build library and runs it with `--file`.
fn load_seed(db: &D1Local) {
    let sources =
        SeedSources::read_dir(&backend_dir().join("seed")).expect("seed files are readable");
    let sql = build_sql(&sources).expect("seed builds");
    let path = db.state_dir().join("seed.sql");
    fs::write(&path, sql).expect("write seed.sql");
    db.exec_file(&path)
        .unwrap_or_else(|e| panic!("seed load failed: {e}"));
}

fn seed_json(file: &str) -> Value {
    let text = fs::read_to_string(backend_dir().join("seed").join(file)).expect("read seed file");
    serde_json::from_str(&text).expect("seed file is JSON")
}

fn rows_value(rows: &[Row]) -> Value {
    Value::Array(rows.iter().cloned().map(Value::Object).collect())
}

fn text(row: &Row, column: &str) -> String {
    row[column]
        .as_str()
        .unwrap_or_else(|| panic!("{column} is not text in {row:?}"))
        .to_string()
}

fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .expect("array")
        .iter()
        .map(|item| item["key"].as_str().expect("key").to_string())
        .collect()
}

/// (card_id, love_status) to text, for card_meanings, card_details and daily_love_texts.
fn by_card_and_status(rows: &[Row]) -> HashMap<(String, String), Value> {
    rows.iter()
        .map(|row| {
            (
                (text(row, "card_id"), text(row, "love_status")),
                row["text"].clone(),
            )
        })
        .collect()
}

fn as_flag(value: &Value) -> i64 {
    i64::from(value.as_bool().expect("boolean"))
}

/// Asserts that every seeded row equals the seed files, with `extra_suggestions` non-seed rows.
fn assert_matches_seed(db: &D1Local, extra_suggestions: i64) {
    let selects = [
        "SELECT (SELECT COUNT(*) FROM love_statuses) AS love_statuses, (SELECT COUNT(*) FROM categories) AS categories, (SELECT COUNT(*) FROM situations) AS situations, (SELECT COUNT(*) FROM suggestions) AS suggestions, (SELECT COUNT(*) FROM positions) AS positions, (SELECT COUNT(*) FROM cards) AS cards, (SELECT COUNT(*) FROM card_meanings) AS card_meanings, (SELECT COUNT(*) FROM card_details) AS card_details, (SELECT COUNT(*) FROM card_art) AS card_art, (SELECT COUNT(*) FROM daily_meta) AS daily_meta, (SELECT COUNT(*) FROM daily_cards) AS daily_cards, (SELECT COUNT(*) FROM daily_love_texts) AS daily_love_texts",
        "SELECT id, name, keyword, art FROM cards ORDER BY sort_order, id",
        "SELECT card_id, love_status, text FROM card_meanings",
        "SELECT card_id, love_status, text FROM card_details",
        "SELECT card_id, love_status, text FROM daily_love_texts",
        "SELECT schema_version, version, updated, status, scope, auto_fallback_to_general_or_spread FROM daily_meta",
        "SELECT card_id, name, arcana, reading_mode, time_horizon, orientation, content_status, approved_for_web, theme, overview, work, money, rabbit_message, colors, affirmation, summary_theme, overview_theme FROM daily_cards ORDER BY sort_order, card_id",
        "SELECT a.card_id AS id, a.art, a.width, a.height, a.rendered_width AS renderedWidth, a.rendered_height AS renderedHeight, a.sha256, a.bytes FROM card_art a JOIN cards c ON c.id = a.card_id ORDER BY c.sort_order, c.id",
        "SELECT key, label, icon, hint FROM categories ORDER BY sort_order, key",
        "SELECT key, daily_label, reading_label FROM love_statuses ORDER BY reading_sort_order, key",
        "SELECT key, reading_label AS label FROM love_statuses ORDER BY reading_sort_order, key",
        "SELECT key, daily_label AS source_label FROM love_statuses ORDER BY daily_sort_order, key",
        "SELECT key, label FROM positions ORDER BY sort_order, key",
        "SELECT category, key, label FROM situations ORDER BY sort_order, key",
        "SELECT category, id, text FROM suggestions ORDER BY sort_order, id",
    ];
    let sets = db.exec(&selects.join("; ")).expect("seed SELECTs succeed");
    let [counts, cards_rows, meanings, details, daily_texts, meta, daily_rows, art_rows, categories, statuses, love_situations, daily_statuses, positions, situations, suggestions]: [Vec<Row>; 15] =
        sets.try_into().expect("one result set per SELECT");

    let cards = seed_json("cards.json");
    let daily = seed_json("daily.json");
    let card_art = seed_json("card-art.json");
    let config = seed_json("reading-config.json");

    // Row counts (SPEC-002a scenarios 1 to 4).
    let expected_counts = [
        ("love_statuses", 6),
        ("categories", 4),
        ("situations", 9),
        ("suggestions", 12 + extra_suggestions),
        ("positions", 3),
        ("cards", 78),
        ("card_meanings", 468),
        ("card_details", 468),
        ("card_art", 74),
        ("daily_meta", 1),
        ("daily_cards", 78),
        ("daily_love_texts", 468),
    ];
    for (table, count) in expected_counts {
        assert_eq!(counts[0][table], count, "row count of {table}");
    }

    // Cards and their love-status texts, in reading-config order.
    let card_list = cards.as_array().expect("cards.json is an array");
    assert_eq!(cards_rows.len(), card_list.len());
    for (row, card) in cards_rows.iter().zip(card_list) {
        for column in ["id", "name", "keyword", "art"] {
            assert_eq!(
                row[column], card[column],
                "cards.{column} of {}",
                card["id"]
            );
        }
    }
    let reading_keys = keys(&config["love_statuses"]);
    let meanings = by_card_and_status(&meanings);
    let details = by_card_and_status(&details);
    for card in card_list {
        let id = card["id"].as_str().expect("id").to_string();
        for key in &reading_keys {
            let k = (id.clone(), key.clone());
            assert_eq!(
                meanings[&k],
                card["meanings"][key.as_str()],
                "meanings {id}/{key}"
            );
            assert_eq!(
                details[&k],
                card["details"][key.as_str()],
                "details {id}/{key}"
            );
        }
    }

    // Daily metadata, entries and love texts.
    for column in ["schema_version", "version", "updated", "status", "scope"] {
        assert_eq!(meta[0][column], daily[column], "daily_meta.{column}");
    }
    assert_eq!(
        meta[0]["auto_fallback_to_general_or_spread"],
        as_flag(&daily["auto_fallback_to_general_or_spread"])
    );
    let entries = daily["cards"].as_array().expect("daily cards");
    assert_eq!(daily_rows.len(), entries.len());
    for (row, entry) in daily_rows.iter().zip(entries) {
        for column in [
            "card_id",
            "name",
            "arcana",
            "reading_mode",
            "time_horizon",
            "orientation",
            "content_status",
        ] {
            assert_eq!(
                row[column], entry[column],
                "daily_cards.{column} of {}",
                entry["card_id"]
            );
        }
        assert_eq!(row["approved_for_web"], as_flag(&entry["approved_for_web"]));
        for column in [
            "theme",
            "overview",
            "work",
            "money",
            "rabbit_message",
            "colors",
            "affirmation",
        ] {
            assert_eq!(
                row[column], entry["content"][column],
                "content.{column} of {}",
                entry["card_id"]
            );
        }
        for column in ["summary_theme", "overview_theme"] {
            assert_eq!(
                row[column], entry["variants"][column],
                "variants.{column} of {}",
                entry["card_id"]
            );
        }
    }
    let daily_keys = keys(&daily["love_statuses"]);
    let daily_texts = by_card_and_status(&daily_texts);
    for entry in entries {
        let id = entry["card_id"].as_str().expect("card_id").to_string();
        for key in &daily_keys {
            assert_eq!(
                daily_texts[&(id.clone(), key.clone())],
                entry["content"]["love_by_status"][key.as_str()],
                "love_by_status {id}/{key}"
            );
        }
    }
    assert_eq!(daily_texts.values().filter(|t| t.is_null()).count(), 22);

    // Card art (manifest order equals card order).
    assert_eq!(rows_value(&art_rows), card_art["cards"]);

    // Reading config.
    assert_eq!(rows_value(&categories), config["categories"]);
    assert_eq!(rows_value(&statuses), config["love_statuses"]);
    assert_eq!(rows_value(&love_situations), config["situations"]["love"]);
    assert_eq!(rows_value(&daily_statuses), daily["love_statuses"]);
    assert_eq!(rows_value(&positions), config["positions"]);
    let situation_map = config["situations"].as_object().expect("situations");
    for (category, list) in situation_map.iter().filter(|(c, _)| c.as_str() != "love") {
        let stored: Vec<Value> = situations
            .iter()
            .filter(|row| row["category"] == category.as_str())
            .map(|row| serde_json::json!({"key": row["key"], "label": row["label"]}))
            .collect();
        assert_eq!(&Value::Array(stored), list, "situations.{category}");
    }
    let suggestion_map = config["suggestions"].as_object().expect("suggestions");
    let seeded_ids: HashSet<&str> = suggestion_map
        .values()
        .flat_map(|list| list.as_array().expect("array"))
        .map(|s| s["id"].as_str().expect("id"))
        .collect();
    for (category, list) in suggestion_map {
        let stored: Vec<Value> = suggestions
            .iter()
            .filter(|row| row["category"] == category.as_str())
            .filter(|row| seeded_ids.contains(text(row, "id").as_str()))
            .map(|row| serde_json::json!({"id": row["id"], "text": row["text"]}))
            .collect();
        assert_eq!(&Value::Array(stored), list, "suggestions.{category}");
    }
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn the_seed_loads_every_row_unchanged() {
    let db = seeded("seed_load");
    assert_matches_seed(&db, 0);
    assert_eq!(db.count("audit_events"), 0);
}

/// SPEC-002a scenario 27.
#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn scenario_27_reseed_restores_seeded_rows_and_keeps_other_rows() {
    let db = seeded("seed_reseed");
    db.exec(&format!(
        "UPDATE cards SET name = 'changed', art = NULL WHERE id = 'the-fool'; \
         UPDATE card_meanings SET text = NULL WHERE card_id = 'the-fool' AND love_status = 'single'; \
         UPDATE daily_love_texts SET text = 'filled by admin' WHERE card_id = 'the-fool' AND love_status = 'complicated'; \
         UPDATE daily_meta SET version = 'changed', auto_fallback_to_general_or_spread = 1 WHERE id = 1; \
         UPDATE categories SET sort_order = 999 WHERE key = 'love'; \
         UPDATE situations SET category = NULL WHERE key = 'current_job'; \
         INSERT INTO suggestions (id, category, text, sort_order) VALUES ('{EXTRA_SUGGESTION_ID}', 'work', 'admin question', 40); \
         INSERT INTO audit_events (id, actor_user_id, event_type, target_id, metadata_json, created_at) \
         VALUES ('audit-before-reseed', NULL, 'master_data.update', 'the-fool', '{{}}', 1764000000);"
    ))
    .expect("direct edits succeed");
    assert_eq!(
        db.query("SELECT name FROM cards WHERE id = 'the-fool'")[0]["name"],
        "changed"
    );

    load_seed(&db);

    assert_matches_seed(&db, 1);
    let extra = db.query(&format!(
        "SELECT category, text, sort_order FROM suggestions WHERE id = '{EXTRA_SUGGESTION_ID}'"
    ));
    assert_eq!(
        extra.len(),
        1,
        "the non-seed suggestion survives the re-seed"
    );
    assert_eq!(extra[0]["category"], "work");
    assert_eq!(extra[0]["text"], "admin question");
    let audit = db.query("SELECT id FROM audit_events");
    assert_eq!(
        audit.len(),
        1,
        "the seed adds no audit_events rows and deletes none"
    );
    assert_eq!(audit[0]["id"], "audit-before-reseed");
}
