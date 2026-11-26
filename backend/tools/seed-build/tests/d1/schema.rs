//! Scenario A2 and the 0002 schema rules on real D1: references, uniqueness, CHECKs, NOT NULL,
//! nullable `category`, and no cascade. Every rejected statement must leave all tables unchanged.

use d1_local::D1Local;
use seed_build::TABLE_ORDER;

const FK: &str = "FOREIGN KEY constraint failed";
const UNIQUE: &str = "UNIQUE constraint failed";
const CHECK: &str = "CHECK constraint failed";
const NOT_NULL: &str = "NOT NULL constraint failed";

/// Contract fields that are required and non-nullable (openapi v0.2.1): every `daily_meta` column.
const DAILY_META_REQUIRED: [&str; 6] = [
    "schema_version",
    "version",
    "updated",
    "status",
    "scope",
    "auto_fallback_to_general_or_spread",
];
/// Every `daily_cards` column except the key, `summary_theme` and `overview_theme`.
const DAILY_CARDS_REQUIRED: [&str; 15] = [
    "name",
    "arcana",
    "reading_mode",
    "time_horizon",
    "orientation",
    "content_status",
    "approved_for_web",
    "theme",
    "overview",
    "work",
    "money",
    "rabbit_message",
    "colors",
    "affirmation",
    "sort_order",
];

/// One valid row per master-data table, parents first.
const FIXTURE: &str = r#"
INSERT INTO love_statuses (key, daily_label, reading_label, daily_sort_order, reading_sort_order) VALUES ('single', 'daily', 'reading', 10, 10);
INSERT INTO categories (key, label, icon, hint, sort_order) VALUES ('work', 'label', 'icon', 'hint', 10);
INSERT INTO situations (key, category, label, sort_order) VALUES ('current_job', 'work', 'label', 10);
INSERT INTO suggestions (id, category, text, sort_order) VALUES ('work-1', 'work', 'text', 10);
INSERT INTO positions (key, label, sort_order) VALUES ('past', 'label', 10);
INSERT INTO cards (id, name, keyword, art, sort_order) VALUES ('the-fool', 'The Fool', 'keyword', '/cards/v3/the-fool-e11e1ee168a4.webp', 10);
INSERT INTO card_meanings (card_id, love_status, text) VALUES ('the-fool', 'single', 'meaning');
INSERT INTO card_details (card_id, love_status, text) VALUES ('the-fool', 'single', NULL);
INSERT INTO card_art (card_id, art, width, height, rendered_width, rendered_height, bytes, sha256) VALUES ('the-fool', '/cards/v3/the-fool-e11e1ee168a4.webp', 637, 1024, 600, 965, 25866, 'e11e1ee168a484e8002b3f17175cdd9f54fe69b5faa754c056f9614bdd70e160');
INSERT INTO daily_meta (id, schema_version, version, updated, status, scope, auto_fallback_to_general_or_spread) VALUES (1, 1, 'daily-v1', '2026-10-20', 'draft', 'scope', 0);
INSERT INTO daily_cards (card_id, name, arcana, reading_mode, time_horizon, orientation, content_status, approved_for_web, theme, overview, work, money, rabbit_message, colors, affirmation, summary_theme, overview_theme, sort_order) VALUES ('the-fool', 'The Fool', 'major', 'daily_single', 'today', 'upright_only', 'draft', 0, 'theme', 'overview', 'work', 'money', 'rabbit', 'colors', 'affirmation', NULL, 'overview theme', 10);
INSERT INTO daily_love_texts (card_id, love_status, text) VALUES ('the-fool', 'single', NULL);
"#;

fn fixture_db(name: &str) -> D1Local {
    let db = D1Local::migrated(name);
    db.exec(FIXTURE).expect("fixture rows are valid");
    db
}

fn table_counts_sql() -> String {
    let counts: Vec<String> = TABLE_ORDER
        .iter()
        .map(|table| format!("(SELECT COUNT(*) FROM {table}) AS {table}"))
        .collect();
    format!("SELECT {}", counts.join(", "))
}

