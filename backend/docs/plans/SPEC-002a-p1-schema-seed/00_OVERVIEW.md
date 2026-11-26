# Plan 00: Overview & Task Manifest (`SPEC-002a-p1-schema-seed`)

- **Spec**: [SPEC-002a](../../specs/SPEC-002a-master-data-schema-seed-public-read.md) (normative data rules: [SPEC-002 sections 4.1 to 4.4](../../specs/SPEC-002-master-data-to-backend.md))
- **Estimate / Shared Contract**: [SPEC-002a-ESTIMATE.md](../SPEC-002a-ESTIMATE.md)
- **Sibling plan**: `SPEC-002a-p2-public-read` (the `client` read path; not planned here)
- **Created**: 2026-11-25
- **Status**: IN_PROGRESS (aligned by the Plan Auditor, see `../SPEC-002a-PLAN-AUDIT.md`; execution order across both plans is in section 2 of that file)

## 1. Objective & Architecture Impact
This plan delivers the database half of SPEC-002a:
- **Migration** `migrations/0002_master_data.sql`. DDL only: 12 tables, 10 foreign keys with no delete or update action, nullable `category`, `NOT NULL` on every column whose contract field is required and non-nullable (all `daily_meta` columns; all `daily_cards` columns except `summary_theme` and `overview_theme`), CHECKs for the two 0/1 booleans and the single `daily_meta` row, and indexes on the foreign-key columns. Art consistency (D09 item 5) is not enforced in the DDL (spec §6 Schema does not list it); `seed-build` checks it and SPEC-002b validates it in code (PROTO-ART-FK). It is applied by the existing `make migrate-*` targets through `admin`'s D1 binding.
- **Seed source** `seed/*.json`. A one-off stripped copy of the frontend data, produced by a deterministic conversion (task 01) that is run once and not committed (D07). The four files have the exact shape of the four public responses in `docs/api/client/openapi.yaml` v0.2.1, so p2 can compare its responses with them.
- **Seed build crate** `tools/seed-build` (workspace member; `serde` and `serde_json` only). It validates the seed (dropped tokens, unknown fields, control characters, cross-file references, derived fields) and writes `seed/build/seed.sql` (gitignored): 1,669 statements of the form `INSERT ... ON CONFLICT(<pk>) DO UPDATE SET`, parents first, with no `DELETE`, no `INSERT OR REPLACE` and no `audit_events`.
- **Makefile** targets `seed-build`, `seed-local`, `seed-staging` [CLOUD], `seed-prod` [CLOUD] and `test-d1`. `check` and the CI wasm32 steps exclude `seed-build` and `d1-local`.
- **D1 test harness** (B3). A dev-only workspace crate `tools/d1-local` (`D1Local`, only `serde_json`) drives `npx wrangler d1 execute --local --persist-to target/tmp/d1/<test>`. Rust integration tests in `tools/seed-build/tests/d1/` use it; so does p2's `client/tests/d1_master_data_sql.rs` (the Worker's own SQL constants through the production parse code). They are `#[ignore]`d in `cargo test`, run by `make test-d1` (p2 adds the prerequisite `test-d1-read`), and run by a new CI job `d1-tests`. Scenarios A2, 27 and 28 are covered here.
- **Local D1 state rule** (all plans). `make migrate-local`, `make seed-local` and p2's `make smoke-local` use wrangler's default developer state `admin/.wrangler/state` (the client Worker reads it with `wrangler dev --persist-to ../admin/.wrangler/state`). Tests never touch it: each `D1Local` owns `backend/target/tmp/d1/<name>`.

The plan touches no `client` or `admin` source code. The two Workers are unchanged.

### Executor environment (every task)
- Run commands from `backend/` unless a task says otherwise. Use the lean env on the 8 GB Mac:
  ```bash
  export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
  ```
- `cargo fmt --all` may reflow the supplied code. That is formatting only and is expected; then `cargo fmt --all --check` must print nothing.
- Never run `[CLOUD]` targets (`migrate-staging`, `migrate-prod`, `seed-staging`, `seed-prod`, `deploy-*`). Local wrangler (`--local`) is allowed.
- Commit `Cargo.lock` with task 02: it gains the `seed-build` entry, and CI uses `--locked`.

