# Plan Audit: SPEC-002a

- **Audited**: 2026-11-26
- **Plans covered**: `SPEC-002a-p1-schema-seed` (overview + tasks 01 to 06), `SPEC-002a-p2-public-read` (overview + tasks 01 to 09)
- **Verdict**: ALIGNED

Scope of the audit: both overviews (Interface Ledgers, Cross-Plan Assumptions, deviations) were read in full. Task files were opened to check or fix a specific mismatch (p1 01, 02, 03, 05, 06 and the test sections of 04; p2 02, 03, 06, 07, 08, 09 and the tails of every task). Nothing was compiled and no cargo command was run (read-only checks, per the rules). Any compile, clippy or rustfmt error found at execution time is a plan defect to be reported as a deviation (workflow 03); every task file now says so (finding 13).

## 1. Findings & Fixes Applied
| # | Type | Where | Problem | Fix applied |
| :--- | :--- | :--- | :--- | :--- |
| 1 | ownership / match | p1 overview CPA-3 vs p2 task 07 | Two homes for the adapter SQL tests: p1 expected `tools/seed-build/tests/d1/adapter_sql.rs` using `D1Local`; p2 planned `client/tests/d1_master_data_sql.rs` with its own private wrangler driver (`target/test-d1-read`). | Resolved (section 5): the harness moves to a new dev-only crate `tools/d1-local`; the SQL test stays in `client/tests/` and uses it. p1 CPA-3 rewritten, `adapter_sql.rs` removed, p2 task 07 rewritten (new test code, `client/Cargo.toml` dev-dependency, Makefile anchors). |
| 2 | ownership | p1 task 02 | The harness (`support.rs`) was test-only inside `seed-build`, so no other crate could reuse it. | Task 02 now creates `tools/d1-local/{Cargo.toml,src/lib.rs}` (same code, `state_dir()` added, state dir fixed to `backend/target/tmp/d1/<name>`, name validated) and wires it as dev-dependency of `seed-build`. `support.rs` removed. Harness self-tests stay in `seed-build/tests/d1/harness.rs`. Ledger rows 02, 03, 04 updated. |
| 3 | ownership | p1 task 04 | Task 04 edited `support.rs` (`seeded`, `seed`), which no longer exists and must not make `d1-local` depend on `seed-build`. | `seeded` and `load_seed` are private helpers at the top of `tests/d1/seed.rs`; call sites updated; main.rs diffs in tasks 02, 03, 04 corrected (`mod support;` gone). |
| 4 | harness / standards | Makefile `check`, CI wasm32 check and clippy, every p2 Green block, p1 06 | The new host-only crate must be excluded from wasm32 like `seed-build`; p2 greens excluded only `create-user`. | `--exclude seed-build --exclude d1-local` added everywhere (p1 02 diffs, p1 06 standards text, all p2 task greens). |
| 5 | match | p1 task 03 (DDL tests) | DDL already had NOT NULL on all `daily_meta` columns and all `daily_cards` columns except `summary_theme` and `overview_theme`, plus the 0/1 CHECKs (owner resolution 2), but no test covered the NOT NULL rule column by column. | Added D1 test `daily_columns_are_not_null_except_the_two_theme_variants` (6 + 15 columns rejected, the two variants accepted, tables unchanged). Expected counts updated (task 03: 8; task 04: 10). p2 CPA-2 marked aligned. |
| 6 | match | p1 overview §6 Q1 | Art consistency (D09 item 5) was an open question. | Closed per owner resolution 4: not enforced in the DDL, checked by `seed-build`, validated by SPEC-002b, logged as PROTO-ART-FK (now `TODO-016`). |
| 7 | match | p1 overview §6 Q2, Q3 | Python used once; sort orders 10, 20, 30. | Closed as accepted (owner resolution 5). |
| 8 | ownership | `docs/knowledges/code-standards.md` | p1 task 06 creates it, p2 task 09 said "create or append". | p1 06 is the sole creator; p2 09 appends one section on top. |
| 9 | ownership | `master-data.md` vs `master-data-public-read.md` | p1 06 left placeholders for p2 to fill in `master-data.md`; p2 09 wrote a different file. | p1 06 is the only author of `master-data.md`; its "Public read API" section links to p2's file. Rule 4 duplicate numbering in p1 06 fixed. |
| 10 | ownership | `docs/deployment.md`, Makefile, `Cargo.toml`, `.gitignore`, CI file | Edit-on-top anchors were stale: p2 07 anchored on a three-line `.PHONY` block and on the `test:` target, both changed by p1 02 and 05; p2 09 said "append" without exact placement. | p2 07 anchors on the final line of the `.PHONY` block and on p1's `test-d1` target; p2 09 specifies two table rows after p1's row plus one section at the end of the file and verifies additions only. Ownership order written in p1 overview CPA-6 (`.gitignore`: nobody, `build/` and `target/` already cover `seed/build/` and `target/tmp/`). |
| 11 | match | local D1 state: `migrate-local`, `seed-local`, `test-d1`, `smoke-local` | p2 used `target/test-d1-read`; p1 used `target/tmp/d1/<name>`; smoke used `admin/.wrangler/state`. | One rule: developer targets (`migrate-local`, `seed-local`, `smoke-local`) use wrangler's default `admin/.wrangler/state` (client Worker started with `--persist-to ../admin/.wrangler/state`); tests only use `backend/target/tmp/d1/<name>`. Stated in both overviews, p1 06 (deployment.md paragraph), p2 07, p2 08. |
| 12 | standards | p1 06 vs p2 03, 06, 09 | p1 06 documents "never `#[allow]`", while p2 test-support modules carry a module-level `#![allow(dead_code)]` (needed because each `tests/*.rs` is its own crate). | Kept as a planned, narrow exception; p1 06 and p2 09 now document it (test-support modules only, with a reason comment); the no-`#[allow]`-to-silence-a-new-error rule stays in every task. Not an open question. |
| 13 | executor-bar | all 15 task files | Only p1 04 told the executor that a compile, clippy or rustfmt error is a plan defect. | Rule 4 added to every task (deviation, workflow 03; never `#[allow]`, never improvise). Frontmatter `model`/`effort`/`reason` present in all 15 files; no executor on opus (haiku: p1 01, p1 05, p2 05; sonnet: the rest). |
| 14 | executor-bar | p2 task 08 | "every line starts with `ok` or `info`" cannot hold (section headings, make and wrangler output). | Expected result reworded: every check line is `ok` or `info`, no `FAIL`, last line `smoke-local: all checks passed`. |
| 15 | coverage | scenario A1 | Spec §7 A1 and §9 item 4 now delete the three `love` suggestions. | Verified: p2 task 06 `A1_SQL` and the memory fixture, p2 task 07 (real SQL) and p2 overview Q2 already match the updated text. p2 Q2 closed. No change needed beyond the Q text. |
| 16 | coverage | scenario 28 (estimate §2 vs §3.4 table typo) | p2 CPA-7 asked for confirmation. | Scenario 28 stays with p1 (`migrations_ddl_only.rs`); p2 CPA-7 marked confirmed. |
| 17 | existence / match | seed shape (p1 01 vs p2 CPA-3), `card_art` order, bind parameters | Checked: each `seed/*.json` equals the matching GET body (`cards.json` array; `daily.json` with `love_statuses` `{key, source_label}`; `card-art.json` `{cards}`; `reading-config.json` with `situations.love`, empty `uncategorized_*`, `love_statuses`). p1 04 D1 test orders `card_art` by `cards.sort_order, id`; p2 SQL selects `c.sort_order` and assembly sorts. No query in either plan uses bind parameters. | None needed (verified). p1 overview CPA-1 and p2 CPA-3 agree. |
| 18 | existence | p2 tasks 02 and 03 against the repo | `worker` 0.8.7 provides `D1Database` (Send), `SendFuture`, `D1Result::results`; the "full content" replacements of `ports.rs`, `adapters/mod.rs` and the diffs to `worker_env.rs` match the current files (additions only); `spec_001_scenarios.rs` builds ports through `into_ports`, so adding `ClientPorts.master_data` does not break it; 0001 holds DDL only, so the scenario 28 test passes on it. | None needed (verified). |
| 19 | harness | p2 task 07 dependency | `client` needed a dev-dependency and a `Cargo.lock` change not owned by any task. | p2 07 owns the `client/Cargo.toml` line and the `Cargo.lock` commit (CI runs `--locked`); wasm32 steps never build dev-dependencies. |
| 20 | executor-bar (completeness) | all p2 task files | Planner overran its budget; truncation risk. | Checked every p2 file: all code fences balanced, the six sections (harness, tests, red, implementation, green, lint) present in each, last lines are complete. No truncation found. |