fn assert_all_rejected(db: &D1Local, cases: &[(&str, &str)]) {
    let before = db.snapshot(&TABLE_ORDER);
    for (sql, needle) in cases {
        db.assert_rejected(sql, needle);
    }
    assert_eq!(
        db.snapshot(&TABLE_ORDER),
        before,
        "a rejected statement changed a table"
    );
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn a_valid_row_is_accepted_in_every_table_and_category_may_be_null() {
    let db = fixture_db("schema_valid_rows");
    let row = &db.query(&table_counts_sql())[0];
    for table in TABLE_ORDER {
        assert_eq!(row[table], 1, "{table}");
    }
    db.exec(
        "UPDATE situations SET category = NULL WHERE key = 'current_job'; \
         INSERT INTO suggestions (id, category, text, sort_order) \
         VALUES ('5f0c6a8e-3b1d-4c2e-9f7a-0b1c2d3e4f50', NULL, 'text', 20);",
    )
    .expect("category is nullable (D10)");
    let uncategorized = db.query(
        "SELECT (SELECT COUNT(*) FROM situations WHERE category IS NULL) AS situations, \
         (SELECT COUNT(*) FROM suggestions WHERE category IS NULL) AS suggestions",
    );
    assert_eq!(uncategorized[0]["situations"], 1);
    assert_eq!(uncategorized[0]["suggestions"], 1);
}

/// SPEC-002a scenario A2, cases (a) to (d).
#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn scenario_a2_rejects_dangling_references_and_duplicate_keys() {
    let db = fixture_db("schema_a2");
    assert_all_rejected(
        &db,
        &[
            (
                "INSERT INTO daily_cards (card_id, name, arcana, reading_mode, time_horizon, orientation, content_status, approved_for_web, theme, overview, work, money, rabbit_message, colors, affirmation, summary_theme, overview_theme, sort_order) VALUES ('x-card', 'X', 'major', 'daily_single', 'today', 'upright_only', 'draft', 0, 't', 'o', 'w', 'm', 'r', 'c', 'a', NULL, NULL, 20)",
                FK,
            ),
            (
                "INSERT INTO situations (key, category, label, sort_order) VALUES ('new_situation', 'nope', 'label', 20)",
                FK,
            ),
            (
                "INSERT INTO card_meanings (card_id, love_status, text) VALUES ('the-fool', 'widowed', 'text')",
                FK,
            ),
            (
                "INSERT INTO cards (id, name, keyword, art, sort_order) VALUES ('the-fool', 'Again', 'keyword', NULL, 20)",
                UNIQUE,
            ),
        ],
    );
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn every_reference_key_check_and_required_column_is_enforced() {
    let db = fixture_db("schema_rules");
    assert_all_rejected(
        &db,
        &[
            // Every reference in SPEC-002 section 4.4.
            ("INSERT INTO card_art (card_id, art, width, height, rendered_width, rendered_height, bytes, sha256) VALUES ('x-card', '/cards/v3/x.webp', 1, 1, 1, 1, 1, 'aa')", FK),
            ("INSERT INTO card_meanings (card_id, love_status, text) VALUES ('x-card', 'single', 'text')", FK),
            ("INSERT INTO card_details (card_id, love_status, text) VALUES ('the-fool', 'widowed', 'text')", FK),
            ("INSERT INTO daily_love_texts (card_id, love_status, text) VALUES ('x-card', 'single', 'text')", FK),
            ("INSERT INTO daily_love_texts (card_id, love_status, text) VALUES ('the-fool', 'widowed', 'text')", FK),
            ("INSERT INTO suggestions (id, category, text, sort_order) VALUES ('nope-1', 'nope', 'text', 20)", FK),
            // Key uniqueness per table.
            ("INSERT INTO love_statuses (key, daily_label, reading_label, daily_sort_order, reading_sort_order) VALUES ('single', 'd', 'r', 20, 20)", UNIQUE),
            ("INSERT INTO categories (key, label, icon, hint, sort_order) VALUES ('work', 'l', 'i', 'h', 20)", UNIQUE),
            ("INSERT INTO situations (key, category, label, sort_order) VALUES ('current_job', 'work', 'l', 20)", UNIQUE),
            ("INSERT INTO suggestions (id, category, text, sort_order) VALUES ('work-1', 'work', 't', 20)", UNIQUE),
            ("INSERT INTO positions (key, label, sort_order) VALUES ('past', 'l', 20)", UNIQUE),
            ("INSERT INTO card_meanings (card_id, love_status, text) VALUES ('the-fool', 'single', 'again')", UNIQUE),
            ("INSERT INTO card_details (card_id, love_status, text) VALUES ('the-fool', 'single', 'again')", UNIQUE),
            ("INSERT INTO card_art (card_id, art, width, height, rendered_width, rendered_height, bytes, sha256) VALUES ('the-fool', '/cards/v3/x.webp', 1, 1, 1, 1, 1, 'aa')", UNIQUE),
            ("INSERT INTO daily_cards (card_id, name, arcana, reading_mode, time_horizon, orientation, content_status, approved_for_web, theme, overview, work, money, rabbit_message, colors, affirmation, summary_theme, overview_theme, sort_order) VALUES ('the-fool', 'X', 'major', 'daily_single', 'today', 'upright_only', 'draft', 0, 't', 'o', 'w', 'm', 'r', 'c', 'a', NULL, NULL, 20)", UNIQUE),
            ("INSERT INTO daily_love_texts (card_id, love_status, text) VALUES ('the-fool', 'single', 'again')", UNIQUE),
            // CHECK constraints: single daily_meta row, 0/1 booleans.
            ("INSERT INTO daily_meta (id, schema_version, version, updated, status, scope, auto_fallback_to_general_or_spread) VALUES (2, 1, 'v', '2026-10-20', 's', 'scope', 0)", CHECK),
            ("UPDATE daily_meta SET auto_fallback_to_general_or_spread = 2 WHERE id = 1", CHECK),
            ("UPDATE daily_cards SET approved_for_web = 2 WHERE card_id = 'the-fool'", CHECK),
            // Required columns.
            ("INSERT INTO cards (id, name, keyword, art, sort_order) VALUES ('new-card', NULL, 'keyword', NULL, 20)", NOT_NULL),
            ("INSERT INTO categories (key, label, icon, hint, sort_order) VALUES ('new_category', 'l', 'i', 'h', NULL)", NOT_NULL),
            ("INSERT INTO card_art (card_id, art, width, height, rendered_width, rendered_height, bytes, sha256) VALUES ('x-card', NULL, 1, 1, 1, 1, 1, 'aa')", NOT_NULL),
            // No cascade: a parent with children cannot be deleted.
            ("DELETE FROM cards WHERE id = 'the-fool'", FK),
            ("DELETE FROM love_statuses WHERE key = 'single'", FK),
            ("DELETE FROM categories WHERE key = 'work'", FK),
            ("DELETE FROM daily_cards WHERE card_id = 'the-fool'", FK),
        ],
    );
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn daily_columns_are_not_null_except_the_two_theme_variants() {
    let db = fixture_db("schema_daily_not_null");
    let mut cases: Vec<(String, &str)> = Vec::new();
    for column in DAILY_META_REQUIRED {
        cases.push((
            format!("UPDATE daily_meta SET {column} = NULL WHERE id = 1"),
            NOT_NULL,
        ));
    }
    for column in DAILY_CARDS_REQUIRED {
        cases.push((
            format!("UPDATE daily_cards SET {column} = NULL WHERE card_id = 'the-fool'"),
            NOT_NULL,
        ));
    }
    let before = db.snapshot(&TABLE_ORDER);
    for (sql, needle) in &cases {
        db.assert_rejected(sql, needle);
    }
    assert_eq!(
        db.snapshot(&TABLE_ORDER),
        before,
        "a rejected statement changed a table"
    );
    // The two variants are nullable (a card may have no variant text).
    db.exec(
        "UPDATE daily_cards SET summary_theme = NULL, overview_theme = NULL WHERE card_id = 'the-fool'",
    )
    .expect("summary_theme and overview_theme are nullable");
}
