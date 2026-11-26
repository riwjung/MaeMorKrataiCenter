# Project Handoff & Session State (`HANDOFF.md`)

> **Role & Purpose**: This document preserves cross-session continuity between human developers and autonomous AI agents.
> **Rule for Incoming Agents**: Always read this file at the start of a session to understand current status, what has been completed, in-flight work, and immediate next steps. Update this file before concluding your session.

One handoff for the whole `backend/` workspace (`client` and `admin` Workers). Until 2026-11-24 each service kept its own file; that history is condensed in section 7.

---

## 1. Executive Summary & Progress

- **Project Status**: `EXECUTING (SPEC-002a plans aligned; next: gb-execute)` · staging deployed 2026-11-25
- **Last Updated**: 2026-11-25
- **Owner**: the project owner, alone since 2026-11-24 (frontend and backend). The scaffold was built by a friend who has stopped.
- **Active Session Agent**: Developer + Claude Code
- **Progress Snapshot**: Both Workers exist; all 23 tests pass and both build for wasm32 on the owner's Mac (2026-11-24). `admin` has complete auth (SPEC-001). `client` serves only `/health`. Staging runs on `*.workers.dev` with D1 (no R2 yet); prod is not deployed and there is no custom domain.

### Milestone Checklist
- [x] SPEC-001 infra scaffold, both services, verified and OWASP-audited (2026-11-16)
- [x] `backend-client` + `backend-admin` merged into one `backend/` workspace (2026-11-20)
- [x] Cloudflare deploy prep: Makefile targets, `backend-ci` and manual `backend-deploy` workflows, `docs/deployment.md` (2026-11-20)
- [x] SPEC-002 master data approved, Spectre audit passed in 3 rounds; API contracts v0.2.0 drafted (2026-11-21)
- [x] Phase 0: local toolchain, `make test` / `make check`, rustfmt + clippy clean (CI now blocking), docs updated for solo ownership, one HANDOFF/TODO (2026-11-24, awaiting commit)
- [x] First staging deploy on `*.workers.dev` (2026-11-25): login, `/v1/me`, JWKS and refresh rotation verified live; `/health` 503 until R2 is enabled
- [ ] SPEC-002 implemented and the frontend reading master data from the API
- [ ] 3+1 reading engine (`TODO-003`)

---

## 2. What We Have in Current (Current State)

- **Working Components**:
  - `admin/`: email + password login (PBKDF2-SHA256 100k + pepper), ES256 access JWT (15 min), HttpOnly refresh cookie (7 days) with rotation and replay detection (family revoke), RBAC `owner > admin > user`, `/v1/me`, owner-only key rotation, `/.well-known/jwks.json`, audit events, CORS/CSRF/rate-limit/body-cap middleware, error envelope.
  - `client/`: security pipeline (request id, CORS, 60 req/min per path+IP, 32 KB body cap), `GET /health`. Turnstile guard and JWKS verifier exist but no route uses them yet.
  - `migrations/0001_init_auth.sql`: `users`, `sessions`, `jwks_keys`, `audit_events`. Applied through the admin Worker's D1 binding.
  - `tools/create-user`: creates a user via `wrangler d1 execute` (password prompted, min 12 chars).
- **Architecture & Tech Stack**:
  - Core Stack: Rust (workers-rs 0.8.5 + axum 0.8) compiled to `wasm32-unknown-unknown`, one crate per Worker.
  - Database / Storage: Cloudflare D1 shared by both Workers (one per env); R2 `PUBLIC_ASSETS` (both) and `PRIVATE_STORAGE` (admin only).
  - Pattern: ports (`ports.rs` traits) + adapters (`adapters/worker_env.rs` real, `adapters/memory.rs` for tests).
