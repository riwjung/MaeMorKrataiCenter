# Battleground Debate Record

- **Plan**: `SPEC-002a-p2-public-read`
- **Task**: `07-d1-sql-tests` (p2's part of estimate battleground B3; p1 owns the harness for A2 and scenario 27)
- **Date**: 2026-11-25
- **Status**: RESOLVED

## 1. The Conflict / Decision Point
The D1 adapter's SQL (`client/src/adapters/d1_master_data.rs`) must run against real SQLite with the real migration and seed, because host tests use the memory adapter and cannot catch D1-only bugs (0/1 booleans, `NULL`, JSON functions, column names). The Shared Contract says p2 "adds its adapter queries" to `make test-d1` but does not fix the harness shape. The planner wrote all three cases.

## 2. Competing Options

### Option A: An ignored Rust integration test that drives `wrangler d1 execute --local`
`client/tests/d1_master_data_sql.rs`, `#[ignore]`, run by `make test-d1-read` (wired as a prerequisite of p1's `test-d1`). It applies migrations and `seed/build/seed.sql` to a throwaway persist dir (`target/test-d1-read`), runs every SQL constant through `npx wrangler d1 execute --json`, then feeds the returned `row_json` strings to the same `*_from_sets` parse functions and `assemble_*` functions the Worker uses, and compares the result with `seed/*.json`. It also runs scenario A1 as direct SQL.
- **Security (#1)**: Uses local state only and never `--remote`. Tests the exact production SQL constants, so drift between SQL keys and structs is caught.
- **Technical Debt (#2)**: No new dependency. Reuses the production parse and assembly code and the seed comparison helpers from task 06. `#[ignore]` shows up as "ignored" in `cargo test` output, so it is not a silent skip, and `make test-d1` runs it explicitly.
- **Speed (#3)**: About 12 wrangler calls, about 30 to 60 s. Needs Node.js (already needed for wrangler).

### Option B: A Node or bash script comparing `wrangler --json` output with expected JSON
- **Security (#1)**: Same local-only property.
- **Technical Debt (#2)**: Re-implements null fill, ordering and boolean mapping in JavaScript to build the expected output, or only checks counts. Either duplicates logic or tests less, and the Rust parse path is not exercised.
- **Speed (#3)**: Fast to write, weaker result.

### Option C: `rusqlite` (bundled) inside `cargo test`
- **Security (#1)**: Neutral.
- **Technical Debt (#2)**: New dev-dependency compiling SQLite C sources on the 8 GB Mac, and it is not the D1/workerd SQLite build. The estimate argues against it.
- **Speed (#3)**: Fast tests, slow first build.

## 3. Outcome & Winning Rationale
- **Winner**: Option A.
- **Justification**: Tied with B on security. It wins on debt because the production SQL, parse and assembly code all run against the real schema and seed with no duplicated logic and no new dependency. The 30 to 60 s runtime is acceptable for a `make test-d1` target that CI runs separately from `cargo test`.