## 2. Battleground Summary
| # | Decision | Record | Winner |
| :--- | :--- | :--- | :--- |
| B3 | How A2, scenario 27 and adapter SQL run on a real engine | [02_D1_TEST_HARNESS/debate.md](../../battlegrounds/SPEC-002a-p1-schema-seed/02_D1_TEST_HARNESS/debate.md) | **wrangler `--local`** from Rust integration tests (over `rusqlite` bundled and the system `sqlite3` CLI). It is the only option that tests D1's FK default, D1's error texts and the `wrangler --file` path the seed targets use. It adds no Rust dependency and no C compile. The losing contenders both recommended a wrangler smoke test as a backstop anyway. |
| B4 | Where the one-off conversion lives | [01_SEED_DATA_CONVERSION/debate.md](../../battlegrounds/SPEC-002a-p1-schema-seed/01_SEED_DATA_CONVERSION/debate.md) | **Python script printed in task 01**, run once from a temp dir with `python3 -I`, SHA-256-checked (script, 4 inputs, 4 outputs), not committed. A committed `convert` subcommand would violate D07 and add a TypeScript parser that rots once the frontend JSON is deleted. |

Smaller choices made inside the plan:
- Sort orders are `(index + 1) * 10`, which leaves gaps for SPEC-002b inserts.
- Derived fields (`missing_fields`, `situations.love`) are kept in the seed files so the files equal the responses. They are checked, not stored.
- Love-status texts that are `null` are stored as rows with `text = NULL`, so a re-seed resets them.
- Art consistency (D09 item 5) is checked by `seed-build` but not by DDL (see open question Q1).

## 3. Interface Ledger
> Exact. The Plan Auditor aligns plans from this table.

