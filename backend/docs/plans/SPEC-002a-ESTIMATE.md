# Plan Estimate: SPEC-002a Master Data: Schema, Seed and Public Read API

- **Spec**: [SPEC-002a-master-data-schema-seed-public-read.md](../specs/SPEC-002a-master-data-schema-seed-public-read.md)
- **Created**: 2026-11-25
- **Recommended planner tier**: opus/medium (spec complexity is `complex`; owner confirms)

## 1. Sizing
- Gate: spec is `APPROVED` and `spectre_audit: PASSED`.
- No bootstrap plan: `cargo fmt`, `cargo clippy -D warnings` (host and wasm32) and `cargo test` with in-memory adapters already run in `backend-ci.yml`. There is no `docs/architectures/` folder.
- Deliverables found:
  1. **Migration** `migrations/0002_master_data.sql`, DDL only: 13 tables (see Shared Contract), FKs without cascade, unique keys, nullable `category`.
  2. **Seed source** `seed/*.json` (English keys, dropped fields already removed, owned by the backend) and a **seed build tool** that validates the seed (fails on any dropped field), then emits one upsert statement per row (about 2,000 statements) as SQL.
  3. **Makefile seed targets** (`seed-build`, `seed-local`, `seed-staging` [CLOUD], `seed-prod` [CLOUD]) and a `test-d1` target.
  4. **`client` read path**: a master-data port, a D1 adapter (real SQL) and a memory adapter; assembly of four responses; four `GET` routes with `Cache-Control: public, max-age=600`; a method-not-allowed fallback (`405` with `Allow`, envelope, `no-store`) for every known route including `/health`; `503` on D1 failure and `no-store` on every error.
  5. **Tests**: host tests for scenarios 1-6, A1 and 28 with the memory adapter; D1-level tests for A2 and 27 against real SQLite semantics; a local `wrangler dev` plus curl smoke script.
  6. **Docs**: `docs/deployment.md` seed section, HANDOFF, conformance check against `openapi.yaml` v0.2.1.
- Token model (about 4 chars per token): context reads (spec 12 KB, D05-D12 about 20 KB, parent §4 about 14 KB, contract about 25 KB, existing client code about 40 KB, seed shape probes) cost about 35-45k tokens per planner. Full code and tests are then written inline into task files.
  - p1 writes about 150 lines of DDL, a seed tool of about 600 lines with tests, a SQL test harness of about 300 lines, Makefile and docs: about 70-85k of output.
  - p2 writes about 1,500 lines (port, D1 adapter with 6 to 10 queries, memory adapter, assembly, routes, fallback, 12+ tests, curl script): about 110-130k of output.
  - One combined plan would be about 250-290k, near the 300k limit. Two plans fit.
- Token budget: target ~200k per session, over at 300k.

## 2. Plans
| # | Plan name | Scope (what it owns) | Est. planner tokens | Est. tasks | Depends on (execution) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | `SPEC-002a-p1-schema-seed` | `migrations/0002_master_data.sql`; `seed/*.json` (copy, strip, English keys, stable IDs `love-1`..`life-3`); seed build tool (new workspace crate `tools/seed-build`, modeled on `tools/create-user`); Makefile `seed-*` and `test-d1` targets; D1-level tests: A2 (FK, unique rejections), 27 (re-seed keeps the extra suggestion, no `audit_events`), 28 (DDL only), seed fails on a dropped field; `docs/deployment.md` seed section. | ~120k | 8 (overview, 6 work, documentation) | none |
| 2 | `SPEC-002a-p2-public-read` | `client` code: `MasterDataPort` in `ports.rs`, D1 adapter and memory adapter, response assembly (`master_data.rs`), the four GET routes, cache header handling, Workers Cache enabled in `client/wrangler.toml` (D13), `405`/`404` fallback for all routes (D12); host tests for scenarios 1-6 and A1; wasm32 check; local smoke script with curl against local D1; contract conformance; HANDOFF and docs. | ~170k | 11 (overview, 9 work, documentation) | p1 (execution only for the last task, local D1 run and curl; code and host tests use the memory adapter) |

