---
model: sonnet
effort: medium
reason: "Documentation from a fixed outline; no code."
---

# Task 09: Documentation and architecture update

- **Plan Overview**: [00_OVERVIEW.md](./00_OVERVIEW.md)
- **Target Knowledge File**: `docs/knowledges/features/master-data-public-read.md`
- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before executing.
  2. Execute ONLY this documentation task as a fresh agent. When complete, hand off to Verifier.
  3. Write in English. Follow `.gb-sdlc/skills/deslop/SKILL.md`: no em dashes, sentence-case headings, plain verbs, no filler. State what the system does and how to verify it.
  4. If a command or check in this task fails (build, lint, format, script or hash mismatch), that is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence a lint with `#[allow]`, never improvise a fix.

Read before writing: tasks 01 to 08 of this plan, `docs/decisions/D05.md`, `D09.md`, `D10.md`, `D11.md`, `D12.md`, `D13.md`, `../docs/api/client/openapi.yaml`, and the code as built in `client/src/master_data.rs`, `client/src/adapters/d1_master_data.rs`, `client/src/routes/master_data.rs`.

Do not edit `HANDOFF.md`, `TODO.md`, any spec, decision, or the contract. Do not edit `docs/knowledges/features/master-data.md` (p1 task 06 created it and already links to your file). Ownership of the shared files: p1 task 06 created `docs/knowledges/code-standards.md` and edited `docs/deployment.md`; you add one section at the end of each (plus the two table rows named in section 3) and change nothing else.

## 1. Objective
Permanent documentation for the `client` master-data read path, plus the patterns this plan established.

## 2. Feature documentation: `docs/knowledges/features/master-data-public-read.md` (new)
Create `docs/knowledges/features/` if missing. Sections, in this order:
1. **Overview.** Four public, read-only, cached GETs on `client` serve master data from D1 (SPEC-002a). Include a request flow list: Workers Cache (hit returns without running the Worker) → `security_pipeline_middleware` (request id, CORS, rate limit, body cap) → router → handler → `MasterDataPort` → D1 batch → `*_from_sets` parse → `assemble_*` → JSON with cache headers.
2. **Endpoints.** A table with path, response schema name in `openapi.yaml`, D1 statements in the batch (from `d1_master_data.rs`), approximate body size (255 KB, 359 KB, 18 KB, 4 KB), and rows read per miss.
3. **Headers and errors.**
   - `200`: `Cache-Control: public, max-age=600` and `Vary: Origin`, also on requests without `Origin`, with the reason (Workers Cache honors `Vary`).
   - Every error carries the envelope and `Cache-Control: no-store`: `503` D1, `500` row decode, `429`, `404`, `405`.
   - `405` behavior on every known path including `/health`, with `Allow: GET,HEAD`. Unknown paths return `404` for any method. `OPTIONS` is handled by the middleware.
   - A write method with a disallowed `Origin` gets `403` before routing.
4. **Data rules.** Where each rule is implemented:
   - sort by `sort_order`, then key/id, in Rust
   - null fill for every current love-status key
   - `missing_fields` computed from `NULL` or absent daily texts, in daily order
   - `situations.love` derived from love statuses
   - uncategorized lists (regular first, then love-derived while no `love` category exists)
   - 0/1 → boolean via `sql_bool`
   - `approved_for_web` served regardless of value (D09 item 6)
5. **Read-only guarantee.** `MasterDataPort` has no write method; every SQL constant is a single `SELECT` (enforced by the unit test `every_statement_is_a_single_read_only_select`).
6. **Workers Cache (D13).**
   - Config blocks in `client/wrangler.toml` for top level, staging and prod.
   - No purge, and the cache key includes the Worker version.
   - What a hit skips (rate limit, request id, logs).
   - The staging verification checklist, copied from `00_OVERVIEW.md` §5.
