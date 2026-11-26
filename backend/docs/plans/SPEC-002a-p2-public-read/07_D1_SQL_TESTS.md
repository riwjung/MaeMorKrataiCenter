---
model: sonnet
effort: medium
reason: "Complete test and Makefile code supplied; needs local wrangler and p1's seed build."
---

# Task 07: Adapter SQL against real SQLite (`make test-d1-read`, part of `make test-d1`)

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

**Preconditions:** p1 is merged: `migrations/0002_master_data.sql`, `seed/*.json`, `make seed-build` (writes `seed/build/seed.sql`), the `test-d1` target and the `tools/d1-local` crate (`D1Local`) exist. Task 06 is done (`client/tests/seed_support/mod.rs`). Node.js and `npx wrangler` work (`cd admin && npx wrangler --version`).

**Design decision (Plan Auditor).** This test lives in the `client` crate (`client/tests/`), because it must run the Worker's own SQL constants and parse code from `backend_client`. It does not duplicate the wrangler driver: it uses `D1Local` from the shared dev-only crate `tools/d1-local` (created by p1 task 02) as a dev-dependency of `backend-client`. `tools/seed-build` has no dependency on `client`, so p1's `tests/d1/` holds no adapter tests and p2 adds no module there.

All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Modify `client/Cargo.toml`: dev-dependency `d1-local`; commit the resulting `Cargo.lock` change (CI uses `--locked`)
- [ ] Create `client/tests/d1_master_data_sql.rs` (one `#[ignore]` test)
- [ ] Modify `Makefile`: target `test-d1-read`, extra prerequisite line `test-d1: test-d1-read`, `.PHONY` line
- [ ] Red by mutation, revert, green, standards check

How it works (battleground `docs/battlegrounds/SPEC-002a-p2-public-read/07-d1-sql-tests/debate.md`):
- `D1Local::migrated("client_master_data_sql")` creates a throwaway local D1 in `backend/target/tmp/d1/client_master_data_sql` (`--persist-to`). It never touches `admin/.wrangler/state` (the developer database used by `make migrate-local`, `seed-local` and `smoke-local`) or any remote database. `D1Local` refuses `--remote`.
- The test applies the real migrations (`admin/wrangler.toml`, `migrations_dir = "../migrations"`, through `D1Local::migrated`) and the real `seed/build/seed.sql` (`D1Local::exec_file`, the same `--file` path `make seed-local` uses), then runs **each SQL constant from `client/src/adapters/d1_master_data.rs`** with `D1Local::query` (`wrangler d1 execute DB --local --json --command <sql>`). The constants take no bind parameters, which is what lets `--command` run them verbatim.
- The `row_json` strings that wrangler returns go through the **same** `*_from_sets` parse functions and `assemble_*` functions the Worker uses, and the result is compared with the seed by the task 06 golden assertions. The JS/wasm boundary is the only production step not covered; `smoke-local` (task 08) covers it.
- Scenario A1 runs as direct SQL (`seed_support::A1_SQL`) on the same database at the end.
- It is `#[ignore]`, so `cargo test --workspace` lists it as ignored (visible, not silent) and does not need Node. `make test-d1-read` runs it with `--ignored`. The extra prerequisite line `test-d1: test-d1-read` makes p1's `make test-d1` run it without editing p1's recipe.

`D1Local::query` returns each result row as a `serde_json::Map`; `row_sets` below asserts that every row has a TEXT `row_json` column and fails with the offending row if not.

## 2. Harness
Uses `client/tests/seed_support/mod.rs` (task 06) and the `d1-local` crate (p1 task 02). One addition: the dev-dependency.

### File: `client/Cargo.toml` (exact diff)
```diff
 [dev-dependencies]
+d1-local = { path = "../tools/d1-local" }
 rand_core = { version = "0.6", features = ["getrandom"] }
```
The first `cargo test -p backend-client --test d1_master_data_sql --no-run` updates `Cargo.lock` (the `backend-client` entry gains `d1-local`). Commit it. A dev-dependency is not built by the wasm32 `cargo check`/`clippy` steps, so the Worker bundle is unaffected.

## 3. Tests