Findings: 20. Fixes applied: 17 (findings 1 to 14, 15 (Q2 wording only), 16 and 19). Findings 17, 18 and 20 were verified with no change needed.

## 2. Global Execution Order
Strictly one fresh executor per task, in this order (p2 tasks 01 to 05 need only the repo, but one global order keeps one cargo process on the 8 GB Mac and lets p2 06 to 08 rely on all of p1):
1. `SPEC-002a-p1-schema-seed` / `01_SEED_DATA_CONVERSION.md`
2. `SPEC-002a-p1-schema-seed` / `02_D1_TEST_HARNESS.md`
3. `SPEC-002a-p1-schema-seed` / `03_MIGRATION_0002.md`
4. `SPEC-002a-p1-schema-seed` / `04_SEED_BUILD_TOOL.md`
5. `SPEC-002a-p1-schema-seed` / `05_MAKEFILE_SEED_TARGETS.md`
6. `SPEC-002a-p1-schema-seed` / `06_DOCUMENTATION.md`
7. `SPEC-002a-p2-public-read` / `01_MASTER_DATA_ROWS_AND_ASSEMBLY.md`
8. `SPEC-002a-p2-public-read` / `02_D1_MASTER_DATA_ADAPTER.md`
9. `SPEC-002a-p2-public-read` / `03_MASTER_DATA_PORT_AND_MEMORY_ADAPTER.md`
10. `SPEC-002a-p2-public-read` / `04_PUBLIC_ROUTES_405_AND_CACHE_HEADERS.md`
11. `SPEC-002a-p2-public-read` / `05_WORKERS_CACHE_CONFIG.md`
12. `SPEC-002a-p2-public-read` / `06_SEED_GOLDEN_SCENARIOS.md`
13. `SPEC-002a-p2-public-read` / `07_D1_SQL_TESTS.md`
14. `SPEC-002a-p2-public-read` / `08_SMOKE_LOCAL.md`
15. `SPEC-002a-p2-public-read` / `09_DOCUMENTATION.md`