Planning of both plans runs in parallel (the Shared Contract below makes that safe). Execution order: p1, then p2. p2 tasks that need only the memory adapter can start once the contract is frozen; its final task (local D1 plus curl verification) needs p1 merged.

**Alternative cut** (if the owner prefers three smaller plans): p1a schema + seed, p1b D1-level test harness, p2 read path. Not recommended: the harness choice (risk 4) is small and belongs with the schema.

## 3. Shared Contract
> Definitions only. Column and table names are law for both planners. p1 owns the DDL and may add columns or indexes that no query in p2 depends on; renaming anything below needs the owner. All `*_sort_order` columns are `INTEGER NOT NULL`. Text is stored verbatim.

### 3.1 Tables and columns (owner p1, read by p2)
| Item | Path | Name / shape | Owner plan | Used by |
| :--- | :--- | :--- | :--- | :--- |
| Migration file | `migrations/0002_master_data.sql` | One migration; DDL only; `CREATE TABLE IF NOT EXISTS` like 0001; no `INSERT`; applied by `make migrate-local` / `migrate-staging` | p1 | p2 (verification), SPEC-002b |
| `love_statuses` | migration | PK `key` TEXT; `daily_label`, `reading_label` TEXT NOT NULL; `daily_sort_order`; `reading_sort_order` | p1 | p2 |
| `categories` | migration | PK `key` TEXT; `label`, `icon`, `hint` TEXT NOT NULL; `sort_order` | p1 | p2 |
| `situations` | migration | PK `key` TEXT; `category` TEXT NULL FK `categories(key)`, no cascade; `label` TEXT NOT NULL; `sort_order` (love situations are not stored here; derived from `love_statuses`) | p1 | p2 |
| `suggestions` | migration | PK `id` TEXT (`love-1`..`life-3` seed, UUIDv4 admin); `category` TEXT NULL FK `categories(key)`; `text` TEXT NOT NULL; `sort_order` | p1 | p2 |
| `positions` | migration | PK `key` TEXT (`past`, `present`, `future`); `label` TEXT NOT NULL; `sort_order` | p1 | p2 |
| `cards` | migration | PK `id` TEXT; `name`, `keyword` TEXT NOT NULL; `art` TEXT NULL; `sort_order` (reading-cards.json order) | p1 | p2 |
| `card_meanings` | migration | PK (`card_id`, `love_status`); `card_id` FK `cards(id)`; `love_status` FK `love_statuses(key)`; `text` TEXT NULL; no cascade | p1 | p2 |
| `card_details` | migration | same shape as `card_meanings` | p1 | p2 |
| `card_art` | migration | PK `card_id` FK `cards(id)`; `art` TEXT NOT NULL; `width`, `height`, `rendered_width`, `rendered_height`, `bytes` INTEGER NOT NULL; `sha256` TEXT NOT NULL; response order follows `cards.sort_order` (equals manifest order) | p1 | p2 |
| `daily_meta` | migration | single row, PK `id` INTEGER CHECK (`id` = 1); `schema_version` INTEGER; `version`, `updated`, `status`, `scope` TEXT; `auto_fallback_to_general_or_spread` INTEGER (0/1) | p1 | p2 |
| `daily_cards` | migration | PK `card_id` FK `cards(id)`; `name`, `arcana`, `reading_mode`, `time_horizon`, `orientation`, `content_status` TEXT; `approved_for_web` INTEGER (0/1); `theme`, `overview`, `work`, `money`, `rabbit_message`, `colors`, `affirmation`, `summary_theme`, `overview_theme` TEXT; `sort_order` (daily-cards.json order) | p1 | p2 |
| `daily_love_texts` | migration | PK (`card_id`, `love_status`); `card_id` FK `daily_cards(card_id)`; `love_status` FK `love_statuses(key)`; `text` TEXT NULL; no cascade | p1 | p2 |
| `missing_fields` | derived, not stored | computed at read time: list of `content.love_by_status.<key>` for each love status whose `daily_love_texts.text` is NULL or whose row is absent, in `daily_sort_order` | p2 | SPEC-002b |
| `PRAGMA foreign_keys` | D1 default ON | tests that use plain SQLite must turn it ON explicitly | p1 | p2 |

