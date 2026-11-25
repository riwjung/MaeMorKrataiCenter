# Project Handoff & Session State (`HANDOFF.md`)

> **Role & Purpose**: This document preserves cross-session continuity between human developers and autonomous AI agents.
> **Rule for Incoming Agents**: Always read this file at the start of a session to understand current status, what has been completed, in-flight work, and immediate next steps. Update this file before concluding your session.

One handoff for the whole `backend/` workspace (`client` and `admin` Workers). Until 2026-11-24 each service kept its own file; that history is condensed in section 7.

---

## 1. Executive Summary & Progress

- **Project Status**: `PLANNING` (SPEC-002 approved, not planned) · Phase 0 solo cleanup done locally, not committed yet
- **Last Updated**: 2026-11-24
- **Owner**: the project owner, alone since 2026-11-24 (frontend and backend). The scaffold was built by a friend who has stopped.
- **Active Session Agent**: Developer + Claude Code
- **Progress Snapshot**: Both Workers exist; all 23 tests pass and both build for wasm32 on the owner's Mac (2026-11-24). `admin` has complete auth (SPEC-001). `client` serves only `/health`. Nothing is deployed: no Cloudflare resources, secrets or domain yet.

### Milestone Checklist
- [x] SPEC-001 infra scaffold, both services, verified and OWASP-audited (2026-11-16)
- [x] `backend-client` + `backend-admin` merged into one `backend/` workspace (2026-11-20)
- [x] Cloudflare deploy prep: Makefile targets, `backend-ci` and manual `backend-deploy` workflows, `docs/deployment.md` (2026-11-20)
- [x] SPEC-002 master data approved, Spectre audit passed in 3 rounds; API contracts v0.2.0 drafted (2026-11-21)
- [x] Phase 0: local toolchain, `make test` / `make check`, rustfmt + clippy clean (CI now blocking), docs updated for solo ownership, one HANDOFF/TODO (2026-11-24, awaiting commit)
- [ ] First staging deploy on `*.workers.dev`
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
  - Tests: `cargo test --workspace --locked` 23/23 on 2026-11-24 (Rust 1.100): admin 7 lib + 5 scenarios, client 3 lib + 5 scenarios, create-user 3. Same counts as the SPEC-001 record (admin 15, client 8). `make check` (wasm32) clean.
  - Security Audit: `PASSED` (OWASP A01–A10, both services, 2026-11-16).
  - Rustfmt / clippy: clean on host (`--all-targets`) and wasm32 with `-D warnings`; blocking in CI since 2026-11-24.

---

## 3. Active In-Flight Work

- **Active Spec**: [`docs/specs/SPEC-002-master-data-to-backend.md`](docs/specs/SPEC-002-master-data-to-backend.md) (`APPROVED`, `spectre_audit: PASSED`, `complexity: complex`)
- **Active Plan**: None (SPEC-002 not planned yet)
- **Current Task In-Flight**: None (Phase 0 changes are in the working tree, waiting for the owner to commit)
- **Active Battleground**: None

---

## 4. Leftovers, Blockers & Next Steps

### Immediate Next Steps (For Next Agent / Developer)
1. [ ] Commit Phase 0 (docs, HANDOFF/TODO merge, fmt/clippy fixes, blocking CI) and check the first `backend-ci` run on GitHub.
2. [ ] Review API contracts v0.2.0 against `frontend-client` (English keys D08, nullable love-status texts and uncategorized lists D09/D10). No second approver any more; review against the frontend code.
3. [ ] First staging deploy, following `docs/deployment.md` one-time setup (`make cf-bootstrap ENV=staging`, secrets, `make migrate-staging`, `make create-user`, `make deploy-staging`). Changes real Cloudflare state: owner approves first.
4. [ ] Plan SPEC-002. Owner decision pending: build it whole (36 admin operations) or ship the 4 public GETs + seed first and the admin CRUD later.
5. [ ] Then the 3+1 reading engine (`TODO-003`).

### Leftovers & Technical Nuances
- **Leftover Tasks**: `TODO-006` items 2–5 (HTTPS URL guard, redacted security logs, login timing parity, `cargo-audit`).
- **Known Blockers / Warnings**:
  - `database_id` values in both `wrangler.toml` files are placeholders (`00000000-…`). Replace with the real IDs before any remote command.
  - `ALLOWED_ORIGINS`, `JWT_ISSUER` and `JWKS_URL` for staging/prod assume `*.maemorkratai.com`, which is not routed yet. For a first deploy on `*.workers.dev`, update those vars to the workers.dev URLs.
  - Deploy admin before client: admin publishes the JWKS the client verifies against.
  - The owner's Mac is low on disk. A lean local build keeps `target/` near 0.6 GB: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`. `cargo clean` frees it. `worker-build` (needed for `make dev-*` and deploys) is not installed yet.
  - The `/gb-plan`, `/gb-execute`, `/gb-verify` skills named in `CLAUDE.md` are not in the repo (`.claude/skills/` was never committed). Follow `.gb-sdlc/workflows/02`–`04` directly, or recreate the skills before planning SPEC-002.
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
| 2026-11-24 | Friend stopped; owner maintains alone. Phase 0: HANDOFF and TODO merged into one set each, docs updated, Rust 1.100 installed, 23/23 tests, fmt + clippy clean, CI lint steps blocking |
