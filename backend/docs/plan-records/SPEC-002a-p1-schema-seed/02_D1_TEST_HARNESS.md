---
model: sonnet
effort: medium
reason: "Complete code supplied; new crate wiring, a process-spawning test helper and a CI job need care but no design decisions"
---

# Task 02: `d1-local` harness crate, `seed-build` crate skeleton and `make test-d1`

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Battleground: [02_D1_TEST_HARNESS/debate.md](../../battlegrounds/SPEC-002a-p1-schema-seed/02_D1_TEST_HARNESS/debate.md) (B3). Winner: Rust integration tests that shell out to `npx wrangler d1 execute --local --persist-to target/tmp/d1/<test>`, `#[ignore]`d in plain `cargo test`, run by `make test-d1`, and run by CI in a separate required job.

**Plan Auditor decision (shared harness).** The harness lives in its own dev-only workspace crate `tools/d1-local` (library `d1_local`, only `serde_json`, `publish = false`, host only). Two test suites use it as a dev-dependency: `tools/seed-build/tests/d1/` (this plan: A2, 27, schema rules, seed load) and `client/tests/d1_master_data_sql.rs` (plan p2: the Worker's own SQL constants and parse code on local D1). This way the real SQL text and the production parse code run in the `client` crate's tests, there is exactly one copy of the wrangler driver, and `tools/seed-build` does not depend on `client`. Local D1 state for tests is always `backend/target/tmp/d1/<name>` (never `admin/.wrangler/state`).

All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Create `tools/d1-local/Cargo.toml` (the library file comes in the implementation step).
- [ ] Create `tools/seed-build/Cargo.toml` and `tools/seed-build/src/lib.rs` (skeleton).
- [ ] Add `tools/seed-build` and `tools/d1-local` to the workspace `members` in `Cargo.toml`.
- [ ] Makefile: exclude `seed-build` and `d1-local` from `check`; add the `test-d1` target.
- [ ] `../.github/workflows/backend-ci.yml`: exclude both from the wasm32 steps; add the `d1-tests` job.
- [ ] Write the harness self-tests `tools/seed-build/tests/d1/main.rs` and `tests/d1/harness.rs` (red), then `tools/d1-local/src/lib.rs` (green).
- [ ] Commit the updated `Cargo.lock`: it gains `seed-build` and `d1-local` package entries. CI runs `--locked`.

## 2. Harness: crate wiring

### File: `tools/d1-local/Cargo.toml` (new)
```toml
[package]
name = "d1-local"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
publish = false

[dependencies]
serde_json = "1"
```

### File: `tools/seed-build/Cargo.toml` (new)
```toml
[package]
name = "seed-build"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
d1-local = { path = "../d1-local" }
```

### File: `tools/seed-build/src/lib.rs` (new, skeleton; task 04 replaces this file completely)
```rust
//! Seed build for the SPEC-002a master data (D07).

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
```

### File: `Cargo.toml` (workspace), exact diff
```diff
-members = ["client", "admin", "tools/create-user"]
+members = ["client", "admin", "tools/create-user", "tools/seed-build", "tools/d1-local"]
```

### File: `Makefile`, exact diffs
```diff
 .PHONY: help check test build-admin build-client dev-admin dev-client \
-	migrate-local migrate-staging migrate-prod create-user \
+	test-d1 migrate-local migrate-staging migrate-prod create-user \
 	cf-whoami cf-bootstrap cf-secret cf-dry-run deploy-staging deploy-prod
```
```diff
 check: ## Type-check Workers for wasm32
-	cargo check --workspace --exclude create-user --target wasm32-unknown-unknown
+	cargo check --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown
 
 test: ## Run all tests
 	cargo test --workspace
 
+# D1-level tests: real SQL on isolated local D1 state under target/tmp/d1 (never the dev DB, never remote).
+# Needs Node.js and wrangler (npx). Tests are #[ignore]d in `make test`; one thread keeps memory low.
+# Plan p2 adds the prerequisite line `test-d1: test-d1-read` (client SQL on the same harness) without editing this recipe.
+test-d1: ## Run D1-level SQL tests on local D1 via wrangler (A2, scenario 27, client SQL)
+	cargo test -p seed-build --test d1 -- --ignored --test-threads=1
+
 build-admin: ## Build the admin Worker bundle
```

### File: `../.github/workflows/backend-ci.yml`, exact diffs
```diff
       - name: Cargo check (wasm32, workers only)
-        run: cargo check --workspace --exclude create-user --target wasm32-unknown-unknown --locked
+        run: cargo check --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked
```
```diff
-      - name: Clippy (host, all targets incl. tests and create-user)
+      - name: Clippy (host, all targets incl. tests, create-user, seed-build and d1-local)
         run: cargo clippy --workspace --all-targets --locked -- -D warnings
 
       - name: Clippy (wasm32, workers only)
-        run: cargo clippy --workspace --exclude create-user --target wasm32-unknown-unknown --locked -- -D warnings
+        run: cargo clippy --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked -- -D warnings
+
+  # Real SQL on local D1 (B3). Separate job so the Rust job stays Node-free. No secrets; wrangler pinned.
+  d1-tests:
+    runs-on: ubuntu-latest
+    timeout-minutes: 20
+    steps:
+      - uses: actions/checkout@v4
+
+      - uses: dtolnay/rust-toolchain@stable
+
+      - uses: Swatinem/rust-cache@v2
+        with:
+          workspaces: backend
+
+      - uses: actions/setup-node@v4
+        with:
+          node-version: 22
+
+      - name: Install wrangler (pinned; local D1 only)
+        working-directory: backend/admin
+        run: npm install --no-save wrangler@4.168.0
+
+      - name: D1-level tests (make test-d1)
+        env:
+          WRANGLER_SEND_METRICS: "false"
+        run: make test-d1
```
The new job sits at the same indentation as `check-and-test:` under `jobs:`. The workflow-level `defaults.run.working-directory: backend` and `permissions: contents: read` apply to it. Ask the owner to mark `d1-tests` as a required check in branch protection (an owner step, listed in `00_OVERVIEW.md` section 6).

## 3. Tests (written first)

### File: `tools/seed-build/tests/d1/main.rs` (new)
```rust
//! D1-level tests (`make test-d1`): real SQL on isolated local D1 state through wrangler, using
//! the `d1-local` harness crate. Every test is `#[ignore]`d, so plain `cargo test` needs no Node.js.

mod harness;
```
Later tasks add one `mod` line each and keep the list alphabetical (`rustfmt` sorts it): task 03 adds `mod schema;` and task 04 adds `mod seed;`. The `client` crate's own SQL test (plan p2, task 07) is a separate test binary that uses the same `d1-local` crate; it does not add a module here.

### File: `tools/seed-build/tests/d1/harness.rs` (new)
```rust
//! Self-tests of the D1 harness: isolation, error capture, foreign keys, file rollback.

use d1_local::D1Local;
use std::fs;

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn statements_run_on_an_isolated_database() {
    let db = D1Local::fresh("harness_statements");
    let sets = db
        .exec("CREATE TABLE t (k TEXT PRIMARY KEY NOT NULL); INSERT INTO t (k) VALUES ('a;\nb'); SELECT k FROM t;")
        .expect("statements succeed");
    assert_eq!(sets.len(), 3);
    assert_eq!(sets[2][0]["k"], "a;\nb");
    db.assert_rejected("INSERT INTO t (k) VALUES ('a;\nb')", "UNIQUE constraint failed");
    assert_eq!(db.count("t"), 1);
    let missing = db
        .exec("SELECT * FROM no_such_table")
        .expect_err("an unknown table is an error");
    assert!(missing.contains("no such table"), "{missing}");
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn foreign_keys_are_on_without_a_pragma() {
    let db = D1Local::fresh("harness_foreign_keys");
    assert_eq!(db.query("PRAGMA foreign_keys")[0]["foreign_keys"], 1);
    db.exec(
        "CREATE TABLE p (k TEXT PRIMARY KEY NOT NULL); \
         CREATE TABLE c (id TEXT PRIMARY KEY NOT NULL, k TEXT REFERENCES p (k));",
    )
    .expect("schema");
    db.assert_rejected(
        "INSERT INTO c (id, k) VALUES ('1', 'missing')",
        "FOREIGN KEY constraint failed",
    );
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn a_failing_file_rolls_back_every_statement() {
    let db = D1Local::fresh("harness_file_rollback");
    db.exec(
        "CREATE TABLE p (k TEXT PRIMARY KEY NOT NULL); \
         CREATE TABLE c (id TEXT PRIMARY KEY NOT NULL, k TEXT REFERENCES p (k));",
    )
    .expect("schema");
    let before = db.snapshot(&["p", "c"]);
    let file = db.state_dir().join("rollback.sql");
    fs::write(
        &file,
        "INSERT INTO p (k) VALUES ('a');\nINSERT INTO c (id, k) VALUES ('1', 'missing');\n",
    )
    .expect("write SQL file");
    let err = db
        .exec_file(&file)
        .expect_err("the second statement violates the foreign key");
    assert!(err.contains("FOREIGN KEY constraint failed"), "{err}");
    assert_eq!(db.snapshot(&["p", "c"]), before);
}

#[test]
#[ignore = "needs npx wrangler and local D1; run with make test-d1"]
fn repository_migrations_apply_to_a_fresh_database() {
    let db = D1Local::migrated("harness_migrations");
    assert_eq!(db.count("users"), 0);
}
```

## 4. Red (run before writing `tools/d1-local/src/lib.rs`)
```bash
cargo test -p seed-build --test d1 --no-run
```
Expected: FAIL while loading the workspace, with an error that mentions `d1-local` (it has no `src/lib.rs` yet). Nothing is compiled.

## 5. Implementation

### File: `tools/d1-local/src/lib.rs` (new)
```rust
//! Runs SQL on isolated local D1 state through `npx wrangler d1 execute --local`.
//!
//! Test-only helper shared by `tools/seed-build/tests/d1/` and `client/tests/d1_master_data_sql.rs`.
//! Each `D1Local` owns a `--persist-to` directory `backend/target/tmp/d1/<name>`. The directory is
//! wiped when the value is created, so tests never touch the developer's `admin/.wrangler/state`.
//! Wrangler runs from `admin/` because the `DB` binding and `migrations_dir` are defined in
//! `admin/wrangler.toml`. Every call passes `--local`; an argument list with `--remote` panics.

use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// One result row: column name to value. D1 returns integers, text and `null`; booleans are 0/1.
pub type Row = Map<String, Value>;

/// An isolated local D1 database for one test.
pub struct D1Local {
    persist_dir: PathBuf,
}

/// The `backend/` workspace root (this crate lives in `backend/tools/d1-local`).
pub fn backend_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

impl D1Local {
    /// An empty database in `backend/target/tmp/d1/<name>`. State left from an earlier run is
    /// removed. `name` is letters, digits and underscores, and must be unique per test.
    pub fn fresh(name: &str) -> Self {
        assert!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "D1 test names are letters, digits and underscores, got `{name}`"
        );
        let persist_dir = backend_dir().join("target").join("tmp").join("d1").join(name);
        if persist_dir.exists() {
            fs::remove_dir_all(&persist_dir).expect("remove previous D1 test state");
        }
        fs::create_dir_all(&persist_dir).expect("create D1 test state directory");
        Self { persist_dir }
    }

    /// A fresh database with every file in `migrations/` applied.
    pub fn migrated(name: &str) -> Self {
        let db = Self::fresh(name);
        db.migrate();
        db
    }

    /// The `--persist-to` directory of this database (useful for scratch files such as SQL).
    pub fn state_dir(&self) -> &Path {
        &self.persist_dir
    }

    /// Applies `migrations/` through `wrangler d1 migrations apply DB --local`.
    pub fn migrate(&self) {
        let out = self.wrangler(&["d1", "migrations", "apply", "DB", "--local"]);
        assert!(
            out.status.success(),
            "wrangler d1 migrations apply failed\n{}",
            describe(&out)
        );
    }

    /// Runs one or more statements. `Ok` holds one row list per statement; `Err` holds the
    /// error text D1 reported (for example `FOREIGN KEY constraint failed: SQLITE_CONSTRAINT ...`).
    pub fn exec(&self, sql: &str) -> Result<Vec<Vec<Row>>, String> {
        parse_execute(&self.wrangler(&["d1", "execute", "DB", "--local", "--json", "--command", sql]))
    }

    /// Runs a SQL file the way `make seed-local` does. A failing statement rolls back the file.
    pub fn exec_file(&self, path: &Path) -> Result<Vec<Vec<Row>>, String> {
        let path = path.to_str().expect("SQL file path is UTF-8");
        parse_execute(&self.wrangler(&["d1", "execute", "DB", "--local", "--json", "--file", path]))
    }

    /// Runs exactly one statement that must succeed and returns its rows.
    pub fn query(&self, sql: &str) -> Vec<Row> {
        let mut sets = self
            .exec(sql)
            .unwrap_or_else(|e| panic!("query failed: {e}\nSQL: {sql}"));
        assert_eq!(sets.len(), 1, "query() takes exactly one statement: {sql}");
        sets.remove(0)
    }

    /// `SELECT COUNT(*)` of one table.
    pub fn count(&self, table: &str) -> i64 {
        let rows = self.query(&format!("SELECT COUNT(*) AS n FROM {table}"));
        rows[0]["n"].as_i64().expect("COUNT(*) returns an integer")
    }

    /// Every row of each table, ordered by rowid, in one wrangler call. Compare two snapshots to
    /// prove that a rejected statement left the tables unchanged.
    pub fn snapshot(&self, tables: &[&str]) -> Vec<Vec<Row>> {
        let sql: Vec<String> = tables
            .iter()
            .map(|table| format!("SELECT * FROM {table} ORDER BY rowid;"))
            .collect();
        self.exec(&sql.join(" "))
            .unwrap_or_else(|e| panic!("snapshot failed: {e}"))
    }

    /// Asserts that `sql` fails and that the error text contains `needle`, for example
    /// `FOREIGN KEY constraint failed`, `UNIQUE constraint failed`, `CHECK constraint failed`
    /// or `NOT NULL constraint failed`.
    pub fn assert_rejected(&self, sql: &str, needle: &str) {
        match self.exec(sql) {
            Ok(_) => panic!("expected an error containing `{needle}`, but it succeeded: {sql}"),
            Err(e) => assert!(
                e.contains(needle),
                "expected an error containing `{needle}`, got `{e}` for: {sql}"
            ),
        }
    }

    fn wrangler(&self, args: &[&str]) -> Output {
        assert!(
            !args.contains(&"--remote"),
            "D1 tests never run against a remote database"
        );
        Command::new("npx")
            .arg("wrangler")
            .args(args)
            .arg("--persist-to")
            .arg(&self.persist_dir)
            .current_dir(backend_dir().join("admin"))
            .env("WRANGLER_SEND_METRICS", "false")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| {
                panic!(
                    "cannot run `npx wrangler` ({e}); make test-d1 needs Node.js and wrangler \
                     (cd admin && npm install --no-save wrangler@4.168.0)"
                )
            })
    }
}

fn describe(out: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Parses `wrangler d1 execute --json` output: an array of `{"results": [...]}` on success, or
/// `{"error": {"text": ..., "notes": [{"text": ...}]}}` (exit status 1) on failure.
fn parse_execute(out: &Output) -> Result<Vec<Vec<Row>>, String> {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let start = stdout
        .find(['[', '{'])
        .ok_or_else(|| format!("no JSON in wrangler output\n{}", describe(out)))?;
    let value: Value = serde_json::from_str(&stdout[start..])
        .map_err(|e| format!("unparseable wrangler output ({e})\n{}", describe(out)))?;
    if let Some(error) = value.get("error") {
        return Err(error_text(error));
    }
    if !out.status.success() {
        return Err(format!("wrangler failed\n{}", describe(out)));
    }
    let Value::Array(sets) = value else {
        return Err(format!("unexpected wrangler output\n{}", describe(out)));
    };
    let mut result = Vec::with_capacity(sets.len());
    for set in &sets {
        let rows = set
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("result set without `results`: {set}"))?;
        let mut parsed = Vec::with_capacity(rows.len());
        for row in rows {
            let row = row
                .as_object()
                .cloned()
                .ok_or_else(|| format!("row is not an object: {row}"))?;
            parsed.push(row);
        }
        result.push(parsed);
    }
    Ok(result)
}

fn error_text(error: &Value) -> String {
    let mut parts: Vec<String> = error
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .into_iter()
        .collect();
    if let Some(notes) = error.get("notes").and_then(Value::as_array) {
        parts.extend(
            notes
                .iter()
                .filter_map(|note| note.get("text").and_then(Value::as_str))
                .map(str::to_owned),
        );
    }
    if parts.is_empty() {
        error.to_string()
    } else {
        parts.join(" | ")
    }
}
```

## 6. Green + Standards Check
```bash
cargo test -p seed-build
make test-d1
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked -- -D warnings
make check
git diff --stat Cargo.lock
```
Expected:
- `cargo test -p seed-build`: the `d1` binary reports `test result: ok. 0 passed; 0 failed; 4 ignored`.
- `make test-d1`: `running 4 tests`, each `... ok`, then `test result: ok. 4 passed; 0 failed; 0 ignored` (about 15 to 25 s).
- `cargo fmt --all` may reflow lines; that is formatting only. `--check` then prints nothing. Both clippy runs and `make check` finish with no warnings.
- `Cargo.lock` shows the added `seed-build` and `d1-local` packages (commit it). The first cargo run of this task adds them; the `--locked` runs above come after it.

If `make test-d1` fails with `cannot run npx wrangler`, install Node.js 22 and run `cd admin && npm install --no-save wrangler@4.168.0`. Do not change the tests.
