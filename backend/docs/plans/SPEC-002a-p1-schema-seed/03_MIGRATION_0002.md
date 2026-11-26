---
model: sonnet
effort: medium
reason: "Exact DDL and tests supplied; the executor runs host and D1-level red/green"
---

# Task 03: Migration `0002_master_data.sql` (DDL only), scenario 28 and scenario A2

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Depends on task 02 (crates `seed-build` and `d1-local`, `TABLE_ORDER`, `D1Local`, `make test-d1`).

All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Write `tools/seed-build/tests/migrations_ddl_only.rs` (scenario 28, host test).
- [ ] Write `tools/seed-build/tests/d1/schema.rs` (scenario A2 and the schema rules) and add `mod schema;` to `tests/d1/main.rs`.
- [ ] Red, then create `migrations/0002_master_data.sql`, then green.

## 2. Harness
None new. Uses `D1Local` from the `d1-local` crate (task 02). The D1 tests insert one valid fixture row per table directly (A2 says "GIVEN the migrations are applied"; no seed is needed).

## 3. Tests (written first)

### File: `tools/seed-build/tests/migrations_ddl_only.rs` (new)
```rust
//! Scenario 28: `migrations/` holds DDL only, no seed data. Also checks the shape of 0002.

use seed_build::TABLE_ORDER;
use std::fs;
use std::path::{Path, PathBuf};

fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations")
}

/// The statements of a migration file with `--` comments removed.
fn statements(sql: &str) -> Vec<String> {
    let without_comments: Vec<&str> = sql
        .lines()
        .map(|line| line.split("--").next().unwrap_or(""))
        .collect();
    without_comments
        .join("\n")
        .split(';')
        .map(|stmt| stmt.trim().to_string())
        .filter(|stmt| !stmt.is_empty())
        .collect()
}

fn master_data_statements() -> Vec<String> {
    let path = migrations_dir().join("0002_master_data.sql");
    let sql = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    statements(&sql)
}

#[test]
fn every_migration_holds_ddl_only() {
    let mut files: Vec<PathBuf> = fs::read_dir(migrations_dir())
        .expect("read migrations/")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    assert!(files.len() >= 2, "expected 0001 and 0002, found {files:?}");
    for file in &files {
        let sql = fs::read_to_string(file).expect("read migration");
        for stmt in statements(&sql) {
            let upper = stmt.to_ascii_uppercase();
            assert!(
                upper.starts_with("CREATE TABLE ")
                    || upper.starts_with("CREATE INDEX ")
                    || upper.starts_with("CREATE UNIQUE INDEX "),
                "{} holds a non-DDL statement: {stmt}",
                file.display()
            );
        }
    }
}

#[test]
fn master_data_migration_creates_every_table() {
    let ddl = master_data_statements().join(";\n");
    for table in TABLE_ORDER {
        assert!(
            ddl.contains(&format!("CREATE TABLE IF NOT EXISTS {table} (")),
            "0002 does not create {table}"
        );
    }
}

#[test]
fn master_data_foreign_keys_have_no_action() {
    let ddl = master_data_statements().join(";\n").to_ascii_uppercase();
    assert!(
        !ddl.contains("ON DELETE") && !ddl.contains("ON UPDATE"),
        "master-data foreign keys take no delete or update action (D09, D10)"
    );
    assert_eq!(ddl.matches(" REFERENCES ").count(), 10, "expected 10 foreign keys");
}
```

### File: `tools/seed-build/tests/d1/main.rs`, exact diff
```diff
 mod harness;
+mod schema;
```

### File: `tools/seed-build/tests/d1/schema.rs` (new)
```rust
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
        cases.push((format!("UPDATE daily_meta SET {column} = NULL WHERE id = 1"), NOT_NULL));
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
```
SQLite checks NOT NULL while it writes the row and checks foreign keys at the end of the statement, so the `card_art` row for `x-card` with `art = NULL` fails with `NOT NULL constraint failed: card_art.art`.

## 4. Red (run before creating the migration)
```bash
cargo test -p seed-build --test migrations_ddl_only
make test-d1
```
Expected:
- Host: FAIL. `every_migration_holds_ddl_only` panics with `expected 0001 and 0002, found [".../migrations/0001_init_auth.sql"]`, and the other two panic with `cannot read .../0002_master_data.sql: No such file or directory`.
- D1: the 4 harness tests pass; the 4 `schema::` tests FAIL with `fixture rows are valid: "no such table: love_statuses: SQLITE_ERROR"`.