### 3.2 Seed (owner p1)
| Item | Path | Name / shape | Owner plan | Used by |
| :--- | :--- | :--- | :--- | :--- |
| Seed source files | `seed/cards.json`, `seed/daily.json`, `seed/card-art.json`, `seed/reading-config.json` | English keys; one file per source; none of `source_url`, `source_message`, `source_line`, `review_status`, `details_status`, `source`, `raw_file`, `sourceFile`, `sourceSha256`, `capturedAt`, `selection`, `duplicates`, `ignored`, `chatgpt.com` anywhere | p1 | p2 (golden values for tests) |
| Seed build crate | `tools/seed-build/` | New workspace member `seed-build`; `serde` and `serde_json` only; `Cargo.toml` `members` gets the entry | p1 | Makefile |
| Seed build output | `seed/build/seed.sql` | Generated, gitignored; one `INSERT ... ON CONFLICT(<pk>) DO UPDATE SET` per row (never `INSERT OR REPLACE`, which deletes first and breaks FKs); parents before children; no write to `audit_events`; no `DELETE` | p1 | Makefile, D1 tests |
| Golden test values | `seed/*.json` | p2 tests load the seed JSON (via `include_str!`) and compare it with the HTTP responses (counts 78, 78, 74, 4 / 9+6 / 12 / 3 / 6; 22 null `complicated`) | p1 | p2 |

### 3.3 Makefile targets and scripts
| Item | Path | Name / shape | Owner plan | Used by |
| :--- | :--- | :--- | :--- | :--- |
| `seed-build` | `Makefile` | Builds `seed/build/seed.sql`; exits non-zero on a dropped field before any D1 call | p1 | p1 |
| `seed-local` | `Makefile` | `seed-build`, then `cd admin && npx wrangler d1 execute DB --local --file ../seed/build/seed.sql` | p1 | p2 |
| `seed-staging`, `seed-prod` | `Makefile` | Marked `[CLOUD]`, confirm first, `--env <env> --remote` | p1 | owner |
| `test-d1` | `Makefile` | Runs D1-level tests (A2, 27, adapter SQL) on a real SQLite engine; CI runs it | p1 | p2 (adds its adapter queries) |
| `smoke-local` | `Makefile` + `scripts/smoke-master-data.sh` | `migrate-local`, `seed-local`, `wrangler dev` for `client`, curl the four GETs, a `POST` (405) and an unknown path (404); asserts status, headers, counts | p2 | verifier |

### 3.4 `client` code (owner p2; p1 touches none of it)
| Item | Path | Name / shape | Owner plan | Used by |
| :--- | :--- | :--- | :--- | :--- |
| `MasterDataPort` | `client/src/ports.rs` | async trait, read-only methods only (no write method exists); `ClientPorts.master_data: Arc<dyn MasterDataPort>` | p2 | p2 |
| D1 adapter | `client/src/adapters/worker_env.rs` (or `adapters/d1_master_data.rs`) | implements the port on binding `DB` with `SELECT` only; `SendFuture` / `SendWrapper` pattern as `check_d1` | p2 | p2 |
| Memory adapter | `client/src/adapters/memory.rs` | `MemoryClientStore` gains fixtures loaded from `seed/*.json`, a D1-failure switch and direct fixture edits (A1) | p2 | p2 |
| Response assembly | `client/src/master_data.rs` | pure functions from rows to response structs; `null` fill for every love-status key; `missing_fields`; uncategorized lists and ordering (spec §6) | p2 | p2 |
| Workers Cache config | `client/wrangler.toml` | cache enabled for every env (D13); no purge; owner p2 | p2 | verifier (staging) |
| Routes | `client/src/routes/master_data.rs` | `GET /v1/cards`, `/v1/daily-cards`, `/v1/card-art`, `/v1/reading-config`; 200 sets `Cache-Control: public, max-age=600` | p2 | p2 |
| Method fallback | `client/src/lib.rs`, `routes/health.rs` | known path + other method: `405` + `Allow` + envelope + `no-store`; unknown path: `404` for any method; `OPTIONS` stays in middleware | p2 | p2 |
| Method error | `client/src/error.rs` | `ClientError::method_not_allowed()`, code `METHOD_NOT_ALLOWED` (the contract wins if it names another) | p2 | p2 |
| Response JSON keys | `../docs/api/client/openapi.yaml` v0.2.1 | authoritative for every field name and nullability; any change is made in the contract first | owner | p2 |
| Host tests | `client/tests/spec_002a_scenarios.rs` | scenarios 1-6, A1, 28; in-memory adapters | p2 | verifier |
| D1-level tests | `tools/seed-build/tests/` (p1 decides the final home) | A2, 27 and each adapter query on real SQL | p1 | p2 |

