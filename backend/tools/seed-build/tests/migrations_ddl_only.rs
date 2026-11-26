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
    let sql =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
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
    assert_eq!(
        ddl.matches(" REFERENCES ").count(),
        10,
        "expected 10 foreign keys"
    );
}