| Task | Creates / modifies | Exports (name, signature) | Consumes (and where it comes from) |
| :--- | :--- | :--- | :--- |
| `01_SEED_DATA_CONVERSION` | creates `seed/cards.json`, `seed/daily.json`, `seed/card-art.json`, `seed/reading-config.json` | Files. `cards.json` is a JSON array of `ReadingCard`; `daily.json` is `DailyCardsDocument` with `missing_fields`; `card-art.json` is `CardArtManifest`; `reading-config.json` is `ReadingConfig` with `situations.love` and empty `uncategorized_*`. Field names follow openapi v0.2.1. Suggestion ids `love-1` … `life-3`. | `../frontend-client/src/lib/{reading-cards.json,daily-cards.json,reading.ts}`, `../frontend-client/public/cards/v3/manifest.json` (read once, never committed); `docs/specs/SPEC-002-master-data-to-backend.md` section 4.3 tables (assertions) |
| `02_D1_TEST_HARNESS` | creates `tools/d1-local/{Cargo.toml,src/lib.rs}`, `tools/seed-build/Cargo.toml` (dev-dependency `d1-local`), `tools/seed-build/src/lib.rs` (skeleton), `tools/seed-build/tests/d1/{main.rs,harness.rs}`; modifies `Cargo.toml` (members: `tools/seed-build`, `tools/d1-local`), `Makefile` (`check` exclusions, `test-d1`, `.PHONY`), `../.github/workflows/backend-ci.yml` (wasm32 exclusions, job `d1-tests`), `Cargo.lock` | `seed_build::TABLE_ORDER: [&str; 12]`; crate `d1_local::{backend_dir() -> PathBuf, Row = serde_json::Map<String, Value>, D1Local}` with `D1Local::fresh(&str) -> Self`, `migrated(&str) -> Self`, `migrate(&self)`, `state_dir(&self) -> &Path`, `exec(&self, &str) -> Result<Vec<Vec<Row>>, String>`, `exec_file(&self, &Path) -> Result<Vec<Vec<Row>>, String>`, `query(&self, &str) -> Vec<Row>`, `count(&self, &str) -> i64`, `snapshot(&self, &[&str]) -> Vec<Vec<Row>>`, `assert_rejected(&self, sql: &str, needle: &str)` (state dir `backend/target/tmp/d1/<name>`); Make target `test-d1` (recipe: `cargo test -p seed-build --test d1 -- --ignored --test-threads=1`) | `npx wrangler` 4.168.0, `admin/wrangler.toml` (`DB`, `migrations_dir`) |
| `03_MIGRATION_0002` | creates `migrations/0002_master_data.sql`, `tools/seed-build/tests/migrations_ddl_only.rs`, `tools/seed-build/tests/d1/schema.rs` (4 D1 tests, incl. NOT NULL on every `daily_meta` and `daily_cards` column except `summary_theme`, `overview_theme`); modifies `tests/d1/main.rs` (`mod schema;`) | Tables and columns exactly as Shared Contract 3.1, plus NOT NULL on all non-nullable columns (all `daily_meta`; all `daily_cards` except `summary_theme`, `overview_theme`), CHECKs (`daily_meta.id = 1`, both booleans in 0/1), and indexes `idx_situations_category`, `idx_suggestions_category`, `idx_card_meanings_love_status`, `idx_card_details_love_status`, `idx_daily_love_texts_love_status` | `TABLE_ORDER` (`seed_build`), `d1_local::D1Local` (task 02) |
| `04_SEED_BUILD_TOOL` | replaces `tools/seed-build/src/lib.rs`; creates `src/{model.rs,validate.rs,sql.rs,main.rs}`, `tests/{seed_files.rs,seed_validation.rs}`, `tests/d1/seed.rs`; modifies `tests/d1/main.rs` (`mod seed;`) | `seed_build::{TABLE_ORDER, SEED_FILES: [&str; 4], FORBIDDEN_TOKENS: [&str; 14], SeedSources { reading_config, cards, card_art, daily: String }, SeedSources::read_dir(&Path) -> Result<Self, String>, build_statements(&SeedSources) -> Result<Vec<String>, String>, render_sql(&[String]) -> String, build_sql(&SeedSources) -> Result<String, String>}`; binary `seed-build --seed-dir=<dir> --out=<file>` (exit 1 and stale output removed on error); test-only private helpers `seeded(&str) -> D1Local`, `load_seed(&D1Local)` in `tests/d1/seed.rs` | `seed/*.json` (task 01), migration (task 03), harness (task 02) |
| `05_MAKEFILE_SEED_TARGETS` | modifies `Makefile` | Make targets `seed-build`, `seed-local`, `seed-staging` [CLOUD], `seed-prod` [CLOUD]; output `seed/build/seed.sql` | `seed-build` binary (task 04) |
| `06_DOCUMENTATION` | creates `docs/knowledges/features/master-data.md` (schema, seed, how to use, testing; its "Public read API" section links to p2's `master-data-public-read.md`), `docs/knowledges/code-standards.md` (sole creator); modifies `docs/deployment.md` (rows, step 5b, `d1-tests` bullet, "Local D1 state" paragraph), `docs/admin/architectures/database.md` | none | all tasks |

## 3b. Cross-Plan Assumptions
What p2 can rely on from this plan, and what this plan assumes about p2:
1. **Golden values.** p2 loads `seed/cards.json`, `seed/daily.json`, `seed/card-art.json` and `seed/reading-config.json` with `include_str!` (from `client/tests/`, the path is `../../seed/<file>`). Each file equals the expected body of the matching GET on a freshly seeded D1, including `missing_fields`, `situations.love` and the empty `uncategorized_*` lists. Love-status map key order inside the files follows the source order: daily order in `daily.json`, reading order in `cards.json`. JSON object comparison should be order-insensitive (`serde_json::Value`).
2. **Schema.** Names are exactly Shared Contract 3.1. Additions p2 may read but need not depend on:
   - `daily_meta` columns are all NOT NULL;
   - `card_art` has no sort column, so p2 orders it by joining `cards.sort_order` (the contract says the same);
   - sort orders are `10, 20, 30, ...`;
   - NULL love texts are stored as rows with `text = NULL`. Absent rows are still possible after SPEC-002b creates a love status, so p2 must handle both.
3. **D1 harness for p2's adapter SQL.** p2 does not add tests to `tools/seed-build`. Its test `client/tests/d1_master_data_sql.rs` lives in the `client` crate (it needs `backend_client`'s SQL constants and parse code) and uses the dev-only crate `tools/d1-local` as a dev-dependency of `backend-client` (p2 task 07 adds the line to `client/Cargo.toml`).
   - `d1_local::D1Local` (task 02) is the only wrangler driver. p2 calls `D1Local::migrated("client_master_data_sql")`, `exec_file` with `seed/build/seed.sql`, `query` for each SQL constant, and `exec` for the A1 fixture. Rows come back as `serde_json::Map` with integers, text and `null`; booleans are 0/1.
   - `wrangler d1 execute --command` cannot bind parameters, so adapter queries must be parameter-free (they are: whole-table `SELECT json_object(...)`).
   - `tools/seed-build` has no dependency on `client`.
