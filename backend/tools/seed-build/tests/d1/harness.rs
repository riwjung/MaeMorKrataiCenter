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
    db.assert_rejected(
        "INSERT INTO t (k) VALUES ('a;\nb')",
        "UNIQUE constraint failed",
    );
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
