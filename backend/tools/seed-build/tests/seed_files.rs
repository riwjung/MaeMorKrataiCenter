//! The committed `seed/` files: hygiene (no dropped field, no ChatGPT link) and the rendered SQL.

use seed_build::{
    build_sql, build_statements, SeedSources, FORBIDDEN_TOKENS, SEED_FILES, TABLE_ORDER,
};
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
    assert!(
        !sql.contains("audit_events"),
        "the seed must not write audit_events"
    );
    assert!(
        !sql.lines()
            .any(|line| line.trim_start().to_ascii_uppercase().starts_with("DELETE")),
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
    assert!(
        order.windows(2).all(|pair| pair[0] <= pair[1]),
        "tables out of load order"
    );
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