## 5. Implementation

### File: `migrations/0002_master_data.sql` (new)
```sql
-- Canonical shared D1 schema migration 0002: master data (SPEC-002a; SPEC-002 sections 4.3 and 4.4).
-- DDL only (D07): rows come from seed/*.json through `make seed-local|seed-staging|seed-prod`.
-- Foreign keys take no delete or update action (D09). D1 enforces foreign keys on every statement.
-- `category` is nullable on situations and suggestions: NULL means uncategorized (D10).
-- Love situations are not stored; they are derived from love_statuses (SPEC-002 section 4.3).
-- Booleans are INTEGER 0/1. Sort orders are per screen; ties are broken by key or id when read.

CREATE TABLE IF NOT EXISTS love_statuses (
    key TEXT PRIMARY KEY NOT NULL,
    daily_label TEXT NOT NULL,
    reading_label TEXT NOT NULL,
    daily_sort_order INTEGER NOT NULL,
    reading_sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    key TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    icon TEXT NOT NULL,
    hint TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS situations (
    key TEXT PRIMARY KEY NOT NULL,
    category TEXT REFERENCES categories (key),
    label TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_situations_category ON situations (category);

CREATE TABLE IF NOT EXISTS suggestions (
    id TEXT PRIMARY KEY NOT NULL,
    category TEXT REFERENCES categories (key),
    text TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_suggestions_category ON suggestions (category);

CREATE TABLE IF NOT EXISTS positions (
    key TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    keyword TEXT NOT NULL,
    art TEXT,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS card_meanings (
    card_id TEXT NOT NULL REFERENCES cards (id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_card_meanings_love_status ON card_meanings (love_status);

CREATE TABLE IF NOT EXISTS card_details (
    card_id TEXT NOT NULL REFERENCES cards (id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_card_details_love_status ON card_details (love_status);

CREATE TABLE IF NOT EXISTS card_art (
    card_id TEXT PRIMARY KEY NOT NULL REFERENCES cards (id),
    art TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    rendered_width INTEGER NOT NULL,
    rendered_height INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    sha256 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS daily_meta (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    schema_version INTEGER NOT NULL,
    version TEXT NOT NULL,
    updated TEXT NOT NULL,
    status TEXT NOT NULL,
    scope TEXT NOT NULL,
    auto_fallback_to_general_or_spread INTEGER NOT NULL CHECK (auto_fallback_to_general_or_spread IN (0, 1))
);

CREATE TABLE IF NOT EXISTS daily_cards (
    card_id TEXT PRIMARY KEY NOT NULL REFERENCES cards (id),
    name TEXT NOT NULL,
    arcana TEXT NOT NULL,
    reading_mode TEXT NOT NULL,
    time_horizon TEXT NOT NULL,
    orientation TEXT NOT NULL,
    content_status TEXT NOT NULL,
    approved_for_web INTEGER NOT NULL CHECK (approved_for_web IN (0, 1)),
    theme TEXT NOT NULL,
    overview TEXT NOT NULL,
    work TEXT NOT NULL,
    money TEXT NOT NULL,
    rabbit_message TEXT NOT NULL,
    colors TEXT NOT NULL,
    affirmation TEXT NOT NULL,
    summary_theme TEXT,
    overview_theme TEXT,
    sort_order INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS daily_love_texts (
    card_id TEXT NOT NULL REFERENCES daily_cards (card_id),
    love_status TEXT NOT NULL REFERENCES love_statuses (key),
    text TEXT,
    PRIMARY KEY (card_id, love_status)
);

CREATE INDEX IF NOT EXISTS idx_daily_love_texts_love_status ON daily_love_texts (love_status);
```

## 6. Green + Standards Check
```bash
cargo test -p seed-build
make test-d1
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```
Expected:
- `cargo test -p seed-build`: `migrations_ddl_only`: `test result: ok. 3 passed`; `d1`: `8 ignored`.
- `make test-d1`: `test result: ok. 8 passed; 0 failed; 0 ignored` (about 70 to 110 s; about 40 wrangler calls).
- `fmt --check` prints nothing; clippy has no warnings.

Do not run `make migrate-staging` or `make migrate-prod` (`[CLOUD]`, owner only).
