# Project Handoff & Session State (`HANDOFF.md`)

> **Role & Purpose**: This document preserves cross-session continuity between human developers and autonomous AI agents.
> **Rule for Incoming Agents**: Always read this file at the start of a session to understand current status, what has been completed, in-flight work, and immediate next steps. Update this file before concluding your session.

One handoff for the whole `backend/` workspace (`client` and `admin` Workers). Until 2026-11-24 each service kept its own file; that history is condensed in section 7.

---

## 1. Executive Summary & Progress

- **Project Status**: `IDLE (Verified)` · SPEC-002a verified and deployed to staging 2026-11-26 (admin `7de18622`, client `691103c7`); D13 checks pass; miss CPU above the 10 ms target (owner decision pending, step 1)
- **Last Updated**: 2026-11-26
- **Owner**: the project owner, alone since 2026-11-24 (frontend and backend). The scaffold was built by a friend who has stopped.
- **Active Session Agent**: Developer + Claude Code
- **Progress Snapshot**: Both Workers exist; 98 tests pass (11 more D1 tests behind `make test-d1`) and both build for wasm32 on the owner's Mac (2026-11-26). `admin` has complete auth (SPEC-001). `client` now also serves the four public master-data GETs from D1 (SPEC-002a, local only until the owner deploys). Staging runs on `*.workers.dev` with D1 (no R2 yet) and still holds the pre-SPEC-002a code, schema and no seed; prod is not deployed and there is no custom domain.

### Milestone Checklist
- [x] SPEC-001 infra scaffold, both services, verified and OWASP-audited (2026-11-16)
- [x] `backend-client` + `backend-admin` merged into one `backend/` workspace (2026-11-20)
- [x] Cloudflare deploy prep: Makefile targets, `backend-ci` and manual `backend-deploy` workflows, `docs/deployment.md` (2026-11-20)
- [x] SPEC-002 master data approved, Spectre audit passed in 3 rounds; API contracts v0.2.0 drafted (2026-11-21)
- [x] Phase 0: local toolchain, `make test` / `make check`, rustfmt + clippy clean (CI now blocking), docs updated for solo ownership, one HANDOFF/TODO (2026-11-24, awaiting commit)
- [x] First staging deploy on `*.workers.dev` (2026-11-25): login, `/v1/me`, JWKS and refresh rotation verified live; `/health` 503 until R2 is enabled
- [x] SPEC-002a master data schema, seed and public read API implemented, verified and OWASP-audited (2026-11-26); staging migrate, seed and deploy pending (owner steps below)
- [ ] SPEC-002b admin write, then the frontend reading master data from the API
- [ ] 3+1 reading engine (`TODO-003`)

---

## 2. What We Have in Current (Current State)

- **Working Components**:
  - `admin/`: email + password login (PBKDF2-SHA256 100k + pepper), ES256 access JWT (15 min), HttpOnly refresh cookie (7 days) with rotation and replay detection (family revoke), RBAC `owner > admin > user`, `/v1/me`, owner-only key rotation, `/.well-known/jwks.json`, audit events, CORS/CSRF/rate-limit/body-cap middleware, error envelope.
  - `client/`: security pipeline (request id, CORS, 60 req/min per path+IP, 32 KB body cap), `GET /health`, and four public master-data GETs (`/v1/cards`, `/v1/daily-cards`, `/v1/card-art`, `/v1/reading-config`; contract `../docs/api/client/openapi.yaml` v0.2.1) read from D1 through a read-only `MasterDataPort` and static `SELECT` statements; `Cache-Control: public, max-age=600` only on 200 (D05); `405` with `Allow` on a known path, `404` on an unknown path, errors `no-store` (D12); Workers Cache enabled in `client/wrangler.toml` for dev, staging and prod (D13, no purge). Turnstile guard and JWKS verifier exist but no route uses them yet.
  - `migrations/0001_init_auth.sql`: `users`, `sessions`, `jwks_keys`, `audit_events`. Applied through the admin Worker's D1 binding.
  - `migrations/0002_master_data.sql`: 12 master-data tables (DDL only, foreign keys without cascade, nullable `category`).
  - `seed/*.json` + `tools/seed-build` (validates, renders 1,669 upserts) + `make seed-local` / `seed-staging` / `seed-prod` ([CLOUD], confirm first). `tools/d1-local` is the dev-only local-D1 test harness behind `make test-d1`.
  - `tools/create-user`: creates a user via `wrangler d1 execute` (password prompted, min 12 chars).
