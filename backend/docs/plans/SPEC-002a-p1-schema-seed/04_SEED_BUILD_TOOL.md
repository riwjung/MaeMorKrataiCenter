---
model: sonnet
effort: medium
reason: "Complete code for a ~550-line std + serde tool and its tests is supplied; the executor only applies it and runs red/green"
---

# Task 04: Seed build tool (`tools/seed-build`), seed hygiene tests, scenario 27 on D1

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Depends on task 01 (`seed/*.json`), task 02 (crates `seed-build` and `d1-local`, D1 harness) and task 03 (migration 0002).

All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## What the tool does
- **Input.** The four seed files have the exact shape of the four public `client` responses: `cards.json` is `GET /v1/cards`, `daily.json` is `GET /v1/daily-cards`, `card-art.json` is `GET /v1/card-art`, and `reading-config.json` is `GET /v1/reading-config` (openapi v0.2.1). p2 compares its responses with these files.
- **Derived fields.** `missing_fields` and `situations.love` appear in the files but are derived. The tool checks that they equal the derived values, then does not store them.
- **Validation.** It fails on any dropped token (SPEC-002 section 4.2), on unknown fields (`#[serde(deny_unknown_fields)]`), on control characters other than `\n`, and on any cross-file inconsistency.
- **Output.** One `INSERT INTO <t> (...) VALUES (...) ON CONFLICT(<pk>) DO UPDATE SET <every non-key column> = excluded.<column>;` per row, in `TABLE_ORDER`. There is no `DELETE`, no `INSERT OR REPLACE` and no `audit_events`. 1,669 statements.
- **Sort orders.** `(index + 1) * 10` in file order, which leaves gaps for SPEC-002b inserts. `daily_sort_order` follows `daily.json` `love_statuses`; `reading_sort_order` follows `reading-config.json` `love_statuses`.
- **CLI.** `seed-build --seed-dir=<dir> --out=<file>`. On any error it removes a stale output file and exits 1, so `make seed-local` stops before wrangler runs.

## 1. Action Items
- [ ] Tests first: `tests/seed_files.rs`, `tests/seed_validation.rs`, `tests/d1/seed.rs` (+ `mod seed;`). The helpers that load the seed into a `D1Local` live in `tests/d1/seed.rs`, not in the `d1-local` crate (which must not depend on `seed-build`).
- [ ] Red.
- [ ] Implementation: replace `src/lib.rs`; add `src/model.rs`, `src/validate.rs`, `src/sql.rs`, `src/main.rs`.
- [ ] Green, `make test-d1`, standards check.

## 2. Harness

No change to the `d1-local` crate: `D1Local::exec_file` and `D1Local::state_dir` (task 02) are enough. The two seed helpers are private functions at the top of `tests/d1/seed.rs` (below).

### File: `tools/seed-build/tests/d1/main.rs`, exact diff
```diff
 mod harness;
 mod schema;
+mod seed;
```

## 3. Tests (written first)

### File: `tools/seed-build/tests/seed_files.rs` (new)
```rust
//! The committed `seed/` files: hygiene (no dropped field, no ChatGPT link) and the rendered SQL.

use seed_build::{build_sql, build_statements, SeedSources, FORBIDDEN_TOKENS, SEED_FILES, TABLE_ORDER};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn seed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../seed")
}

fn statements() -> Vec<String> {
    let sources = SeedSources::read_dir(&seed_dir()).expect("seed files are readable");
    build_statements(&sources).expect("the committed seed builds")
}

fn table_of(statement: &str) -> &str {
    statement
        .strip_prefix("INSERT INTO ")
        .and_then(|rest| rest.split(' ').next())
        .unwrap_or_else(|| panic!("not an INSERT: {statement}"))
}

#[test]
fn seed_dir_holds_exactly_the_four_seed_files() {
    let mut files = Vec::new();
    for entry in fs::read_dir(seed_dir()).expect("read seed/") {
        let entry = entry.expect("directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || (name == "build" && entry.path().is_dir()) {
            continue;
        }
        files.push(name);
    }
    files.sort();
    let mut expected: Vec<String> = SEED_FILES.iter().map(|f| (*f).to_string()).collect();
    expected.sort();
    assert_eq!(files, expected);
}

#[test]
fn seed_files_contain_no_dropped_field_or_chatgpt_link() {
    for file in SEED_FILES {
        let text = fs::read_to_string(seed_dir().join(file)).expect("read seed file");
        for needle in ["chatgpt.com", "source_url", "raw_file"] {
            assert!(!text.contains(needle), "seed/{file} contains {needle}");
        }
        for token in FORBIDDEN_TOKENS {
            assert!(!text.contains(token), "seed/{file} contains {token}");
        }
    }
}

#[test]
fn committed_seed_renders_the_expected_rows_per_table() {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let statements = statements();
    for statement in &statements {
        *counts.entry(table_of(statement)).or_default() += 1;
    }
    let expected: BTreeMap<&str, usize> = [
        ("love_statuses", 6),
        ("categories", 4),
        ("situations", 9),
        ("suggestions", 12),
        ("positions", 3),
        ("cards", 78),
        ("card_meanings", 468),
        ("card_details", 468),
        ("card_art", 74),
        ("daily_meta", 1),
        ("daily_cards", 78),
        ("daily_love_texts", 468),
    ]
    .into_iter()
    .collect();
    assert_eq!(counts, expected);
    assert_eq!(statements.len(), 1669);
}

#[test]
fn every_statement_is_a_single_row_upsert() {
    for statement in statements() {
        assert!(statement.starts_with("INSERT INTO "), "{statement}");
        assert_eq!(statement.matches(" VALUES (").count(), 1, "{statement}");
        assert!(statement.contains(") ON CONFLICT("), "{statement}");
        assert!(statement.contains(" DO UPDATE SET "), "{statement}");
        assert!(statement.ends_with(';'), "{statement}");
        assert!(!statement.contains("INSERT OR REPLACE"), "{statement}");
        assert!(!statement.contains("REPLACE INTO"), "{statement}");
    }
    let sources = SeedSources::read_dir(&seed_dir()).expect("seed files are readable");
    let sql = build_sql(&sources).expect("seed builds");
    assert!(!sql.contains("audit_events"), "the seed must not write audit_events");
    assert!(
        !sql.lines().any(|line| line.trim_start().to_ascii_uppercase().starts_with("DELETE")),
        "the seed must not delete"
    );
}

#[test]
fn parents_load_before_children() {
    let order: Vec<usize> = statements()
        .iter()
        .map(|statement| {
            let table = table_of(statement);
            TABLE_ORDER
                .iter()
                .position(|t| *t == table)
                .unwrap_or_else(|| panic!("unknown table {table}"))
        })
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] <= pair[1]), "tables out of load order");
    for (index, table) in TABLE_ORDER.iter().enumerate() {
        assert!(order.contains(&index), "no rows for {table}");
    }
}

#[test]
fn thai_text_is_copied_into_the_sql_unchanged() {
    let cards: Value =
        serde_json::from_str(&fs::read_to_string(seed_dir().join("cards.json")).expect("read"))
            .expect("cards.json is JSON");
    let card = &cards[0];
    let id = card["id"].as_str().expect("id");
    let text = card["meanings"]["single"].as_str().expect("text");
    let expected = format!(
        "INSERT INTO card_meanings (card_id, love_status, text) VALUES ('{id}', 'single', '{}') \
         ON CONFLICT(card_id, love_status) DO UPDATE SET text = excluded.text;",
        text.replace('\'', "''")
    );
    assert!(statements().contains(&expected), "missing: {expected}");
}

#[test]
fn the_build_is_deterministic() {
    let sources = SeedSources::read_dir(&seed_dir()).expect("seed files are readable");
    assert_eq!(build_sql(&sources), build_sql(&sources));
}
```