7. **Testing.** Commands and what each covers:
   - `cargo test -p backend-client`: unit tests plus `spec_002a_scenarios`, `spec_002a_seed_scenarios`
   - `make test-d1-read` (part of `make test-d1`; `client/tests/d1_master_data_sql.rs` on the shared `tools/d1-local` harness)
   - `make smoke-local`
   - `make check` and the two clippy commands
   - A note that host tests use the memory adapter and that `test-d1-read` plus `smoke-local` exist for D1-only and wasm-only bugs.
8. **CPU budget.**
   - The B1 outcome and the reasoning in short.
   - How to measure CPU on staging.
   - The fallback (proto-spec P2-PS1).
   - Link the debate record.
9. **Linked decisions.** D05, D07, D08, D09, D10, D11, D12, D13, D04 (`docs/client/decisions/D04.md`).

## 3. `docs/deployment.md`
Append a section `## Client master-data read: local checks and staging verification` at the very end of the file (after the last existing section; do not touch p1's seed rows, step 5b, `d1-tests` bullet or "Local D1 state" paragraph). Also add two rows to the "Command guideline" table directly after p1's `D1-level SQL tests` row: `| Client SQL on real local D1 | make test-d1-read (also run by make test-d1) |` and `| Local Worker smoke test | make smoke-local (migrates and seeds admin/.wrangler/state, runs client with wrangler dev, curls the GETs) |`. The new section holds:
- `make test-d1-read` and `make smoke-local`: what they do and their preconditions (Node.js, `worker-build`, p1's seed targets). Say that `test-d1-read` uses an isolated `target/tmp/d1/client_master_data_sql` state through the `d1-local` crate, while `smoke-local` uses the developer state `admin/.wrangler/state`.
- the staging verification checklist from `00_OVERVIEW.md` §5, marked as owner-approved `[CLOUD]` steps (deploy, seed load) followed by read-only checks

## 4. `docs/knowledges/code-standards.md`
The file exists (p1 task 06 created it). Keep its content and append, at the end, a section `## Patterns from SPEC-002a (client read path)` with these rules:
- A port returns unsorted typed rows; ordering and response shaping live in one pure module (`client/src/master_data.rs`) shared by the D1 and memory adapters.
- D1 statements are static `SELECT`s returning one `row_json` TEXT column built with `json_object`, parsed with serde_json into typed rows; booleans use `sql_bool`; `"key"` is quoted.
- Every D1 call in an `async_trait` method goes through one `SendFuture::new(async move { ... })` block.
- Every adapter SQL constant is exercised against real local D1 (SQLite) in `make test-d1` through the production parse and assembly functions, by `client/tests/d1_master_data_sql.rs`, which uses the shared `tools/d1-local` harness. `tools/seed-build` does not depend on `client`.
- Public cacheable responses set `Cache-Control` and `Vary: Origin` in the handler and only on `200`; errors use `ClientError::into_response_with_request_id` (`no-store`).
- Test helpers shared by several files under `client/tests/` live in a `support`-style module with a module-level `#![allow(dead_code)]` and a comment giving the reason (each test file is its own crate). This is the documented exception to the no-`#[allow]` rule in the Conventions section above.
No new lint rule: clippy has no rule for these patterns.

## 5. Architecture update
There is no `docs/architectures/` folder. Create `docs/architectures/client-worker.md` with:
- the `client` module map (`ports.rs`, `adapters/{worker_env,d1_master_data,memory}.rs`, `master_data.rs`, `routes/{health,master_data}.rs`, `middleware/security.rs`, `error.rs`)
- the Cloudflare features it uses (D1 binding `DB` read-only for master data, `PUBLIC_ASSETS` R2 for `/health`, `[[ratelimits]]`, Workers Cache since D13)
- the routes table

No new library and no new environment variable were added.

## 6. Verification
```bash
git status --short docs
```
Expected: only `docs/knowledges/features/master-data-public-read.md`, `docs/deployment.md`, `docs/knowledges/code-standards.md` and `docs/architectures/client-worker.md` changed or added by this task. `git diff docs/deployment.md docs/knowledges/code-standards.md` shows additions only (no removed lines).
