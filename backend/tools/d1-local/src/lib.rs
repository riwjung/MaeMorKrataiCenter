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
        let persist_dir = backend_dir()
            .join("target")
            .join("tmp")
            .join("d1")
            .join(name);
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
        parse_execute(&self.wrangler(&[
            "d1",
            "execute",
            "DB",
            "--local",
            "--json",
            "--command",
            sql,
        ]))
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