### File: `tools/seed-build/tests/seed_validation.rs` (new)
```rust
//! Each check of the seed build, triggered by a one-field edit of the committed seed.

use seed_build::{build_sql, SeedSources};
use serde_json::{json, Value};
use std::path::Path;

fn committed() -> SeedSources {
    SeedSources::read_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../seed"))
        .expect("seed files are readable")
}

fn edit(text: &str, change: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(text).expect("seed file is JSON");
    change(&mut value);
    serde_json::to_string_pretty(&value).expect("serialize")
}

fn assert_fails(sources: &SeedSources, needle: &str) {
    let err = build_sql(sources).expect_err("the build must fail");
    assert!(err.contains(needle), "expected `{needle}` in: {err}");
}

#[test]
fn the_committed_seed_builds() {
    assert!(build_sql(&committed()).is_ok());
}

#[test]
fn a_dropped_reading_card_field_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["source_url"] = json!("https://example.invalid/x"));
    assert_fails(&s, "source_url");
}

#[test]
fn a_dropped_daily_source_object_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["source"] = json!({"raw_file": "raw/x.json"}));
    assert_fails(&s, "source");
}

#[test]
fn a_dropped_manifest_field_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| v["cards"][0]["sourceFile"] = json!("the-fool.jpg"));
    assert_fails(&s, "sourceFile");
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| v["capturedAt"] = json!("2026-10-30T04:43:42.890Z"));
    assert_fails(&s, "capturedAt");
}

#[test]
fn a_chatgpt_link_inside_text_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["meanings"]["single"] = json!("see https://chatgpt.com/share/x"));
    assert_fails(&s, "chatgpt.com");
}

#[test]
fn an_unknown_field_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["notes"] = json!("x"));
    assert_fails(&s, "unknown field `notes`");
}

#[test]
fn a_control_character_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[0]["keyword"] = json!("a\u{0007}b"));
    assert_fails(&s, "control character U+0007");
}

#[test]
fn a_missing_love_status_key_in_meanings_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| {
        v[0]["meanings"].as_object_mut().expect("object").remove("single");
    });
    assert_fails(&s, "meanings keys");
}

#[test]
fn an_unknown_love_status_in_daily_texts_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["content"]["love_by_status"]["widowed"] = json!("x"));
    assert_fails(&s, "content.love_by_status keys");
}

#[test]
fn missing_fields_that_disagree_with_null_texts_fail() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["missing_fields"] = json!([]));
    assert_fails(&s, "missing_fields");
}

#[test]
fn an_art_entry_that_differs_from_its_card_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| v["cards"][0]["art"] = json!("/cards/v3/other.webp"));
    assert_fails(&s, "art differs");
}

#[test]
fn an_art_entry_for_a_card_without_art_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| {
        let mut entry = v["cards"][0].clone();
        entry["id"] = json!("six-of-wands");
        v["cards"].as_array_mut().expect("array").push(entry);
    });
    assert_fails(&s, "art differs");
}

#[test]
fn an_art_entry_without_a_card_fails() {
    let mut s = committed();
    s.card_art = edit(&s.card_art, |v| v["cards"][0]["id"] = json!("x-card"));
    assert_fails(&s, "has no card");
}

#[test]
fn a_daily_entry_without_a_card_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["cards"][0]["card_id"] = json!("x-card"));
    assert_fails(&s, "has no card");
}

#[test]
fn a_duplicate_card_id_fails() {
    let mut s = committed();
    s.cards = edit(&s.cards, |v| v[1]["id"] = v[0]["id"].clone());
    assert_fails(&s, "duplicate card id");
}

#[test]
fn love_situations_that_differ_from_love_statuses_fail() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["situations"]["love"].as_array_mut().expect("array").swap(0, 1);
    });
    assert_fails(&s, "situations.love");
}

#[test]
fn uncategorized_items_in_the_seed_fail() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["uncategorized_suggestions"] = json!([{"id": "x-1", "text": "x"}]);
    });
    assert_fails(&s, "must be empty");
}

#[test]
fn a_daily_label_that_differs_between_files_fails() {
    let mut s = committed();
    s.daily = edit(&s.daily, |v| v["love_statuses"][0]["source_label"] = json!("x"));
    assert_fails(&s, "source_label differs");
}

#[test]
fn a_situation_under_an_unknown_category_fails() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["situations"]["nope"] = json!([{"key": "k", "label": "l"}]);
    });
    assert_fails(&s, "unknown category `nope`");
}

#[test]
fn a_duplicate_suggestion_id_fails() {
    let mut s = committed();
    s.reading_config = edit(&s.reading_config, |v| {
        v["suggestions"]["work"][0]["id"] = json!("love-1");
    });
    assert_fails(&s, "duplicate suggestion id `love-1`");
}
```