- **Verification Status**:
  - Tests: `cargo test --workspace --locked` 24/24 on 2026-11-25 (Rust 1.100): admin 7 lib + 6 scenarios, client 3 lib + 5 scenarios, create-user 3. `make check` (wasm32) clean.
  - Host tests use the pure-Rust PBKDF2 path and the in-memory store, so they cannot catch wasm-only or D1-only bugs. Run the local Worker (`make dev-admin`) and the login/refresh smoke test before deploying.
  - Security Audit: `PASSED` (OWASP A01–A10, both services, 2026-11-16).
  - Rustfmt / clippy: clean on host (`--all-targets`) and wasm32 with `-D warnings`; blocking in CI since 2026-11-24.

---

## 3. Active In-Flight Work

- **Active Spec**: [`docs/specs/SPEC-002a-master-data-schema-seed-public-read.md`](docs/specs/SPEC-002a-master-data-schema-seed-public-read.md) (`APPROVED`, `PASSED`, `complex`). Next: [`SPEC-002b`](docs/specs/SPEC-002b-master-data-admin-write.md) (`APPROVED`, `PASSED`, `complex`), after 002a is verified. Parent SPEC-002 split by D11.
- **Active Plan**: [`docs/plans/SPEC-002a-PLAN-AUDIT.md`](docs/plans/SPEC-002a-PLAN-AUDIT.md) (`ALIGNED`, 2026-11-26): plans `SPEC-002a-p1-schema-seed` (6 tasks) then `SPEC-002a-p2-public-read` (9 tasks), one global order in the audit §2. Done: p1/01–04 (2026-11-26). Next task: `docs/plans/SPEC-002a-p1-schema-seed/05_MAKEFILE_SEED_TARGETS.md`. Owner steps (staging deploy, seed load, required `d1-tests` check, D13 checklist) are in audit §4.
- **Current Task In-Flight**: None between tasks (gb-execute SPEC-002a; p1/01 seed converted; p1/02 `d1-local` harness, `seed-build` skeleton, `make test-d1` (4 passing), CI job `d1-tests`; p1/03 migration `0002_master_data.sql` (12 tables), scenario 28 and A2 green, `make test-d1` 8 passing; p1/04 `seed-build` (1 plan deviation fixed: header comment), scenario 27 green, `make test-d1` 10 passing)
- **Active Battleground**: None

---

## 4. Leftovers, Blockers & Next Steps

### Immediate Next Steps (For Next Agent / Developer)
1. [ ] Enable R2 on the Cloudflare account (needs a payment method; free tier), run `make cf-bootstrap`-style bucket creation for staging, restore the commented `r2_buckets` blocks in both `wrangler.toml`, redeploy. `/health` should then return 200.
2. [ ] Review the client contract against `frontend-client` as the first step of `gb-plan` SPEC-002a (D11 item 5): English keys D08, nullable love-status texts and uncategorized lists D09/D10/D11, plus the two notes in SPEC-002a §10 (405 vs unknown paths, staging server URL).
3. [ ] Replace the Turnstile always-pass test secret on staging client with a real widget secret before any route uses Turnstile.
4. [ ] `gb-plan` SPEC-002a (schema, seed, public GETs), then `gb-execute`, `gb-verify`, deploy to staging. Then the frontend switch (outside the spec, D11), then SPEC-002b.
5. [ ] Then the 3+1 reading engine (`TODO-003`).

### Leftovers & Technical Nuances
- **Leftover Tasks**: `TODO-006` items 2–5 (HTTPS URL guard, redacted security logs, login timing parity, `cargo-audit`).
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
- **Decisions**: D05–D10 in [docs/decisions/](docs/decisions/); D01–D04 in [docs/admin/decisions/](docs/admin/decisions/) (same text in `docs/client/decisions/`)
- **Architectures**: [docs/admin/architectures/](docs/admin/architectures/), [docs/client/architectures/](docs/client/architectures/)
- **Researches**: [docs/researches/](docs/researches/), plus the per-service folders
- **Completed Plan Records**: [docs/admin/plan-records/](docs/admin/plan-records/), [docs/client/plan-records/](docs/client/plan-records/)
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
| 2026-11-24 | Friend stopped; owner maintains alone. Phase 0: HANDOFF and TODO merged into one set each, docs updated, Rust 1.100 installed, 23/23 tests, fmt + clippy clean, CI lint steps blocking |
