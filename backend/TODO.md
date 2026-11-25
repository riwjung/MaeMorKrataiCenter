# Project Roadmap & Proto-Spec Incubator (`TODO.md`)

> **The Proto-Spec Concept**:
> In `gb-sdlc`, task files strictly require full code and forbid in-code `// TODO: implement later` placeholders.
> However, during planning, execution, and code review, developers and agents frequently uncover insights, edge cases, refactorings, or architectural enhancements that are **worth not doing right now, but valuable to add later**.
>
> Rather than letting these ideas get lost or polluting active tasks, capture them here as **Proto-Specs** (Candidate Specs).
> When the project is ready for its next cycle, **Spectre** reviews this file and graduates high-value proto-specs into formal specifications (`docs/specs/<spec-name>.md`).

One register for both Workers (`client`, `admin`). Each item names the service(s) it touches. New items take the next free ID (`TODO-015`).

---

## 0. Former IDs (read this before following an ID in older docs)

Until 2026-11-24 `client` and `admin` kept separate registers and some IDs collided. Older specs, decisions and plan records still use the old IDs. Map them like this:

| Old ID | Where it appears | Meaning there | Now |
| :--- | :--- | :--- | :--- |
| `TODO-001` | both registers | empty placeholder | removed |
| `TODO-002` | both | AI chat, RAG, agentic tools | `TODO-002` |
| `TODO-003` | `docs/client/*` / `docs/admin/*` | reading engine runtime / knowledge curation and eval | `TODO-003` (merged) |
| `TODO-004` | `docs/client/*` / `docs/admin/*` | orders and credits / payment reconciliation admin | `TODO-004` (merged) |
| `TODO-005` | `docs/client/*` | customer accounts | `TODO-005` |
| `TODO-005` | `docs/admin/*` | Content Studio job queue, Social Center | `TODO-012` |
| `TODO-006` | `docs/client/*`, CI comments | infra hardening, clippy | `TODO-006` |
| `TODO-006` | `docs/admin/*` | Cloudflare Access, key rotation automation | `TODO-013` |
| `TODO-007` | `docs/admin/*` (D03, SPEC-001) | Argon2id upgrade | `TODO-007` |
| `TODO-007` | deployment notes (2026-11-20) | ingestion pipeline | `TODO-014` |
| `TODO-008` | `docs/admin/*` | session / JWK cleanup cron | `TODO-008` |
| `TODO-009` | `docs/admin/*` | clippy, login timing parity, cargo-audit | merged into `TODO-006` |
| `TODO-010`, `TODO-011` | SPEC-002, D05, client OpenAPI | pagination, cache invalidation | unchanged |

---

## 1. Proto-Spec Register

| ID | Title | Service | Priority | Status | Discovered During | Graduated Spec |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `TODO-002` | AI-Native LLM Chat, RAG Database & Agentic Tools | both | High | PROPOSED | SPEC-001 Scoping | - |
| `TODO-003` | 3+1 Reading Engine: Sessions, Knowledge Releases & Eval | both | High | PROPOSED | SPEC-001 Scoping | - |
| `TODO-004` | Orders, Credits Ledger, Payment Webhooks & Reconciliation | both | Medium | PROPOSED | SPEC-001 Scoping | - |
| `TODO-005` | Customer Accounts, Auth & Saved Reading History | client | Medium | PROPOSED | SPEC-001 Scoping | - |
| `TODO-006` | Hardening: Clippy/Rustfmt, HTTPS URL Guard, Audit Logs, Timing Parity, `cargo-audit` | both | Low | IN_PROGRESS | SPEC-001 Security Audit | - |
| `TODO-007` | Argon2id Password Hashing Upgrade (Workers Paid Tier) | admin | Low | PROPOSED | SPEC-001 Spectre Audit | - |
| `TODO-008` | Scheduled D1 Session & Retiring-JWK Cleanup Cron | admin | Low | PROPOSED | SPEC-001 Planning | - |
| `TODO-010` | Pagination for Public Master-Data List Endpoints | client | Low | PROPOSED | SPEC-002 Scoping | - |
| `TODO-011` | Cache Invalidation on Admin Edit for Public Master Data | both | Medium | PROPOSED | SPEC-002 Scoping | - |
| `TODO-012` | Content Studio Job Queue (Mac Pull Worker) & Social Center | admin | Medium | PROPOSED | SPEC-001 Scoping | - |
| `TODO-013` | Cloudflare Access Zero Trust Migration & Key Rotation Automation | admin | Medium | PROPOSED | SPEC-001 Auth Design | - |
| `TODO-014` | Cloudflare Ingestion Pipeline (needs spec) | admin | Medium | PROPOSED | Cloudflare deploy prep | - |