No cycles. Each plan's last task is its documentation task (p1 06, p2 09; the files keep their historical names `06_DOCUMENTATION.md` and `09_DOCUMENTATION.md`).

Shared-file edit order: `Makefile` p1 02, p1 05, p2 07, p2 08; `Cargo.toml` (workspace) p1 02 only; `client/Cargo.toml` and `Cargo.lock` additions p2 07 (p1 02 first adds `seed-build` and `d1-local` to the lock); `backend-ci.yml` p1 02 only; `docs/deployment.md` p1 06 then p2 09; `code-standards.md` p1 06 creates, p2 09 appends; `master-data.md` p1 06 only; `.gitignore` untouched.

## 3. Acceptance Criteria Coverage
| Criterion | Test (plan/task) |
| :--- | :--- |
| Scenario 1 (`GET /v1/cards`, 78 cards in file order, `Cache-Control`) | p2/06 `scenario_1_cards_equal_seed_in_file_order_with_public_cache`; header path p2/04; real SQL p2/07; curl p2/08; seed rows p1/04 `tests/d1/seed.rs` |
| Scenario 2 (`/v1/daily-cards`, 22 null `complicated`, `missing_fields`, no provenance) | p2/06 `scenario_2_daily_cards_equal_seed_with_computed_missing_fields_and_no_provenance`; p2/07; p2/08; p1/04 `seed_files.rs` (no provenance in `seed/`) |
| Scenario 3 (`/v1/card-art`, 74 entries, 4 cards with `art: null`) | p2/06 `scenario_3_card_art_equals_seed_and_cards_without_art_are_null`; p2/07 `assert_art_consistent`; p2/08 |
| Scenario 4 (`/v1/reading-config`, 4 categories, 9 + 6 situations, 12 suggestions, 3 positions, 6 love statuses, empty uncategorized lists, daily order kept) | p2/06 `scenario_4_reading_config_equals_seed_and_daily_keeps_daily_order`; p2/07; p2/08 |
| Scenario 5 (D1 unavailable: 503, envelope, `no-store`) | p2/04 `scenario_5_d1_unavailable_returns_503_no_store_on_every_endpoint` (p2/03 port tests) |
| Scenario 6 (POST to `/v1/cards`: 405, envelope, `no-store`, D1 untouched) | p2/04 `scenario_6_write_methods_on_known_paths_return_405_with_allow_and_never_touch_d1`; p2/08 curl |
| Scenario 27 (re-seed keeps the extra suggestion, no `audit_events`) | p1/04 `tests/d1/seed.rs::scenario_27_reseed_restores_seeded_rows_and_keeps_other_rows` |
| Scenario 28 (migrations are DDL only) | p1/03 `tests/migrations_ddl_only.rs` |
| Scenario A1 (deleted `work` and `love` categories; uncategorized order; love suggestions deleted first) | p2/01 unit tests; p2/06 `scenario_a1_deleted_categories_leave_uncategorized_items_in_order` (memory); p2/07 (`A1_SQL` on real local D1) |
| Scenario A2 (dangling references and duplicate key rejected, tables unchanged) | p1/03 `tests/d1/schema.rs::scenario_a2_rejects_dangling_references_and_duplicate_keys`, `every_reference_key_check_and_required_column_is_enforced`, `daily_columns_are_not_null_except_the_two_theme_variants` |
| Seed fails on a dropped field before D1 | p1/04 `seed_validation.rs`, `main.rs` stale-output test; p1/05 negative run |
| `client` has no write path to master data | p2/02 `every_statement_is_a_single_read_only_select`; p2/03 (port has no write method) |
| 405 on all known paths incl. `/health`, 404 on unknown, errors `no-store` | p2/04, p2/08 |
| D13 Workers Cache config | p2/05 (config test); staging checklist is an owner step (section 4) |