### File: `tools/seed-build/tests/d1/seed.rs` (new)
```rust
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
        .map(|row| ((text(row, "card_id"), text(row, "love_status")), row["text"].clone()))
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
            assert_eq!(row[column], card[column], "cards.{column} of {}", card["id"]);
        }
    }
    let reading_keys = keys(&config["love_statuses"]);
    let meanings = by_card_and_status(&meanings);
    let details = by_card_and_status(&details);
    for card in card_list {
        let id = card["id"].as_str().expect("id").to_string();
        for key in &reading_keys {
            let k = (id.clone(), key.clone());
            assert_eq!(meanings[&k], card["meanings"][key.as_str()], "meanings {id}/{key}");
            assert_eq!(details[&k], card["details"][key.as_str()], "details {id}/{key}");
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
        for column in ["card_id", "name", "arcana", "reading_mode", "time_horizon", "orientation", "content_status"] {
            assert_eq!(row[column], entry[column], "daily_cards.{column} of {}", entry["card_id"]);
        }
        assert_eq!(row["approved_for_web"], as_flag(&entry["approved_for_web"]));
        for column in ["theme", "overview", "work", "money", "rabbit_message", "colors", "affirmation"] {
            assert_eq!(row[column], entry["content"][column], "content.{column} of {}", entry["card_id"]);
        }
        for column in ["summary_theme", "overview_theme"] {
            assert_eq!(row[column], entry["variants"][column], "variants.{column} of {}", entry["card_id"]);
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
    assert_eq!(db.query("SELECT name FROM cards WHERE id = 'the-fool'")[0]["name"], "changed");

    load_seed(&db);

    assert_matches_seed(&db, 1);
    let extra = db.query(&format!(
        "SELECT category, text, sort_order FROM suggestions WHERE id = '{EXTRA_SUGGESTION_ID}'"
    ));
    assert_eq!(extra.len(), 1, "the non-seed suggestion survives the re-seed");
    assert_eq!(extra[0]["category"], "work");
    assert_eq!(extra[0]["text"], "admin question");
    let audit = db.query("SELECT id FROM audit_events");
    assert_eq!(audit.len(), 1, "the seed adds no audit_events rows and deletes none");
    assert_eq!(audit[0]["id"], "audit-before-reseed");
}
```

## 4. Red
```bash
cargo test -p seed-build --no-run
```
Expected: FAIL at compile time. `tests/seed_files.rs` and `tests/seed_validation.rs` fail with `error[E0432]: unresolved imports seed_build::build_sql, seed_build::build_statements, seed_build::SeedSources, seed_build::FORBIDDEN_TOKENS, seed_build::SEED_FILES`, and `tests/d1/seed.rs` fails with the same error for `build_sql` and `SeedSources`.

## 5. Implementation