4. **CI.** `make test-d1` runs in the job `d1-tests` (Node 22, `wrangler@4.168.0`). p2 task 07 adds the recipe-less line `test-d1: test-d1-read`, so the job also runs the client SQL test; p2 needs no CI change.
5. **Smoke and local state.** p2's `smoke-local` calls `make migrate-local` and `make seed-local` (task 05) as-is. Both use wrangler's default state `admin/.wrangler/state` (no `--persist-to`); p2 starts the client Worker with `--persist-to ../admin/.wrangler/state`. Tests use `target/tmp/d1/<name>` only.
6. **Assumption about p2.** p2 does not edit `migrations/`, `seed/`, `tools/seed-build/` or the Makefile seed targets. If p2 needs a schema change, it goes through the owner (Shared Contract rule). Shared files are edited in this order: `Makefile` p1 02, p1 05, p2 07, p2 08; `Cargo.toml` (workspace) p1 02 only; `client/Cargo.toml` p2 07 only; `.gitignore` nobody (existing `build/` and `target/` lines cover `seed/build/` and `target/tmp/`); `backend-ci.yml` p1 02 only; `docs/deployment.md` p1 06, then p2 09 (append); `docs/knowledges/code-standards.md` p1 06 creates, p2 09 appends; `docs/knowledges/features/master-data.md` p1 06 only.

**Contract notes for the Plan Auditor** (no silent divergence):
- Shared Contract 3.3 lists `test-d1` as "CI runs it". This plan runs it in a separate job `d1-tests` rather than inside `check-and-test`, so the Rust job stays Node-free.
- Shared Contract 3.1 says "tests that use plain SQLite must turn [foreign keys] ON explicitly". Not applicable: local D1 has `PRAGMA foreign_keys = 1` by default (probed; asserted by `harness::foreign_keys_are_on_without_a_pragma`).
- Shared Contract 3.4 names `tools/seed-build/tests/` as the provisional home of the D1-level tests. Final homes (Plan Auditor decision): the harness is the crate `tools/d1-local`; the schema and seed tests (A2, 27) are in `tools/seed-build/tests/d1/` (one test binary `d1`); the adapter-SQL test is `client/tests/d1_master_data_sql.rs` (p2).
- The seed files carry the derived fields `missing_fields` and `situations.love` (verified, not stored). This is an addition to Shared Contract 3.2. It makes "golden test values" a direct comparison.
- Remote `wrangler d1 execute --file` with 1,669 statements is not testable by executors. The local `--file` path is proven atomic (a failing statement rolls back the file), and the order is parents first. The staging load is an owner `[CLOUD]` step (section 6).

