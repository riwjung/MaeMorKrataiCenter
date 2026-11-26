---
model: sonnet
effort: medium
reason: "Prose documentation from facts listed below; no code"
---

# Task 06: Documentation and architecture update (schema and seed)

- **Plan Overview**: [00_OVERVIEW.md](./00_OVERVIEW.md)
- **Target Knowledge Files**:
  - `docs/knowledges/features/master-data.md` (new; the only creator is this task; plan p2's documentation task never edits it and writes its own file `master-data-public-read.md`);
  - `docs/knowledges/code-standards.md` (new; the only creator is this task; p2's documentation task appends one section on top);
  - `docs/deployment.md` (edit; p2's documentation task appends one section after this task's edits).
- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before executing.
  2. Execute ONLY this documentation task as a fresh agent. When complete, hand off to Verifier.
  3. Follow `.gb-sdlc/skills/deslop/SKILL.md`: plain sentences, no em dashes, sentence-case headings, active voice. Write in English. State only facts from the built code and this plan; read the files named below to confirm each fact before you write it.
  4. If a command or check in this task fails (build, lint, format, script or hash mismatch), that is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence a lint with `#[allow]`, never improvise a fix.
  5. Plan p1 runs before plan p2 (global execution order in `SPEC-002a-PLAN-AUDIT.md`), so none of these files exists yet except `docs/deployment.md`.

## 1. Objective
Record how the master-data schema and seed work, how to run them, and which standards the plan added, so a later agent can change them without reading this plan.

## 2. Feature documentation: `docs/knowledges/features/master-data.md`
Create the file with this structure.

1. `# Master data` with one paragraph: what master data is (cards, daily content, art sizes, reading taxonomy), that it lives in the shared D1 database, and that `client` serves it read-only (SPEC-002a) while `admin` will edit it (SPEC-002b).
2. `## Schema` (this plan). Source: `migrations/0002_master_data.sql`.
   - A table listing all 12 tables in `TABLE_ORDER` order with primary key, foreign keys and nullable columns.
   - Rules: foreign keys take no delete or update action (D09); D1 enforces them per statement; `category` NULL means uncategorized (D10); love situations are not stored but derived from `love_statuses` (key and `reading_label`); `missing_fields` is not stored but computed from NULL `daily_love_texts.text` or absent rows; booleans are INTEGER 0/1 with CHECK constraints; `daily_meta` has exactly one row (`id = 1`); there are two love-status orders, `daily_sort_order` and `reading_sort_order`.
   - Art consistency (D09 item 5) is checked by the seed build and, later, by SPEC-002b validation. The schema does not enforce it.
3. `## Seed` (this plan).
   - The four files and the response each mirrors (`cards.json` is `GET /v1/cards`, and so on).
   - Derived fields in the files (`missing_fields`, `situations.love`) are checked, not stored.
   - `seed/` is a one-off stripped copy (D07). It was produced by the conversion script in `docs/plan-records/SPEC-002a-p1-schema-seed/01_SEED_DATA_CONVERSION.md`, or in `docs/plans/...` until the plan is archived. Never regenerate `seed/` from `../frontend-client`; a new conversion is a new plan.
   - What `seed-build` rejects: forbidden tokens (list them from `FORBIDDEN_TOKENS`), unknown fields, control characters, and each cross-file rule in `validate.rs`.
   - What it emits: one upsert per row, `ON CONFLICT DO UPDATE` of every non-key column, sort orders `10, 20, 30, ...`, suggestion IDs `love-1` … `life-3`, 1,669 statements, no `DELETE`, no `INSERT OR REPLACE`, no `audit_events`.
   - Re-seeding returns seeded rows to the seed values (admin edits to seeded rows are lost) and leaves other rows, such as admin-created suggestions with UUIDv4 IDs, untouched.
4. `## How to use` (this plan): `make migrate-local && make seed-local`; `make seed-build` alone; `make seed-staging` / `make seed-prod` are `[CLOUD]`, ask for confirmation, and need the matching `migrate-*` first. To change seed data, edit `seed/*.json`, run `cargo test -p seed-build`, then `make test-d1`.
5. `## Testing`. Describe `make test-d1`:
   - It runs `#[ignore]`d Rust tests with `--ignored --test-threads=1`: the schema and seed tests in `tools/seed-build/tests/d1/`, and (once plan p2 is merged) the `client` crate's `client/tests/d1_master_data_sql.rs`, which runs the Worker's own SQL constants and parse code. State that `make test-d1` reaches the second one through the prerequisite line `test-d1: test-d1-read`.
   - Both suites use the dev-only crate `tools/d1-local` (`D1Local`). Each test owns `backend/target/tmp/d1/<name>`, passed to `npx wrangler d1 execute --local --persist-to` run from `admin/`; `--remote` is refused. This state is separate from the developer's `admin/.wrangler/state`, which `make migrate-local`, `make seed-local` and `make smoke-local` use.
   - The `D1Local` API: `fresh`, `migrated`, `migrate`, `state_dir`, `exec`, `exec_file`, `query`, `count`, `snapshot`, `assert_rejected`. Test names are letters, digits and underscores and unique per test.
   - How a new test is added to `seed-build`: a file plus one `mod` line in `tools/seed-build/tests/d1/main.rs`.
   - CI job `d1-tests` (Node 22, `wrangler@4.168.0`).
   - Link the B3 debate.
6. `## Public read API`: one line, `See [master-data-public-read.md](master-data-public-read.md) (written by plan SPEC-002a-p2-public-read).`
7. `## Linked decisions`: D05, D07, D08, D09, D10, D11, D12, D13, SPEC-002 sections 4.1 to 4.4, SPEC-002a.

## 3. `docs/deployment.md`
- Add these rows to the "Command guideline" table:
  - `| Load master data (seed) | make seed-staging / make seed-prod (make seed-local for dev; make seed-build to only build) |`
  - `| D1-level SQL tests | make test-d1 (needs Node.js and npx wrangler) |`
- Add a short paragraph "Local D1 state": `make migrate-local`, `make seed-local` and (plan p2) `make smoke-local` use wrangler's default state `admin/.wrangler/state`, and the client Worker reads it only when started with `--persist-to ../admin/.wrangler/state`; `make test-d1` never touches it (it uses `target/tmp/d1/<name>`).
- Add step 5b to "One-time setup per environment" after `make migrate-<env>`: `make seed-<env>` (confirms first; re-running is safe, but it overwrites admin edits to seeded rows).
- In "GitHub Actions", add one bullet for the `d1-tests` job: separate job, Node 22, pinned `wrangler@4.168.0`, local D1 only, no secrets; the owner should mark it as a required check.
- Note that `seed/build/` is generated and ignored by git.

## 4. `docs/knowledges/code-standards.md` (new, from `.gb-sdlc/templates/code-standards.md`)
Fill the four template sections with what exists today.
- **Stack and toolchain.** Rust (stable in CI, 1.100 locally), `worker` 0.8.7, axum 0.8, edition 2021. One check command set:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`;
  - `cargo clippy --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked -- -D warnings`;
  - `make check`;
  - `cargo test --workspace --locked`.
  All are blocking in CI. Add the lean local env line from `00_OVERVIEW.md` for the 8 GB Mac.
- **Test harness.** Host tests use in-memory adapters (`MemoryClientStore`, `tower::ServiceExt::oneshot`). SQL tests use `make test-d1` through the `tools/d1-local` crate (see `features/master-data.md`).
- **Conventions.**
  - Host-only tools live in `tools/<name>` and are excluded from the wasm32 commands (`create-user`, `seed-build`, `d1-local`).
  - CLI arguments use the `--key=value` form, as in `create-user` and `seed-build`.
  - Tool errors are `Result<_, String>` printed with the tool name, followed by exit code 1.
  - Never use `#[allow]` or other lint suppression in production code, and never to get past a new lint; a lint is fixed in the code. The one documented exception is a module-level `#![allow(dead_code)]` with a reason comment in test-support modules that several test binaries of one crate share (`client/tests/support/`, `client/tests/seed_support/`, plan p2).
  - Migrations are DDL only; `migrations_ddl_only.rs` enforces this.
  - Seed data changes go through `seed/*.json` and `seed-build`.
- **Learned rules (from this plan).**
  - Write `format!` arguments inline (`{name}`); clippy's `uninlined_format_args` rejects positional identifiers.
  - Test helpers shared by test binaries of different crates live in a dev-only crate (`tools/d1-local`). Helpers used by several modules of one test binary live in that binary (`tests/d1/main.rs` plus modules), so no `dead_code` allowance is needed.
  - D1 tests are `#[ignore = "needs npx wrangler and local D1; run with make test-d1"]`.
  - The seed upsert is always `ON CONFLICT(<pk>) DO UPDATE`, never `INSERT OR REPLACE`. REPLACE deletes the row first, which fails on foreign keys and loses rows that are not in the seed.

## 5. Architecture update
There is no `docs/architectures/` folder at the backend root yet (only `docs/admin/architectures/`); plan p2's documentation task creates it. Do not create it here. Add one paragraph to `docs/admin/architectures/database.md` listing the 12 master-data tables from migration 0002 with a link to `docs/knowledges/features/master-data.md`. No new library: `seed-build` and `d1-local` use `serde` and `serde_json`, which the workspace already uses.

## 6. Verification
```bash
grep -n "seed-" docs/deployment.md
grep -c "" docs/knowledges/features/master-data.md docs/knowledges/code-standards.md
grep -n -- "—" docs/knowledges/features/master-data.md docs/knowledges/code-standards.md docs/deployment.md
```
Expected: the seed rows and step are present, both new files exist, and the last command prints nothing (no em dashes).