---

## 2. Active Proto-Specs

### [TODO-002] AI-Native LLM Chat, RAG Database & Agentic Tools
- **Status**: `PROPOSED`
- **Service**: both
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Scoping
- **Priority**: `P1 (High)`
- **Why Not Now?**: Deferred from `SPEC-001` so the Workers, D1, R2 and auth were built and audited first. Zero public LLM endpoints in Phase 1.
- **Context & Problem Statement**:
  LLM chat, a RAG knowledge base and agentic tool calls (knowledge proposal extraction, competitor clip analysis, evidence-grounded reading assistance), with strict evidence checks and spend/rate caps.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Cloudflare AI Gateway + Vectorize/RAG index + typed tool contracts + output evidence verifier + fallback to `needs_owner`.
  - Prompt-injection defense and per-job / per-day budget caps.
- **Graduation / Promotion Criteria**:
  - After `SPEC-002` is live and reader-approved knowledge releases exist (`TODO-003`).

### [TODO-003] 3+1 Reading Engine: Sessions, Knowledge Releases & Eval
- **Status**: `PROPOSED`
- **Service**: both (client: sessions and answers; admin: card data, releases, evaluation)
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Scoping (from `../docs/context/05` and `06`)
- **Priority**: `P1 (High)`
- **Why Not Now?**: Needs `SPEC-002` card data in D1 first.
- **Context & Problem Statement**:
  - client: replace the browser shuffle and demo `createReading()` in `frontend-client/src/lib/reading.ts` with a server-authoritative 3+1 draw; the 4th card stays hidden until the follow-up.
  - admin: the reader edits card scores and texts, publishes immutable knowledge releases, resolves `needs_owner` escalations, and runs evaluations on strict `development` / `holdout` splits.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Rules and data model are already drafted: `../docs/context/09-reading-rules-v0.md` (rules v0.2), `10-reading-master-data.md` (D1 DDL: `card_scores`, `knowledge_releases`, `reading_sessions`, `reading_outputs` with `trace_json`, `eval_runs`), `11-phase-2-plan.md` (work items).
  - Session state machine in D1, deterministic Engine A, Turnstile on session start, idempotency key.
  - Real reader answers and audio stay out of Git; rules reference them by `source_ref` only.
- **Graduation / Promotion Criteria**:
  - After `SPEC-002` is deployed and the reader has confirmed a first set of card scores.

### [TODO-004] Orders, Credits Ledger, Payment Webhooks & Reconciliation
- **Status**: `PROPOSED`
- **Service**: both (client: checkout and webhooks; admin: reconciliation views)
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Scoping (from `../docs/context/04`)
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: Phase 1 has no customer accounts or payments.
- **Context & Problem Statement**:
  Credit packs priced in integer satang, signature-verified payment webhooks, idempotent order processing, atomic credit reserve/commit/release, plus admin visibility into orders, provider events, ledger adjustments, refunds and stuck reservations.
- **Proto-Spec Sketch (Suggested Approach)**:
  - D1 `batch()` conditional ledger updates, or a Durable Object per account.
  - Admin read/reconcile endpoints over the same ledger tables.
- **Graduation / Promotion Criteria**:
  - Paired with customer accounts (`TODO-005`) before a paid beta.

### [TODO-005] Customer Accounts, Auth & Saved Reading History
- **Status**: `PROPOSED`
- **Service**: client
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Scoping
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: Phase 1 is account-less (`D04` public-connection model).
- **Context & Problem Statement**:
  Customer login, saved reading history and async `needs_owner` notifications, using the `user` role already defined in `D03`.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Reuse the client `JwksVerifier` scaffold and the `users` table (`role = 'user'`).
- **Graduation / Promotion Criteria**:
  - Required before a paid beta (`TODO-004`).