## 4. Notes

### Repo findings
- `client` serves `/health` only. `build_router(ports)` in `lib.rs` wraps every route in `security_pipeline_middleware`, which applies the security headers including `no-store`. The 200 `Cache-Control` override for the four GETs must happen after that layer (or the middleware must allow it per route). Likely bug site in p2.
- axum's default `405` has no envelope; D12 wants a custom fallback for all routes, including `/health`.
- Tests use `MemoryClientStore`, `tower::ServiceExt::oneshot` and a hand-rolled `block_on`. No SQLite exists in any test today.
- `tools/create-user` shows the pattern for a workspace tool that shells out to `npx wrangler d1 execute DB` with `--local` or `--env <env> --remote`.
- Migrations are shared and applied through `admin`'s binding; no wrangler change is needed. `client/tests/spec_001_scenarios.rs` asserts `client/wrangler.toml` has no `migrations_dir`; keep it that way.
- Staging D1 is live with real auth data; `0002` only creates tables.
- `docs/plans/` did not exist; it was created with this file.

### Risks the owner should see before planners run
1. **10 ms CPU (Free plan) against 255 KB and 359 KB bodies.** `GET /v1/cards` reads about 1,014 rows (78 cards + 936 texts), `GET /v1/daily-cards` about 546 rows (78 + 468 love texts). Each row crossing the D1/wasm boundary costs CPU, and serializing 255-359 KB costs more. Time waiting on D1 is not CPU, but this work is. No measurement exists. Options for a battleground: aggregate in SQL (`json_group_object` per card, about 78 rows instead of about 1,000), avoid an intermediate `serde_json::Value`, and a per-isolate memo (risk 2). A CPU measurement must be a required verification task (local `wrangler dev` plus a staging request). If 10 ms is missed, the fallbacks are the Paid plan or pre-rendered JSON blobs (a spec change).
2. **D1 rows read per day.** The Cache API is a no-op on `*.workers.dev` (D05), so every new visitor reaches D1. One visitor loading cards and daily reads about 1,560 rows; the 5M/day limit is hit at about 3,200 such visitors per day, after which every endpoint returns `503` until 00:00 UTC. Browser `max-age=600` helps only repeat visits. The 50-queries-per-invocation limit is not a concern. Options: a per-isolate memo with a short TTL (needs the `Clock` port for tests; staleness becomes up to 10 min + memo TTL, so the owner must accept that), or accept the risk until a custom domain and edge cache exist.
3. **Seed build and load.** Build: a light Rust crate `tools/seed-build` reads `seed/*.json`, rejects dropped or unknown fields, emits `seed.sql`. Load: `wrangler d1 execute --file`; planners must confirm remote `--file` accepts about 2,000 single-row upserts, that FK order (parents first) is respected because D1 enforces FKs per statement, and test `--local` first; the staging load is an owner-approved `[CLOUD]` step. Upsert must be `ON CONFLICT DO UPDATE`, never `INSERT OR REPLACE`; rows not in the seed stay untouched (scenario 27). `seed/` is a stripped one-off copy of the 290-410 KB sources and must never be rebuilt from `../frontend-client` at build time (D07). Where the one-off conversion script lives is a detail for p1.
4. **A2 and scenario 27 against real SQLite (open choice, battleground B3).** `MemoryClientStore` cannot enforce foreign keys. Options: (a) `wrangler d1 execute --local` driven by a script via `make test-d1`: closest to production, no new Rust dependency, slower, needs `npx wrangler` (CI would need `npm ci`); (b) `rusqlite` with `bundled` as a dev-dependency: fast, inside `cargo test`, but compiles SQLite C sources (time and disk on an 8 GB Mac) and is not the D1 binding; (c) the system `sqlite3` CLI called from a Rust test: light, but a silent skip hides regressions. Suggested battleground starting point: (a) for A2, 27 and the adapter SQL, since the verification step needs local D1 anyway. If plain SQLite is used, set `PRAGMA foreign_keys = ON` explicitly.
5. **Host tests miss wasm-only and D1-only bugs** (two reached `main` once). Required: the adapter's real SQL runs in `test-d1`, `make check` (wasm32) passes, and `smoke-local` curls the running Worker against local D1. D1 returns booleans as 0/1 and `NULL` as `null`; the adapter must map `approved_for_web` and `auto_fallback_to_general_or_spread` to booleans, which the memory adapter cannot catch.
6. **Header override.** The middleware forces `no-store`; the four GETs must send `public, max-age=600` on 200 only, keeping `Vary: Origin` and CORS. Test with and without an `Origin` header.
7. **`/health` 405 is a behavior change** (D12). SPEC-001 tests that assume the bare axum `405` may need updating; p2 must list them.
8. **Ordering and null edge cases.** Three orderings (cards, daily cards, love statuses with two sort orders) and two uncategorized lists; A1 mixes `sort_order` and `reading_sort_order`. Use direct SQL fixtures.
9. **Data hygiene.** Add a test that scans `seed/` and every response body for `chatgpt.com`, `source_url`, `raw_file`.
10. **Staging has no R2**, so `/health` is `503` there. The new routes must not depend on `/health`, and the smoke script must not read that `503` as a failure of them.