### File: `tools/seed-build/src/lib.rs` (replace the whole file)
```rust
//! Builds the master-data seed SQL from `seed/*.json` (SPEC-002a, D07).
//!
//! The four seed files have the shape of the four public `client` responses. `build_sql` rejects
//! dropped provenance fields, unknown fields and control characters, checks cross-file references,
//! and renders one `INSERT ... ON CONFLICT(<key>) DO UPDATE SET` statement per row, parents before
//! children. It never emits `DELETE`, `INSERT OR REPLACE` or a write to `audit_events`.

mod model;
mod sql;
mod validate;

use serde_json::Value;
use std::fs;
use std::path::Path;

/// Master-data tables in load order: every parent table comes before its children.
pub const TABLE_ORDER: [&str; 12] = [
    "love_statuses",
    "categories",
    "situations",
    "suggestions",
    "positions",
    "cards",
    "card_meanings",
    "card_details",
    "card_art",
    "daily_meta",
    "daily_cards",
    "daily_love_texts",
];

/// Seed file names inside the seed directory.
pub const SEED_FILES: [&str; 4] = [
    "reading-config.json",
    "cards.json",
    "card-art.json",
    "daily.json",
];

/// Text that must never appear in a seed file: ChatGPT links and the dropped fields of
/// SPEC-002 section 4.2, written as JSON keys.
pub const FORBIDDEN_TOKENS: [&str; 14] = [
    "chatgpt.com",
    "\"source_url\"",
    "\"source_message\"",
    "\"source_line\"",
    "\"review_status\"",
    "\"details_status\"",
    "\"source\"",
    "\"raw_file\"",
    "\"sourceFile\"",
    "\"sourceSha256\"",
    "\"capturedAt\"",
    "\"selection\"",
    "\"duplicates\"",
    "\"ignored\"",
];

/// Raw text of the four seed files.
#[derive(Debug, Clone)]
pub struct SeedSources {
    pub reading_config: String,
    pub cards: String,
    pub card_art: String,
    pub daily: String,
}

impl SeedSources {
    /// Reads the four seed files from `dir`.
    pub fn read_dir(dir: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            let path = dir.join(name);
            fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))
        };
        Ok(Self {
            reading_config: read(SEED_FILES[0])?,
            cards: read(SEED_FILES[1])?,
            card_art: read(SEED_FILES[2])?,
            daily: read(SEED_FILES[3])?,
        })
    }

    fn files(&self) -> [(&'static str, &str); 4] {
        [
            (SEED_FILES[0], &self.reading_config),
            (SEED_FILES[1], &self.cards),
            (SEED_FILES[2], &self.card_art),
            (SEED_FILES[3], &self.daily),
        ]
    }
}

/// Validates the seed and returns one SQL statement per row, in load order.
pub fn build_statements(sources: &SeedSources) -> Result<Vec<String>, String> {
    for (name, text) in sources.files() {
        check_forbidden_tokens(name, text)?;
        check_control_characters(name, text)?;
    }
    let seed = model::Seed::parse(sources)?;
    validate::validate(&seed)?;
    Ok(sql::render(&seed))
}

/// The complete `seed.sql` text: a comment header, then one statement per line group.
pub fn render_sql(statements: &[String]) -> String {
    let mut out = String::from(sql::HEADER);
    for statement in statements {
        out.push_str(statement);
        out.push('\n');
    }
    out
}

/// Validates the seed and returns the complete `seed.sql` text.
pub fn build_sql(sources: &SeedSources) -> Result<String, String> {
    Ok(render_sql(&build_statements(sources)?))
}

fn check_forbidden_tokens(name: &str, text: &str) -> Result<(), String> {
    match FORBIDDEN_TOKENS.iter().find(|token| text.contains(**token)) {
        Some(token) => Err(format!(
            "seed/{name} contains the dropped field or provenance {token} (SPEC-002 section 4.2)"
        )),
        None => Ok(()),
    }
}

/// Rejects control characters other than newline in any string (SPEC-002 section 4.5).
fn check_control_characters(name: &str, text: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("seed/{name}: {e}"))?;
    let mut stack = vec![&value];
    while let Some(value) = stack.pop() {
        match value {
            Value::String(s) => {
                if let Some(c) = s.chars().find(|c| c.is_control() && *c != '\n') {
                    return Err(format!(
                        "seed/{name}: control character U+{:04X} in {s:?}",
                        u32::from(c)
                    ));
                }
            }
            Value::Array(items) => stack.extend(items),
            Value::Object(map) => stack.extend(map.values()),
            _ => {}
        }
    }
    Ok(())
}
```

### File: `tools/seed-build/src/model.rs` (new)
```rust
//! Seed file shapes. Every struct denies unknown fields, so a dropped or misspelled field fails.

use crate::SeedSources;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Love-status key to text; `None` is a JSON `null` (no text stored).
pub type TextByStatus = BTreeMap<String, Option<String>>;

pub struct Seed {
    pub reading_config: ReadingConfig,
    pub cards: Vec<ReadingCard>,
    pub card_art: CardArtManifest,
    pub daily: DailyDocument,
}

impl Seed {
    pub fn parse(sources: &SeedSources) -> Result<Self, String> {
        Ok(Self {
            reading_config: parse("reading-config.json", &sources.reading_config)?,
            cards: parse("cards.json", &sources.cards)?,
            card_art: parse("card-art.json", &sources.card_art)?,
            daily: parse("daily.json", &sources.daily)?,
        })
    }
}

fn parse<T: DeserializeOwned>(name: &str, text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("seed/{name}: {e}"))
}

/// `GET /v1/cards` item.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingCard {
    pub id: String,
    pub name: String,
    pub keyword: String,
    pub art: Option<String>,
    pub meanings: TextByStatus,
    pub details: TextByStatus,
}

/// `GET /v1/daily-cards`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyDocument {
    pub schema_version: i64,
    pub version: String,
    pub updated: String,
    pub status: String,
    pub scope: String,
    pub auto_fallback_to_general_or_spread: bool,
    pub love_statuses: Vec<DailyLoveStatus>,
    pub cards: Vec<DailyCard>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyLoveStatus {
    pub key: String,
    pub source_label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyCard {
    pub card_id: String,
    pub name: String,
    pub arcana: String,
    pub reading_mode: String,
    pub time_horizon: String,
    pub orientation: String,
    pub content_status: String,
    pub approved_for_web: bool,
    pub missing_fields: Vec<String>,
    pub variants: Variants,
    pub content: DailyContent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variants {
    pub summary_theme: Option<String>,
    pub overview_theme: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyContent {
    pub theme: String,
    pub overview: String,
    pub work: String,
    pub money: String,
    pub love_by_status: TextByStatus,
    pub rabbit_message: String,
    pub colors: String,
    pub affirmation: String,
}

/// `GET /v1/card-art`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardArtManifest {
    pub cards: Vec<CardArt>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardArt {
    pub id: String,
    pub art: String,
    pub width: i64,
    pub height: i64,
    #[serde(rename = "renderedWidth")]
    pub rendered_width: i64,
    #[serde(rename = "renderedHeight")]
    pub rendered_height: i64,
    pub sha256: String,
    pub bytes: i64,
}

/// `GET /v1/reading-config`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingConfig {
    pub categories: Vec<Category>,
    pub situations: BTreeMap<String, Vec<LabeledKey>>,
    pub suggestions: BTreeMap<String, Vec<Suggestion>>,
    pub positions: Vec<LabeledKey>,
    pub uncategorized_situations: Vec<LabeledKey>,
    pub uncategorized_suggestions: Vec<Suggestion>,
    pub love_statuses: Vec<LoveStatus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub hint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledKey {
    pub key: String,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub id: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoveStatus {
    pub key: String,
    pub daily_label: String,
    pub reading_label: String,
}
```

### File: `tools/seed-build/src/validate.rs` (new)
```rust
//! Cross-file checks that serde cannot express. Each error names the file and the record.

use crate::model::{Seed, TextByStatus};
use std::collections::{BTreeSet, HashMap};

pub fn validate(seed: &Seed) -> Result<(), String> {
    let love_keys = love_status_keys(seed)?;
    reading_config(seed)?;
    let card_art = cards(seed, &love_keys)?;
    card_art_entries(seed, &card_art)?;
    daily(seed, &love_keys, &card_art)
}

fn unique<'a>(
    file: &str,
    what: &str,
    keys: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<&'a str>, String> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return Err(format!("seed/{file}: duplicate {what} `{key}`"));
        }
    }
    Ok(seen)
}

fn love_status_keys(seed: &Seed) -> Result<BTreeSet<&str>, String> {
    let config = &seed.reading_config;
    let keys = unique(
        "reading-config.json",
        "love status",
        config.love_statuses.iter().map(|s| s.key.as_str()),
    )?;
    let daily_keys = unique(
        "daily.json",
        "love status",
        seed.daily.love_statuses.iter().map(|s| s.key.as_str()),
    )?;
    if keys.is_empty() {
        return Err("seed/reading-config.json: love_statuses is empty".to_string());
    }
    if keys != daily_keys {
        return Err(format!(
            "love statuses differ: reading-config.json has {keys:?}, daily.json has {daily_keys:?}"
        ));
    }
    for status in &seed.daily.love_statuses {
        let same_label = config
            .love_statuses
            .iter()
            .any(|s| s.key == status.key && s.daily_label == status.source_label);
        if !same_label {
            return Err(format!(
                "love status `{}`: daily.json source_label differs from reading-config.json daily_label",
                status.key
            ));
        }
    }
    Ok(keys)
}

fn same_keys(
    file: &str,
    record: &str,
    field: &str,
    map: &TextByStatus,
    love_keys: &BTreeSet<&str>,
) -> Result<(), String> {
    let keys: BTreeSet<&str> = map.keys().map(String::as_str).collect();
    if keys == *love_keys {
        Ok(())
    } else {
        Err(format!(
            "seed/{file}: {record}.{field} keys {keys:?} differ from the love statuses {love_keys:?}"
        ))
    }
}

fn reading_config(seed: &Seed) -> Result<(), String> {
    const FILE: &str = "reading-config.json";
    let config = &seed.reading_config;
    let categories = unique(FILE, "category", config.categories.iter().map(|c| c.key.as_str()))?;
    unique(FILE, "position", config.positions.iter().map(|p| p.key.as_str()))?;
    if !config.uncategorized_situations.is_empty() || !config.uncategorized_suggestions.is_empty() {
        return Err(format!(
            "seed/{FILE}: uncategorized_situations and uncategorized_suggestions must be empty in the seed"
        ));
    }
    for category in config.situations.keys().chain(config.suggestions.keys()) {
        if !categories.contains(category.as_str()) {
            return Err(format!(
                "seed/{FILE}: situations or suggestions use unknown category `{category}`"
            ));
        }
    }
    // situations.love is derived from love_statuses (key, reading_label) and is not stored.
    let derived: Vec<(&str, &str)> = config
        .love_statuses
        .iter()
        .map(|s| (s.key.as_str(), s.reading_label.as_str()))
        .collect();
    let listed: Option<Vec<(&str, &str)>> = config.situations.get("love").map(|list| {
        list.iter()
            .map(|s| (s.key.as_str(), s.label.as_str()))
            .collect()
    });
    match (categories.contains("love"), listed) {
        (true, Some(listed)) if listed == derived => {}
        (false, None) => {}
        _ => {
            return Err(format!(
                "seed/{FILE}: situations.love must equal love_statuses (key, reading_label) in order, \
                 and exist only when the category `love` exists"
            ))
        }
    }
    unique(
        FILE,
        "situation",
        config
            .situations
            .iter()
            .filter(|(category, _)| category.as_str() != "love")
            .flat_map(|(_, list)| list.iter().map(|s| s.key.as_str())),
    )?;
    unique(
        FILE,
        "suggestion id",
        config.suggestions.values().flatten().map(|s| s.id.as_str()),
    )?;
    Ok(())
}

/// Returns card id to art path (`None` for a card without art).
fn cards<'a>(
    seed: &'a Seed,
    love_keys: &BTreeSet<&str>,
) -> Result<HashMap<&'a str, Option<&'a str>>, String> {
    const FILE: &str = "cards.json";
    unique(FILE, "card id", seed.cards.iter().map(|c| c.id.as_str()))?;
    for card in &seed.cards {
        same_keys(FILE, &card.id, "meanings", &card.meanings, love_keys)?;
        same_keys(FILE, &card.id, "details", &card.details, love_keys)?;
    }
    Ok(seed
        .cards
        .iter()
        .map(|c| (c.id.as_str(), c.art.as_deref()))
        .collect())
}

/// D09 item 5: an art entry's `art` equals its card's `art`; a card without art has no entry.
fn card_art_entries(seed: &Seed, card_art: &HashMap<&str, Option<&str>>) -> Result<(), String> {
    const FILE: &str = "card-art.json";
    unique(FILE, "art entry", seed.card_art.cards.iter().map(|a| a.id.as_str()))?;
    for entry in &seed.card_art.cards {
        match card_art.get(entry.id.as_str()) {
            None => return Err(format!("seed/{FILE}: art entry `{}` has no card", entry.id)),
            Some(art) if *art != Some(entry.art.as_str()) => {
                return Err(format!(
                    "seed/{FILE}: art entry `{}` art differs from the card's art {art:?}",
                    entry.id
                ))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn daily(
    seed: &Seed,
    love_keys: &BTreeSet<&str>,
    cards: &HashMap<&str, Option<&str>>,
) -> Result<(), String> {
    const FILE: &str = "daily.json";
    unique(FILE, "daily card", seed.daily.cards.iter().map(|d| d.card_id.as_str()))?;
    for entry in &seed.daily.cards {
        if !cards.contains_key(entry.card_id.as_str()) {
            return Err(format!("seed/{FILE}: daily card `{}` has no card", entry.card_id));
        }
        same_keys(
            FILE,
            &entry.card_id,
            "content.love_by_status",
            &entry.content.love_by_status,
            love_keys,
        )?;
        let expected: Vec<String> = seed
            .daily
            .love_statuses
            .iter()
            .filter(|s| matches!(entry.content.love_by_status.get(&s.key), Some(None)))
            .map(|s| format!("content.love_by_status.{}", s.key))
            .collect();
        if entry.missing_fields != expected {
            return Err(format!(
                "seed/{FILE}: daily card `{}` missing_fields {:?} differ from the null texts {expected:?}",
                entry.card_id, entry.missing_fields
            ));
        }
    }
    Ok(())
}
```

### File: `tools/seed-build/src/sql.rs` (new)
```rust
//! Renders the validated seed as one upsert per row, parents before children.

use crate::model::{Seed, TextByStatus};

pub const HEADER: &str = "-- Generated by tools/seed-build from seed/*.json (SPEC-002a). Do not edit.\n\
-- One INSERT ... ON CONFLICT DO UPDATE per row, parents before children. No DELETE, no audit_events.\n";

enum Val<'a> {
    Text(&'a str),
    Null,
    Int(i64),
}

impl Val<'_> {
    fn sql(&self) -> String {
        match self {
            Val::Text(s) => format!("'{}'", s.replace('\'', "''")),
            Val::Null => "NULL".to_string(),
            Val::Int(n) => n.to_string(),
        }
    }
}

fn opt(value: Option<&str>) -> Val<'_> {
    value.map_or(Val::Null, Val::Text)
}

fn flag(value: bool) -> Val<'static> {
    Val::Int(i64::from(value))
}

/// Sort order for the item at `index` in file order: 10, 20, 30, ... (gaps for later inserts).
fn order(index: usize) -> Val<'static> {
    Val::Int((index as i64 + 1) * 10)
}

/// `INSERT INTO <table> (...) VALUES (...) ON CONFLICT(<key>) DO UPDATE SET <non-key> = excluded.<non-key>;`
fn upsert(table: &str, key: &[&str], columns: &[(&str, Val<'_>)]) -> String {
    let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    let values: Vec<String> = columns.iter().map(|(_, value)| value.sql()).collect();
    let updates: Vec<String> = names
        .iter()
        .filter(|name| !key.contains(*name))
        .map(|name| format!("{name} = excluded.{name}"))
        .collect();
    format!(
        "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT({}) DO UPDATE SET {};",
        names.join(", "),
        values.join(", "),
        key.join(", "),
        updates.join(", ")
    )
}

fn love_texts(out: &mut Vec<String>, table: &str, card_id: &str, texts: &TextByStatus, keys: &[&str]) {
    for key in keys {
        let text = texts.get(*key).and_then(Option::as_deref);
        out.push(upsert(
            table,
            &["card_id", "love_status"],
            &[
                ("card_id", Val::Text(card_id)),
                ("love_status", Val::Text(key)),
                ("text", opt(text)),
            ],
        ));
    }
}

pub fn render(seed: &Seed) -> Vec<String> {
    let config = &seed.reading_config;
    let daily = &seed.daily;
    let reading_keys: Vec<&str> = config.love_statuses.iter().map(|s| s.key.as_str()).collect();
    let daily_keys: Vec<&str> = daily.love_statuses.iter().map(|s| s.key.as_str()).collect();
    let mut out = Vec::new();

    for (i, status) in config.love_statuses.iter().enumerate() {
        let daily_index = daily_keys
            .iter()
            .position(|key| *key == status.key)
            .expect("validated: both files list the same love statuses");
        out.push(upsert(
            "love_statuses",
            &["key"],
            &[
                ("key", Val::Text(&status.key)),
                ("daily_label", Val::Text(&status.daily_label)),
                ("reading_label", Val::Text(&status.reading_label)),
                ("daily_sort_order", order(daily_index)),
                ("reading_sort_order", order(i)),
            ],
        ));
    }
    for (i, category) in config.categories.iter().enumerate() {
        out.push(upsert(
            "categories",
            &["key"],
            &[
                ("key", Val::Text(&category.key)),
                ("label", Val::Text(&category.label)),
                ("icon", Val::Text(&category.icon)),
                ("hint", Val::Text(&category.hint)),
                ("sort_order", order(i)),
            ],
        ));
    }
    for category in config.categories.iter().filter(|c| c.key != "love") {
        for (i, situation) in config.situations.get(&category.key).into_iter().flatten().enumerate() {
            out.push(upsert(
                "situations",
                &["key"],
                &[
                    ("key", Val::Text(&situation.key)),
                    ("category", Val::Text(&category.key)),
                    ("label", Val::Text(&situation.label)),
                    ("sort_order", order(i)),
                ],
            ));
        }
    }
    for category in &config.categories {
        for (i, suggestion) in config.suggestions.get(&category.key).into_iter().flatten().enumerate() {
            out.push(upsert(
                "suggestions",
                &["id"],
                &[
                    ("id", Val::Text(&suggestion.id)),
                    ("category", Val::Text(&category.key)),
                    ("text", Val::Text(&suggestion.text)),
                    ("sort_order", order(i)),
                ],
            ));
        }
    }
    for (i, position) in config.positions.iter().enumerate() {
        out.push(upsert(
            "positions",
            &["key"],
            &[
                ("key", Val::Text(&position.key)),
                ("label", Val::Text(&position.label)),
                ("sort_order", order(i)),
            ],
        ));
    }
    for (i, card) in seed.cards.iter().enumerate() {
        out.push(upsert(
            "cards",
            &["id"],
            &[
                ("id", Val::Text(&card.id)),
                ("name", Val::Text(&card.name)),
                ("keyword", Val::Text(&card.keyword)),
                ("art", opt(card.art.as_deref())),
                ("sort_order", order(i)),
            ],
        ));
    }
    for card in &seed.cards {
        love_texts(&mut out, "card_meanings", &card.id, &card.meanings, &reading_keys);
    }
    for card in &seed.cards {
        love_texts(&mut out, "card_details", &card.id, &card.details, &reading_keys);
    }
    for entry in &seed.card_art.cards {
        out.push(upsert(
            "card_art",
            &["card_id"],
            &[
                ("card_id", Val::Text(&entry.id)),
                ("art", Val::Text(&entry.art)),
                ("width", Val::Int(entry.width)),
                ("height", Val::Int(entry.height)),
                ("rendered_width", Val::Int(entry.rendered_width)),
                ("rendered_height", Val::Int(entry.rendered_height)),
                ("bytes", Val::Int(entry.bytes)),
                ("sha256", Val::Text(&entry.sha256)),
            ],
        ));
    }
    out.push(upsert(
        "daily_meta",
        &["id"],
        &[
            ("id", Val::Int(1)),
            ("schema_version", Val::Int(daily.schema_version)),
            ("version", Val::Text(&daily.version)),
            ("updated", Val::Text(&daily.updated)),
            ("status", Val::Text(&daily.status)),
            ("scope", Val::Text(&daily.scope)),
            (
                "auto_fallback_to_general_or_spread",
                flag(daily.auto_fallback_to_general_or_spread),
            ),
        ],
    ));
    for (i, entry) in daily.cards.iter().enumerate() {
        let content = &entry.content;
        out.push(upsert(
            "daily_cards",
            &["card_id"],
            &[
                ("card_id", Val::Text(&entry.card_id)),
                ("name", Val::Text(&entry.name)),
                ("arcana", Val::Text(&entry.arcana)),
                ("reading_mode", Val::Text(&entry.reading_mode)),
                ("time_horizon", Val::Text(&entry.time_horizon)),
                ("orientation", Val::Text(&entry.orientation)),
                ("content_status", Val::Text(&entry.content_status)),
                ("approved_for_web", flag(entry.approved_for_web)),
                ("theme", Val::Text(&content.theme)),
                ("overview", Val::Text(&content.overview)),
                ("work", Val::Text(&content.work)),
                ("money", Val::Text(&content.money)),
                ("rabbit_message", Val::Text(&content.rabbit_message)),
                ("colors", Val::Text(&content.colors)),
                ("affirmation", Val::Text(&content.affirmation)),
                ("summary_theme", opt(entry.variants.summary_theme.as_deref())),
                ("overview_theme", opt(entry.variants.overview_theme.as_deref())),
                ("sort_order", order(i)),
            ],
        ));
    }
    for entry in &daily.cards {
        love_texts(
            &mut out,
            "daily_love_texts",
            &entry.card_id,
            &entry.content.love_by_status,
            &daily_keys,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_values_escape_single_quotes() {
        assert_eq!(Val::Text("it's").sql(), "'it''s'");
    }

    #[test]
    fn newlines_and_semicolons_stay_inside_the_literal() {
        assert_eq!(Val::Text("a;\nb").sql(), "'a;\nb'");
    }

    #[test]
    fn missing_text_is_null() {
        assert_eq!(opt(None).sql(), "NULL");
        assert_eq!(flag(true).sql(), "1");
        assert_eq!(flag(false).sql(), "0");
    }

    #[test]
    fn sort_order_starts_at_ten_with_gaps() {
        assert_eq!(order(0).sql(), "10");
        assert_eq!(order(2).sql(), "30");
    }

    #[test]
    fn upsert_updates_every_non_key_column() {
        let sql = upsert(
            "card_meanings",
            &["card_id", "love_status"],
            &[
                ("card_id", Val::Text("the-fool")),
                ("love_status", Val::Text("single")),
                ("text", Val::Null),
            ],
        );
        assert_eq!(
            sql,
            "INSERT INTO card_meanings (card_id, love_status, text) VALUES ('the-fool', 'single', NULL) \
             ON CONFLICT(card_id, love_status) DO UPDATE SET text = excluded.text;"
        );
    }
}
```

### File: `tools/seed-build/src/main.rs` (new)
```rust
//! `seed-build --seed-dir=<dir> --out=<file>` validates the seed and writes the SQL file.
//! On any error it removes a stale output file and exits with status 1, so `make seed-local`
//! stops before it calls wrangler.

use seed_build::{build_statements, render_sql, SeedSources};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, io};

struct Args {
    seed_dir: PathBuf,
    out: PathBuf,
}

const USAGE: &str = "usage: seed-build --seed-dir=<dir> --out=<file>";

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut seed_dir = None;
    let mut out = None;
    for arg in args {
        if let Some(value) = arg.strip_prefix("--seed-dir=") {
            seed_dir = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--out=") {
            out = Some(PathBuf::from(value));
        } else {
            return Err(format!("unsupported argument: {arg} ({USAGE})"));
        }
    }
    Ok(Args {
        seed_dir: seed_dir.ok_or_else(|| format!("missing --seed-dir ({USAGE})"))?,
        out: out.ok_or_else(|| format!("missing --out ({USAGE})"))?,
    })
}

/// Builds and writes the SQL file; returns the number of statements.
fn run(args: &Args) -> Result<usize, String> {
    let sources = SeedSources::read_dir(&args.seed_dir)?;
    let statements = build_statements(&sources)?;
    if let Some(parent) = args.out.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    fs::write(&args.out, render_sql(&statements))
        .map_err(|e| format!("cannot write {}: {e}", args.out.display()))?;
    Ok(statements.len())
}

/// `run`, plus removal of a stale output file when the build fails.
fn execute(args: &Args) -> Result<usize, String> {
    run(args).map_err(|msg| match remove_stale(&args.out) {
        Ok(()) => msg,
        Err(e) => format!("{msg}; also cannot remove stale {}: {e}", args.out.display()),
    })
}

fn remove_stale(out: &Path) -> io::Result<()> {
    match fs::remove_file(out) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = env::args().skip(1).collect();
    let result = parse_args(&raw).and_then(|args| {
        execute(&args).map(|count| (count, args.out))
    });
    match result {
        Ok((count, out)) => {
            println!("seed-build: wrote {count} statements to {}", out.display());
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("seed-build: {msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("seed-build-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clean temp dir");
        }
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_string()).collect()
    }

    #[test]
    fn rejects_unknown_and_missing_arguments() {
        assert!(parse_args(&args(&["--seed-dir=seed", "--out=x.sql", "--remote"])).is_err());
        assert!(parse_args(&args(&["--seed-dir=seed"])).is_err());
        assert!(parse_args(&args(&["--out=x.sql"])).is_err());
    }

    #[test]
    fn a_failed_build_removes_the_stale_output_and_writes_nothing() {
        let dir = temp_dir("failed");
        let out = dir.join("build").join("seed.sql");
        fs::create_dir_all(out.parent().expect("parent")).expect("create build dir");
        fs::write(&out, "-- stale").expect("write stale file");
        let parsed = Args {
            seed_dir: dir.join("no-such-seed-dir"),
            out: out.clone(),
        };
        assert!(execute(&parsed).is_err());
        assert!(!out.exists(), "a stale seed.sql must not survive a failed build");
    }

    #[test]
    fn a_successful_build_writes_every_statement() {
        let dir = temp_dir("ok");
        let parsed = Args {
            seed_dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../seed"),
            out: dir.join("build").join("seed.sql"),
        };
        assert_eq!(execute(&parsed), Ok(1669));
        let sql = fs::read_to_string(&parsed.out).expect("read output");
        assert!(sql.starts_with("-- Generated by tools/seed-build"));
    }
}
```

## 6. Green + Standards Check
```bash
cargo test -p seed-build
make test-d1
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
make check
```
Expected:
- `cargo test -p seed-build` passes:
  - lib unit tests (`sql::tests`): 5 passed;
  - bin unit tests (`main.rs`): 3 passed;
  - `migrations_ddl_only`: 3 passed;
  - `seed_files`: 7 passed;
  - `seed_validation`: 20 passed;
  - `d1`: 10 ignored.
- `make test-d1`: `test result: ok. 10 passed; 0 failed; 0 ignored` (about 2 minutes).
- `fmt --check` prints nothing; clippy has no warnings; `make check` succeeds (`seed-build` and `d1-local` are excluded from wasm32).

If clippy reports a lint on code from this task, the fix is a plan defect. Report the exact lint; do not add `#[allow]`.