### [TODO-006] Hardening: Clippy/Rustfmt, HTTPS URL Guard, Audit Logs, Timing Parity, `cargo-audit`
- **Status**: `IN_PROGRESS` (item 1 done 2026-11-24; items 2–5 open)
- **Service**: both
- **Discovered On**: 2026-11-16
- **Discovered By / During**: SPEC-001 Verifier & Security Audit (client and admin)
- **Priority**: `P3 (Low)`
- **Why Not Now?**: Non-blocking; all SPEC-001 acceptance criteria and OWASP A01–A10 checks passed.
- **Context & Problem Statement**:
  1. ~~Clippy and rustfmt fail on existing code, so CI runs them as advisory only.~~ Done 2026-11-24 (Rust 1.100): `cargo fmt --all`; fixed `unnecessary_cast` (client and admin `Clock`), unused `DbSessionRow.token_hash` (dropped from the struct and the two SELECTs), `cloned_ref_to_slice_refs` (`auth/jwt.rs` test), `let_unit_value` (`auth/password.rs` test); allowed `result_large_err` on `authenticate_from_parts` (axum rejection type); moved `pbkdf2` to non-wasm dependencies. CI fmt and clippy steps are now blocking.
  2. client: enforce an `https://` scheme on `TURNSTILE_VERIFY_URL` and `JWKS_URL` in staging and prod.
  3. Structured, PII/token-redacted security logs for `401` / `403` / `429` / `503`. (Partly done 2026-11-25: every 5xx with an internal detail is logged server-side with its request id, in both Workers.)
  4. admin: `handle_login` returns before PBKDF2 when the email is unknown, a small timing difference between unknown emails and wrong passwords. Run a dummy verification.
  5. `cargo-audit` is not in CI.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Items 2–5 as one small maintenance PR.
- **Graduation / Promotion Criteria**:
  - Items 2–5 before the first prod deploy.

### [TODO-007] Argon2id Password Hashing Upgrade (Workers Paid Tier)
- **Status**: `PROPOSED`
- **Service**: admin
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Spectre Audit (`D03`)
- **Priority**: `P3 (Low)`
- **Why Not Now?**: The Workers Free tier (10 ms CPU) cannot run memory-hard Argon2id; WebCrypto PBKDF2-HMAC-SHA256 (100,000 iterations) + `PASSWORD_PEPPER` fits.
- **Context & Problem Statement**:
  If the project moves to Workers Paid before replacing password login with Cloudflare Access (`TODO-013`), upgrade hashes lazily on login from `$pbkdf2-sha256-peppered$...` to Argon2id (16 MiB, t=2, p=1).
- **Proto-Spec Sketch (Suggested Approach)**:
  - Argon2id hasher/verifier behind versioned hash-prefix dispatch.
- **Graduation / Promotion Criteria**:
  - Only if Workers Paid is active while email + password is still the primary login.

### [TODO-008] Scheduled D1 Session & Retiring-JWK Cleanup Cron
- **Status**: `PROPOSED`
- **Service**: admin
- **Discovered On**: 2026-11-16
- **Discovered By / During**: SPEC-001 Planning
- **Priority**: `P3 (Low)`
- **Why Not Now?**: Expired rows are inert because every read checks `expires_at`.
- **Context & Problem Statement**:
  Expired `sessions` rows and old `retiring` keys in `jwks_keys` accumulate.
- **Proto-Spec Sketch (Suggested Approach)**:
  - `[triggers] crons` + `#[event(scheduled)]` handler: delete expired sessions beyond a retention window; move expired `retiring` keys to `revoked`.
- **Graduation / Promotion Criteria**:
  - When auth traffic grows, or before the `TODO-013` cutover.

### [TODO-010] Pagination for Public Master-Data List Endpoints
- **Status**: `PROPOSED`
- **Service**: client
- **Discovered On**: 2026-11-20
- **Discovered By / During**: `SPEC-002` scoping (Spectre)
- **Priority**: `P3 (Low)`
- **Why Not Now?**: `SPEC-002` mirrors today's frontend JSON shapes (78 cards) and relies on caching.
- **Context & Problem Statement**:
  Card and daily-content lists are returned whole. As content grows, the frontend may want to lazy-load pages.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Optional pagination parameters (offset or cursor, to be decided), keeping the unpaginated response backward compatible; cache keys include the parameters.