### Open choices that need a battleground
- **B1**: how the responses are assembled (SQL aggregation with `json_group_object` versus row-by-row in Rust) under the 10 ms CPU limit, including how CPU is measured (risk 1).
- ~~**B2**: per-isolate memo with TTL versus none (risk 2).~~ Dropped: D13 (Workers Cache).
- **B3**: how A2, scenario 27 and the adapter SQL are tested on real SQLite (risk 4).
- Planners may decide inside their plan: flat `master_data.rs` versus a `master_data/` folder, and the exact method-not-allowed error code.

### Owner decisions at the checkpoint (2026-11-25)
- Two-plan cut confirmed; both planners on opus/medium.
- **Risk 2 resolved by D13**: enable Workers Cache on `client` (config only, no purge, D05 TTL unchanged). **B2 (per-isolate memo) is dropped.** p2 owns the config change and a staging verification checklist (cache hit incl. the 359 KB body, `Age` header, errors and `/health` not cached, CPU time of a miss). Staging deploy and seed load stay owner-approved `[CLOUD]` steps; the plan lists them as verification steps, not executor steps.
- Risk 1 (CPU on a miss) and B1 still apply: a miss must fit 10 ms.

### Adjustments the owner may want before planners run (as written by the estimator)
- Confirm the two-plan cut and opus/medium for both planners (`config.yaml` default for planner is sonnet/high; `complex` means opus/medium).
- Confirm a CPU measurement on staging is an acceptable verification step (it needs a staging deploy and seed load, both `[CLOUD]`).
- Confirm whether the isolate memo (risk 2) is allowed, since it extends staleness slightly.