## 3c. Proto-Spec Candidates
- **PROTO-SEED-LIMITS**: let `seed-build` apply the SPEC-002 section 4.5 limits (lengths, key and card-id regexes, `art` pattern, `sha256` format) to seed data. *Why not now:* section 4.5 belongs to SPEC-002b; the same rules should be shared with the admin validators once they exist.
- **PROTO-ART-FK**: enforce D09 item 5 (art entry `art` equals card `art`) in the database with a composite FK `card_art (card_id, art) -> cards (id, art)` and a unique index on `cards (id, art)`. *Why not now:* SPEC-002a's database-enforced list (spec §6 Schema) does not include it; `seed-build` checks it and SPEC-002b validates it in code; a composite FK changes how SPEC-002b must order or defer card art updates.
- **PROTO-NPM-LOCK**: pin CI's wrangler through a committed `package-lock.json` and `npm ci` (and optionally `cargo deny`). *Why not now:* the deploy workflow already uses an unlocked `npm install`; changing both is a CI hardening task, not SPEC-002a.

## 4. Task Manifest
> **Execution Policy**: Each task below MUST be executed by a dedicated, fresh **Executor / Coder** agent. Before execution, each agent must read this `00_OVERVIEW.md` file first. Strictly 1 agent per 1 task.
> Each line ends with the executor tier: `[model/effort]`. Tasks run in order; each is read together with this file.
- [x] `01_SEED_DATA_CONVERSION.md`: run the supplied conversion once, check SHA-256 values, commit `seed/*.json` `[haiku/low]`
- [x] `02_D1_TEST_HARNESS.md`: `d1-local` harness crate (`D1Local`), `seed-build` crate skeleton, workspace and CI wiring, `make test-d1`, CI job `d1-tests` `[sonnet/medium]`
- [x] `03_MIGRATION_0002.md`: `migrations/0002_master_data.sql`; scenario 28 (DDL only) and scenario A2 plus the schema rules on D1 `[sonnet/medium]`
- [ ] `04_SEED_BUILD_TOOL.md`: seed validation and SQL rendering, CLI, seed hygiene tests, seed load equality and scenario 27 on D1 `[sonnet/medium]`
- [ ] `05_MAKEFILE_SEED_TARGETS.md`: `seed-build`, `seed-local`, `seed-staging` [CLOUD], `seed-prod` [CLOUD]; local load twice `[haiku/low]`
- [ ] `06_DOCUMENTATION.md`: `docs/knowledges/features/master-data.md` (schema and seed), `docs/knowledges/code-standards.md`, `docs/deployment.md`, `docs/admin/architectures/database.md` `[sonnet/medium]`

Acceptance coverage in this plan:
- **Scenario 28**: `tests/migrations_ddl_only.rs`.
- **A2**: `tests/d1/schema.rs::scenario_a2_rejects_dangling_references_and_duplicate_keys`, plus `every_reference_key_check_and_required_column_is_enforced` and `daily_columns_are_not_null_except_the_two_theme_variants`.
- **Scenario 27**: `tests/d1/seed.rs::scenario_27_reseed_restores_seeded_rows_and_keeps_other_rows`.
- **Seed fails on a dropped field before D1**: `tests/seed_validation.rs`, the `main.rs` stale-output test, and the task 05 negative run.
- **No `chatgpt.com`, `source_url` or `raw_file` in `seed/`**: `tests/seed_files.rs`.
- **Seed counts 78, 78, 74, 4, 9+6, 12, 3, 6 and 22 null `complicated`**: `tests/seed_files.rs` and `tests/d1/seed.rs`.
- **Scenarios 1 to 6 and A1**: p2 (A1 also on real SQL in p2 task 07).

## 5. Owner steps (not executor steps)
1. After merge: `make migrate-staging`, then `make seed-staging` ([CLOUD]; confirms). This proves the remote `--file` load of 1,669 statements. If remote D1 rejects the file size or the statement count, report it; the fallback is splitting `seed.sql` per table (a small plan change).
2. Mark the CI job `d1-tests` as a required status check on `main`.

## 6. Questions resolved by the owner and orchestrator (2026-11-25), no longer open
- **Q1 (art consistency in the database).** Not enforced in the DDL; checked by `seed-build`, validated in code by SPEC-002b. Logged as PROTO-ART-FK.
- **Q2 (Python once).** Accepted: `python3` is used once in task 01 and nothing Python is committed.
- **Q3 (sort-order gaps).** Accepted: sort orders `10, 20, 30, ...`.