- **Graduation / Promotion Criteria**:
  - When the frontend needs lazy loading, or a list response becomes noticeably slow or large.

### [TODO-011] Cache Invalidation on Admin Edit for Public Master Data
- **Status**: `PROPOSED`
- **Service**: both
- **Discovered On**: 2026-11-21
- **Discovered By / During**: `SPEC-002` scoping (Spectre + Researcher)
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: `SPEC-002` ships a plain 10-minute TTL (`Cache-Control: public, max-age=600`, D05). Edits may take up to 10 minutes to reach customers.
- **Context & Problem Statement**:
  `admin` and `client` are separate Workers on `*.workers.dev`; `caches.default` is a no-op there and per-colo anyway; KV propagates in 60 s or more. Research: [docs/researches/2026-11-20-workers-cache-invalidation.md](docs/researches/2026-11-20-workers-cache-invalidation.md).
- **Proto-Spec Sketch (Suggested Approach)**:
  - Option A: `content_version` row bumped in the same D1 `batch()` as every admin write; client returns `ETag` + `Cache-Control: no-cache` and answers `304` on match.
  - Option B: Workers Cache on `client` + admin-to-client purge over a JWT-authenticated service binding (needs a workers-rs spike).
- **Graduation / Promotion Criteria**:
  - When 10-minute staleness bothers the reader or owner, or a custom domain is attached.

### [TODO-012] Content Studio Job Queue (Mac Pull Worker) & Social Center
- **Status**: `PROPOSED`
- **Service**: admin
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Scoping (from `../docs/context/06` and `../docs/reference/social-center.md`)
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: Needs the private R2 bucket and admin auth in production first.
- **Context & Problem Statement**:
  Durable job state for Blender/ffmpeg video pipelines, pulled by the owner's Mac with a scoped token, plus Social Center Meta OAuth/analytics and competitor research.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Cloudflare Queues or D1 lease-based jobs + R2 artifact manifests + encrypted Meta OAuth token storage.
- **Graduation / Promotion Criteria**:
  - When Content Studio or Social Center work starts in `frontend-admin/`.

### [TODO-013] Cloudflare Access Zero Trust Migration & Key Rotation Automation
- **Status**: `PROPOSED`
- **Service**: admin
- **Discovered On**: 2026-11-15
- **Discovered By / During**: SPEC-001 Auth Design (`D03`)
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: Phase 1 uses email + password for about 3 admin users.
- **Context & Problem Statement**:
  Replace email + password with Cloudflare Access JWT verification, and/or automate scheduled JWKS rotation.
- **Proto-Spec Sketch (Suggested Approach)**:
  - Point the JWKS verifier at the Cloudflare Access `certs` URL and validate `Cf-Access-Jwt-Assertion`, mapped to `users.email` / `role`.
- **Graduation / Promotion Criteria**:
  - When Cloudflare Access is provisioned for the admin team.

### [TODO-014] Cloudflare Ingestion Pipeline (needs spec)
- **Status**: `PROPOSED`
- **Service**: admin
- **Discovered On**: 2026-11-20
- **Discovered By / During**: Cloudflare deployment prep (worktree `backend-cloudflare-deploy`)
- **Priority**: `P2 (Medium)`
- **Why Not Now?**: No ingestion pipeline exists and its shape is an architecture decision. Scope is not defined.
- **Context & Problem Statement**:
  Deployment prep was requested "with ingestion". Candidates: R2 `PRIVATE_STORAGE` upload event → Queue → consumer Worker; or RAG / knowledge-release ingestion (see `TODO-002`, `TODO-003`).
- **Graduation / Promotion Criteria**:
  - The owner confirms the ingestion scope; Spectre drafts a spec and logs the decision.

---

## 3. Graduated & Archived Proto-Specs

| ID | Title | Outcome |
| :--- | :--- | :--- |
| `TODO-001` (both) | Initial Feature Enhancements | Removed 2026-11-24: empty placeholder |
| `TODO-009` (admin) | Code Quality & Auth Hardening Follow-Ups | Merged into `TODO-006` on 2026-11-24 |