### File: `client/tests/d1_master_data_sql.rs` (new, full content)
```rust
//! Runs the client's master-data SQL on a real local D1 (SQLite) with the real migrations and
//! seed, and feeds the rows through the same parse and assembly code the Worker uses. Needs
//! Node.js (`npx wrangler`) and `make seed-build`; run it with `make test-d1-read`.

mod seed_support;

use backend_client::adapters::d1_master_data::{
    card_art_from_sets, daily_from_sets, reading_cards_from_sets, reading_config_from_sets,
    CARD_ART_BATCH, DAILY_CARDS_BATCH, READING_CARDS_BATCH, READING_CONFIG_BATCH,
};
use backend_client::master_data::{
    assemble_card_art, assemble_daily_cards, assemble_reading_cards, assemble_reading_config,
};
use d1_local::{backend_dir, D1Local};
use seed_support::{
    assert_a1_reading_config, assert_art_consistent, assert_card_art_matches_seed,
    assert_cards_match_seed, assert_clean, assert_daily_matches_seed,
    assert_reading_config_matches_seed, A1_SQL,
};

/// The `row_json` strings of each statement, as `D1Database::batch` would return them.
fn row_sets(db: &D1Local, statements: &[&str]) -> Vec<Vec<String>> {
    statements
        .iter()
        .map(|&sql| {
            db.query(sql)
                .iter()
                .map(|row| {
                    row["row_json"]
                        .as_str()
                        .unwrap_or_else(|| panic!("row without TEXT row_json: {row:?}"))
                        .to_string()
                })
                .collect()
        })
        .collect()
}

fn seeded_database() -> D1Local {
    let seed_sql = backend_dir().join("seed").join("build").join("seed.sql");
    assert!(
        seed_sql.exists(),
        "{} is missing; run `make seed-build` (make test-d1-read does)",
        seed_sql.display()
    );
    let db = D1Local::migrated("client_master_data_sql");
    db.exec_file(&seed_sql)
        .unwrap_or_else(|e| panic!("seed load failed: {e}"));
    db
}

#[test]
#[ignore = "needs Node.js and a seed build; run with `make test-d1-read`"]
fn d1_master_data_sql_on_real_sqlite_matches_the_seed() {
    let db = seeded_database();

    let cards = assemble_reading_cards(
        reading_cards_from_sets(row_sets(&db, READING_CARDS_BATCH)).expect("cards rows parse"),
    );
    let cards = serde_json::to_value(&cards).unwrap();
    assert_cards_match_seed(&cards);

    let daily = assemble_daily_cards(
        daily_from_sets(row_sets(&db, DAILY_CARDS_BATCH)).expect("daily rows parse"),
    );
    let daily_text = serde_json::to_string(&daily).unwrap();
    assert_daily_matches_seed(&serde_json::from_str(&daily_text).unwrap());
    assert_clean("daily (D1)", &daily_text);

    let art = assemble_card_art(
        card_art_from_sets(row_sets(&db, CARD_ART_BATCH)).expect("art rows parse"),
    );
    let art = serde_json::to_value(&art).unwrap();
    assert_card_art_matches_seed(&art);
    assert_art_consistent(&cards, &art);

    let config = assemble_reading_config(
        reading_config_from_sets(row_sets(&db, READING_CONFIG_BATCH)).expect("config rows parse"),
    );
    assert_reading_config_matches_seed(&serde_json::to_value(&config).unwrap());

    // Scenario A1 as direct SQL on the same database (FKs are enforced by D1).
    db.exec(A1_SQL).expect("the A1 fixture applies");
    let config = assemble_reading_config(
        reading_config_from_sets(row_sets(&db, READING_CONFIG_BATCH)).expect("config rows parse"),
    );
    assert_a1_reading_config(&serde_json::to_value(&config).unwrap());
}
```

## 4. Red (by mutation; the SQL already exists from task 02)
1. Build the seed and run the test once:
   ```bash
   make seed-build
   cargo test -p backend-client --test d1_master_data_sql -- --ignored --test-threads=1
   ```
   Expected: 1 passed (about 30 to 90 s). If it fails on the wrangler output shape, on a missing table, or on a column name, that is a real mismatch between p1's DDL and the Shared Contract or this plan. Report it with the message and do not edit the SQL to match a different column name.
2. Mutation. In `client/src/adapters/d1_master_data.rs`, in `DAILY_CARDS_SQL`, change `'approved_for_web', d.approved_for_web` to `'approved_for_web', 'false'`. Run the command again. Expected: FAIL with `master-data row decode error: invalid type: string "false", expected i64` (the `expect("daily rows parse")` message). Revert the change.
3. Confirm `cargo test -p backend-client --test d1_master_data_sql` (without `--ignored`) reports `1 ignored`.

## 5. Implementation

### File: `Makefile` (exact diffs, applied on top of p1 tasks 02 and 05)
1. The `.PHONY` block at the top now ends with the line `	cf-whoami cf-bootstrap cf-secret cf-dry-run deploy-staging deploy-prod`. Directly after that line add a new line:
```make
.PHONY: test-d1-read smoke-local
```
2. Directly after p1's `test-d1` target (the two lines `test-d1: ## Run D1-level SQL tests ...` and `	cargo test -p seed-build --test d1 -- --ignored --test-threads=1`), add:
```make

# Runs the client master-data SQL (client/src/adapters/d1_master_data.rs) through the production
# parse code on an isolated local D1 (target/tmp/d1/client_master_data_sql, via the d1-local
# crate) with the real migrations and seed. Needs Node.js (npx wrangler).
test-d1-read: seed-build ## Run the client master-data SQL against a real local D1 (SQLite)
	cargo test -p backend-client --test d1_master_data_sql -- --ignored --test-threads=1

# p1 owns the test-d1 recipe; this recipe-less line only adds the client SQL check as a prerequisite.
test-d1: test-d1-read
```
Recipe lines start with a tab character, not spaces. `smoke-local` is declared phony here and defined in task 08.

## 6. Green + Standards Check
```bash
make test-d1-read
make test-d1
cargo test -p backend-client
cargo fmt --all
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --stat Cargo.lock
```
Expected: `make test-d1-read` prints `test d1_master_data_sql_on_real_sqlite_matches_the_seed ... ok` and `1 passed`; `make test-d1` runs it first, then p1's `seed-build` D1 tests, and all pass; `cargo test -p backend-client` shows `1 ignored` for this file and everything else passing; fmt and clippy are clean; `Cargo.lock` differs only by `d1-local` in the `backend-client` dependency list (commit it).