## 4. Owner Steps (not executor tasks)
Executors never run these. Each needs the owner's approval.
1. `make migrate-staging`.
2. `make seed-staging` ([CLOUD], confirms first). Proves the remote `--file` load of 1,669 statements; if remote D1 rejects the size or statement count, the fallback is splitting `seed.sql` per table (small plan change).
3. `make deploy-staging` (admin first, then client). Before it: `make cf-dry-run ENV=staging` (local, proves wrangler accepts `[env.staging.cache]`).
4. Mark the GitHub check `d1-tests` as required on `main`.
5. D13 staging checklist (p2 overview §5): cache hit including the 359 KB `/v1/daily-cards` body and the 255 KB `/v1/cards`; `Age` header; `Vary: Origin` and per-Origin variants; errors (`405`, `404`) and `/health` (503 on staging, no R2) not cached; CPU of a miss (p99 under 10 ms, no error 1102; above about 8 ms the owner chooses between `TODO-018` and the Paid plan).

## 5. Resolution of the adapter-SQL test home (item 6)
Chosen design: the harness is a dev-only crate `tools/d1-local` (`D1Local`, `serde_json` only, `publish = false`, host only, excluded from wasm32 steps). The adapter-SQL test stays in the `client` crate as `client/tests/d1_master_data_sql.rs` and takes `d1-local` as a dev-dependency; `make test-d1-read` is a prerequisite of `make test-d1`.
- (a) The test runs the Worker's exact SQL constants and `*_from_sets` / `assemble_*` code from `backend_client`, so no SQL text is copied or `include_str!`ed.
- (b) One wrangler driver: `seed-build` tests and the client test both call `D1Local`; p2's private driver is deleted.
- (c) `make test-d1` runs p1's `seed-build` D1 tests, then (through `test-d1: test-d1-read`) the client test; the `d1-tests` CI job runs `make test-d1`, so no CI change in p2.
- (d) `tools/seed-build` depends on `d1-local` (dev) and not on `client`. The only new edge is `backend-client` (dev) to `d1-local`, which wasm32 builds never compile.
Rejected: tests in `seed-build` (would need a `client` dependency); `#[path]` file include across crates (brittle `CARGO_MANIFEST_DIR` use); putting `D1Local` in the `seed-build` library (test code in a production crate and `client` coupled to the seed tool).

## 6. Open Questions for the Human
None that changes a spec or a decision. For the record only:
- **D13 Q1 (existing, staging):** if staging hits show no `Age` header, the owner decides whether about 20 minutes worst-case staleness is acceptable or the TTL should be split (a contract change). Answerable only after step 5 of section 4.
- Nothing in either plan has been compiled. The first executor to hit a compile, clippy or rustfmt error reports a deviation; the plans do not pre-approve any `#[allow]` beyond the documented test-support `#![allow(dead_code)]`.