- **Architecture & Tech Stack**:
  - Core Stack: Rust (workers-rs 0.8.5 + axum 0.8) compiled to `wasm32-unknown-unknown`, one crate per Worker.
  - Database / Storage: Cloudflare D1 shared by both Workers (one per env); R2 `PUBLIC_ASSETS` (both) and `PRIVATE_STORAGE` (admin only).
  - Pattern: ports (`ports.rs` traits) + adapters (`adapters/worker_env.rs` real, `adapters/memory.rs` for tests).
- **Verification Status**:
  - Tests (2026-11-26, Rust 1.100): `cargo test --workspace --locked` 98 passed, 0 failed, 11 ignored (D1 tests): admin 7 lib + 6 scenarios, client 20 lib + 5 SPEC-001 + 13 SPEC-002a + 6 seed scenarios (+1 ignored D1 SQL), create-user 3, seed-build 5 lib + 3 main + 3 migrations + 7 seed files + 20 validation (+10 ignored D1). `make test-d1` 11 passed on real local D1. `make smoke-local` 70 checks ok on the real client Worker under `wrangler dev`. `make check` (wasm32) clean.
  - Host tests use the pure-Rust PBKDF2 path and the in-memory store, so they cannot catch wasm-only or D1-only bugs. Run the local Worker (`make dev-admin`) and the login/refresh smoke test before deploying.
  - Security Audit: `PASSED` (OWASP A01–A10, both services, 2026-11-16; SPEC-002a plans p1 and p2, 2026-11-26: [`docs/plan-records/SPEC-002a-p1-schema-seed/security-audit.md`](docs/plan-records/SPEC-002a-p1-schema-seed/security-audit.md), [`docs/plan-records/SPEC-002a-p2-public-read/security-audit.md`](docs/plan-records/SPEC-002a-p2-public-read/security-audit.md)).
  - Rustfmt / clippy: clean on host (`--all-targets`) and wasm32 (`--exclude create-user --exclude seed-build --exclude d1-local`) with `-D warnings`; blocking in CI since 2026-11-24.

---

## 3. Active In-Flight Work

- **Active Spec**: None. SPEC-002a is `COMPLETED` (2026-11-26). Next: [`SPEC-002b`](docs/specs/SPEC-002b-master-data-admin-write.md) (`APPROVED`, `PASSED`, `complex`), ready for `gb-plan` once the owner has deployed 002a to staging and checked it (the frontend switch is separate work, D11).
- **Active Plan**: None. Plans archived in `docs/plan-records/` (see section 6).
- **Current Task In-Flight**: None
- **Active Battleground**: None

---

## 4. Leftovers, Blockers & Next Steps

### Immediate Next Steps (For Next Agent / Developer)
Owner steps for SPEC-002a (each needs the owner's approval; agents never run `[CLOUD]` targets):
1. [ ] **Owner decision: CPU of a cache miss.** Staging 2026-11-26, 140 cache-busting requests via `wrangler tail`: `/v1/cards` p50 9 ms, p90 33 ms, p99 45 ms; `/v1/daily-cards` p50 10 ms, p90 20 ms, p99 41 ms; `/v1/card-art` p99 12 ms; `/v1/reading-config` p99 17 ms. All 140 `ok`, no exception, no error 1102, but p99 is above the 10 ms Free-plan target. Hits (most traffic, D13) use no CPU. Options: accept and watch (`TODO-020`), pre-rendered body (`TODO-018`, a spec change), or Workers Paid. Decide before the production frontend reads the API.
2. [ ] Mark the GitHub check `d1-tests` as required on `main` (branch protection). It passed on its first CI run (run 45759956879).
3. [x] Done 2026-11-26: push (`0d847ca`), CI green (both jobs), `cf-dry-run`, `migrate-staging` (`0002`), `seed-staging` (1,669 statements in one `--file`; counts 78/74/78/22 null/12/4), `deploy-staging`. D13 checklist: second GET is `cf-cache-status: HIT` for all four (incl. 355 KB daily-cards, 252 KB cards) with the same `x-request-id`; `age` counts up (24 s observed), so total staleness stays within D05's 10 minutes and the 20-minute question is moot; per-Origin variants are separate (allowed origins get their own `Access-Control-Allow-Origin`, others none); 405/404 have no cache entry, 404 and `/health` 503 show `BYPASS`, all `no-store`.

4. [ ] Enable R2 on the Cloudflare account (needs a payment method; free tier), create the staging buckets, restore the commented `r2_buckets` blocks in both `wrangler.toml`, redeploy. `/health` should then return 200.
5. [ ] Replace the Turnstile always-pass test secret on staging client with a real widget secret before any route uses Turnstile.
6. [ ] `gb-plan` SPEC-002b (admin write). Before it, the admin contract (`docs/api/admin/openapi.yaml`) still names `admin-api-staging.maemorkratai.com` and needs the same review D12 did for the client. The frontend switch to the API (outside the spec, D11) can start now that staging serves the data; its 3-card result box that reads `source_message` / `source_line` must be removed or hidden (D12).
7. [ ] Then the 3+1 reading engine (`TODO-003`).

### Leftovers & Technical Nuances
- **Leftover Tasks**: `TODO-006` items 2–5 (HTTPS URL guard, redacted security logs, login timing parity, `cargo-audit`). SPEC-002a follow-ups: `TODO-015` to `TODO-022` (see TODO.md).
- **Known Blockers / Warnings**:
  - Staging: D1 `maemorkratai-db-staging` (`f920237f-…`, APAC); Workers `https://maemorkratai-backend-admin-staging.maemorkratai.workers.dev` and `https://maemorkratai-backend-client-staging.maemorkratai.workers.dev`. `JWT_ISSUER` / `JWKS_URL` point at the admin workers.dev URL. Secrets are set; the owner keeps the generated values (pepper, KEK, owner password) in a private file outside the repo.
  - Staging has no R2 bindings (R2 not enabled on the account); the blocks are commented in both `wrangler.toml`.
  - Prod: `database_id` is still a placeholder and its vars still assume `*.maemorkratai.com`.
  - Deploy admin before client: admin publishes the JWKS the client verifies against.
  - The owner's Mac is low on disk. A lean local build keeps `target/` near 0.6 GB: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`. `cargo clean` frees it. `worker-build` must stay on 0.8 (`cargo install worker-build@^0.8 --locked`) while the crate is `worker` 0.8; 0.9 refuses to build. Installed on the owner's Mac 2026-11-25; both Worker bundles build.
  - `/gb-plan`, `/gb-execute`, `/gb-verify` are Claude Code skills in `.claude/skills/` that wrap `.gb-sdlc/workflows/02`–`04` and the presets in `.gb-sdlc/agents/` (written for Gemini). `.gb-sdlc/config.yaml` now has the `models:` block the rules referred to.
  - Ingestion scope undecided (`TODO-014`).

---

## 5. Proto-Specs & Deferred Ideas

- See [TODO.md](./TODO.md). One register for both services since 2026-11-24; its section 0 maps the old per-service IDs that older docs still use.

---

## 6. Context & Reference Pointers

- **Master Instructions**: [CLAUDE.md](./CLAUDE.md) · [GEMINI.md](./GEMINI.md) · [AGENTS.md](./AGENTS.md)
- **Specs**: SPEC-002 onward in [docs/specs/](docs/specs/); SPEC-001 in [docs/admin/specs/](docs/admin/specs/) and [docs/client/specs/](docs/client/specs/)
- **Decisions**: D05–D13 in [docs/decisions/](docs/decisions/); D01–D04 in [docs/admin/decisions/](docs/admin/decisions/) (same text in `docs/client/decisions/`)
- **Architectures**: [docs/admin/architectures/](docs/admin/architectures/), [docs/client/architectures/](docs/client/architectures/)
- **Researches**: [docs/researches/](docs/researches/), plus the per-service folders
- **Completed Plan Records**: [docs/admin/plan-records/](docs/admin/plan-records/), [docs/client/plan-records/](docs/client/plan-records/); SPEC-002a: [docs/plan-records/SPEC-002a-p1-schema-seed/](docs/plan-records/SPEC-002a-p1-schema-seed/), [docs/plan-records/SPEC-002a-p2-public-read/](docs/plan-records/SPEC-002a-p2-public-read/), estimate and plan audit in [docs/plan-records/SPEC-002a/](docs/plan-records/SPEC-002a/)
- **Master data docs**: [docs/knowledges/features/master-data.md](docs/knowledges/features/master-data.md), [docs/knowledges/features/master-data-public-read.md](docs/knowledges/features/master-data-public-read.md), [docs/architectures/client-worker.md](docs/architectures/client-worker.md), [docs/knowledges/code-standards.md](docs/knowledges/code-standards.md)
- **Deployment runbook**: [docs/deployment.md](docs/deployment.md)
- **Actors and roles**: [docs/knowledges/actors-and-roles.md](docs/knowledges/actors-and-roles.md)
- **Reading engine rules and data model**: [../docs/context/09-reading-rules-v0.md](../docs/context/09-reading-rules-v0.md), [10](../docs/context/10-reading-master-data.md), [11](../docs/context/11-phase-2-plan.md)

---

## 7. History

| Date | Event |
| :--- | :--- |
| 2026-11-15 | SPEC-001 approved for both services; decisions D01–D04 |
| 2026-11-16 | SPEC-001 executed and verified: admin 15/15 tests, client 8/8, OWASP audit passed; plan records archived |
| 2026-11-20 | Repos merged into `backend/`; Cloudflare deploy prep (Makefile, CI, deploy workflow, runbook) |
| 2026-11-21 | SPEC-002 approved after 3 Spectre rounds; D05–D10; contracts v0.2.0; `TODO-010`, `TODO-011` |
| 2026-11-25 | Local run of the admin Worker found two bugs the host tests could not see: wasm PBKDF2 failed (`WorkerGlobalScope` cast) so login never worked on Workers, and refresh rotation inserted an empty `user_id` (FK failure). Both fixed, regression test added (24 tests). 5xx details now logged server-side. `make create-user` fixed to run from `admin/`. First staging deploy |
| 2026-11-25 | SPEC-002 split into SPEC-002a (read) and SPEC-002b (write), D11; both approved after a Spectre split audit (one gap resolved: uncategorized order) |
| 2026-11-26 | SPEC-002a executed (15 tasks, commits `9a8bb03`..`fc3458e`) and verified: 98 tests + 11 D1 tests, fmt, both clippy runs, `make check`, `make smoke-local` (70 checks) clean; OWASP audit passed for both plans; plans archived to `docs/plan-records/`; `TODO-021`, `TODO-022` logged. Deviations recorded during execution: p1/03 broken code fence in the plan, p1/04 SQL header comment reworded (it contained the token `audit_events` a test forbids) and rustfmt needed two passes, p2/04 red showed 7 failing tests not 9 |
| 2026-11-24 | Friend stopped; owner maintains alone. Phase 0: HANDOFF and TODO merged into one set each, docs updated, Rust 1.100 installed, 23/23 tests, fmt + clippy clean, CI lint steps blocking |
