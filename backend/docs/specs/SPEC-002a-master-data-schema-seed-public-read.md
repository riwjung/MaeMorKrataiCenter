---
id: SPEC-002a
title: "Master Data: Schema, Seed and Public Read API"
status: APPROVED # DRAFT | SPECTRE_REVIEW | APPROVED | IMPLEMENTING | COMPLETED
spectre_audit: PASSED # PENDING | PASSED
complexity: complex # owner's choice 2026-11-25 (D11): the schema here is the base for SPEC-002b
created_at: "2026-11-25"
updated_at: "2026-11-25"
parent_spec: "SPEC-002"
sub_specs: []
---

# Specification: Master Data – Schema, Seed and Public Read API

> First half of [SPEC-002](SPEC-002-master-data-to-backend.md), split by [D11](../decisions/D11.md). The write half is [SPEC-002b](SPEC-002b-master-data-admin-write.md).

## 1. Intent & High-Level Overview
`frontend-client` bundles its master data as static JSON and TypeScript constants. This spec puts that data into the shared D1 database and serves it through public, read-only, cached GETs on `client`, so the site can later read it from the API.

- **Customer** (anonymous) reads the data through `client`. No accounts.
- **Developer** loads and re-loads the data with a seed command.
- Admin editing is not part of this spec (SPEC-002b). Until then, data changes go through the seed files.

Card images stay in `frontend-client/public/`; the API returns the same relative art paths. No R2 work. Shuffling and drawing stay in the browser. Actors: [actors-and-roles](../knowledges/actors-and-roles.md).

## 2. Research & Spikes (Linked)
- [Research: Workers cache invalidation](../researches/2026-11-20-workers-cache-invalidation.md)

## 3. Key Decisions (Linked)
- D01–D04 (SPEC-001): `docs/admin/decisions/`. D04 public `client` security baseline.
- [D05: 10-minute public cache TTL, no invalidation yet](../decisions/D05.md)
- [D07: Migrations hold schema only; seed loaded manually](../decisions/D07.md)
- [D08: English keys, Thai labels kept exactly per screen](../decisions/D08.md)
- [D09: Master-data integrity rules](../decisions/D09.md) (schema-level parts)
- [D10: Deleting a category leaves its items uncategorized](../decisions/D10.md) (nullable `category`, public uncategorized lists)
- [D11: Split into SPEC-002a / SPEC-002b](../decisions/D11.md)
- [D12: Client contract review (405 on known paths, 404 on unknown; staging server URL)](../decisions/D12.md)
- [D13: Workers Cache on `client` (config only, no purge, D05 TTL unchanged)](../decisions/D13.md)

## 4. Data in Scope
[SPEC-002 §4.1–4.4](SPEC-002-master-data-to-backend.md#4-data-in-scope) is normative for this spec: sources, the field keep/drop table, English keys and labels, situation keys, suggestion IDs, per-screen ordering, and the integrity rules. §4.5 (admin write validation) belongs to SPEC-002b.

The schema delivered here is the **complete** master-data schema for SPEC-002: every table, column, foreign key, uniqueness rule and nullable `category` that §4.4, D09 and D10 require, including storage for the daily metadata (`schema_version`, `version`, `updated`, `status`, `scope`, `auto_fallback_to_general_or_spread`). SPEC-002b adds no migration for this data unless its own audit finds a gap and logs a decision.

## 5. API Surface (contract in `docs/api/`)
- `../docs/api/client/openapi.yaml` (v0.2.1, reviewed by the owner in D12): `GET /v1/cards`, `/v1/daily-cards`, `/v1/card-art`, `/v1/reading-config`. Public, no auth, D04 rate limit, `Cache-Control: public, max-age=600`.
- The owner reviews the contract against `frontend-client` while planning (D11); any change to it is made in `docs/api/client/openapi.yaml` before the code.

## 6. Behavioral Requirements (EARS Syntax)

**Public read (`client`)**
- **UBIQUITOUS**: THE SYSTEM SHALL serve master data from D1 with the same structure as the source files, using English keys (D08) and without dropped fields (SPEC-002 §4.2).
- **UBIQUITOUS**: THE SYSTEM SHALL return `art` paths exactly as stored, and `null` where a card has no art.
- **UBIQUITOUS**: THE SYSTEM SHALL return every current love-status key in `meanings`, `details` and `love_by_status`, with `null` where no text is stored.
- **UBIQUITOUS**: THE SYSTEM SHALL compute each daily card's `missing_fields` from its `null` daily texts (`content.love_by_status.<key>`).
- **UBIQUITOUS**: THE SYSTEM SHALL list situations and suggestions whose `category` is `null` in `uncategorized_situations` and `uncategorized_suggestions`, ordered by `sort_order`, then key/id.
- **STATE-DRIVEN**: WHILE no category `love` exists THE SYSTEM SHALL list the situations derived from love statuses in `uncategorized_situations`, ordered by `reading_sort_order`, after all other uncategorized situations (D11 item 6).
- **UBIQUITOUS**: THE SYSTEM SHALL return responses that match `../docs/api/client/openapi.yaml`.
- **EVENT-DRIVEN**: WHEN a public master-data GET succeeds THE SYSTEM SHALL return `200` with `Cache-Control: public, max-age=600` (D05).
- **UNWANTED BEHAVIOR**: IF a public request fails (rate limit, D1 unavailable, unknown route) THE SYSTEM SHALL return the standard error envelope with `Cache-Control: no-store`.
- **UNWANTED BEHAVIOR**: IF a POST, PUT, PATCH or DELETE request reaches a `client` master-data path THE SYSTEM SHALL return `405` with the standard error envelope and `Cache-Control: no-store`, without touching D1. (`OPTIONS` preflight keeps the SPEC-001 CORS behavior.)
- **UBIQUITOUS**: THE `client` Worker SHALL contain no code path that writes master-data tables (it shares the D1 binding with `admin`, so read-only is enforced in code).

**Schema**
- **UBIQUITOUS**: THE SYSTEM SHALL define the master-data tables in a new migration under `backend/migrations/`, containing DDL only.
- **UBIQUITOUS**: THE database SHALL itself enforce every reference in SPEC-002 §4.4 (daily entry → card, art entry → card, situation → category or `null`, suggestion → category or `null`, every love-status text → love status) with foreign keys and no cascade, and key uniqueness per table, so concurrent writes in SPEC-002b cannot leave a dangling reference.
- **UBIQUITOUS**: THE SYSTEM SHALL store love-status texts (`meanings`, `details`, `love_by_status`) as child rows keyed by card and love status.

**Seed (D07)**
- **UBIQUITOUS**: THE SYSTEM SHALL keep seed data in `backend/seed/`, with no data in migrations.
- **EVENT-DRIVEN**: WHEN the developer runs the seed command against an environment THE SYSTEM SHALL upsert every seed row by key, returning seeded rows to their seed values and leaving rows not in the seed untouched.
- **UBIQUITOUS**: THE seed SHALL write no `audit_events` rows.
- **UNWANTED BEHAVIOR**: IF the seed source contains a dropped field THE SYSTEM SHALL fail the seed build before touching D1.
- **UBIQUITOUS**: THE seed command SHALL be run through the Makefile, with the remote-environment targets marked `[CLOUD]` like the other targets that change Cloudflare state.

## 7. Acceptance Criteria
Scenario numbers 1–6, 27 and 28 are the SPEC-002 numbers. A1 and A2 are new.

- [ ] Scenario 1: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/cards` THEN it returns 78 cards whose fields and values equal `reading-cards.json` after the keep/drop table and English-key mapping, in file order, AND the response has `Cache-Control: public, max-age=600`.
- [ ] Scenario 2: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/daily-cards` THEN it returns the metadata and 78 daily cards equal to `daily-cards.json` after the keep/drop table, in file order, with 22 `null` `complicated` texts and matching `missing_fields`, AND no response contains `chatgpt.com`, `source_url` or `raw_file`.
- [ ] Scenario 3: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/card-art` THEN it returns 74 entries whose `art`, `width`, `height`, `renderedWidth`, `renderedHeight`, `sha256` and `bytes` equal `v3/manifest.json`, AND `six-of-wands`, `nine-of-wands`, `page-of-pentacles`, `king-of-pentacles` are absent and have `art: null` in `GET /v1/cards`.
- [ ] Scenario 4: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/reading-config` THEN it returns 4 categories; 9 non-love situations (3 each for work, money, life) plus 6 love situations; 12 suggestions with IDs `love-1` … `life-3`; 3 positions; 6 love statuses with both labels in `reading.ts` order; empty `uncategorized_situations` and `uncategorized_suggestions`, AND `GET /v1/daily-cards` `love_statuses` keep the daily file's order.
- [ ] Scenario 5: GIVEN D1 is unavailable WHEN a visitor requests any master-data endpoint THEN the response is `503` with the error envelope and `Cache-Control: no-store`.
- [ ] Scenario 6: GIVEN the client Worker WHEN a `POST` is sent to `/v1/cards` THEN the response is `405` with the error envelope and `Cache-Control: no-store`, and D1 is unchanged.
- [ ] Scenario 27: GIVEN a seeded D1 plus one suggestion row inserted directly with a UUIDv4 id (standing in for an admin-created one) WHEN the developer runs the seed again THEN seeded rows equal the seed values, the extra suggestion still exists, and no `audit_events` rows were added by the seed.
- [ ] Scenario 28: GIVEN `backend/migrations/` WHEN it is inspected THEN it contains DDL only, no seed INSERTs.
- [ ] Scenario A1: GIVEN a seeded D1 in which, directly in the database, the `work` situations and suggestions are set to `category = null`, the three `love` suggestions are deleted (the foreign key forbids deleting `love` while they reference it), and the `work` and `love` category rows are deleted, WHEN a visitor requests `GET /v1/reading-config` THEN there is no `work` or `love` category, `uncategorized_situations` holds the 3 work situations (by `sort_order`, then key) followed by the 6 love situations (by `reading_sort_order`), and `uncategorized_suggestions` holds the 3 work suggestions (by `sort_order`, then id).
- [ ] Scenario A2: GIVEN the migrations are applied WHEN a test inserts directly (a) a daily entry for card `x-card`, (b) a situation with category `nope`, (c) a love-status text for status `widowed`, and (d) a second card with an existing id THEN D1 rejects each statement with a constraint error and the tables are unchanged.

## 8. Scope Boundaries
- **In Scope**: the master-data migration; the seed source in `backend/seed/`, the seed build check, and Makefile seed targets; the four public GETs; `405` on `client` master-data writes; conformance to the client contract.
- **Out of Scope**:
  - All `admin` master-data endpoints, payload validation (SPEC-002 §4.5), audit rows for writes, the daily `updated`/`version` bump: SPEC-002b.
  - Switching `frontend-client` to the API: separate frontend work after this spec reaches staging (D11).
  - Image hosting and R2; server-side shuffle, 3+1 sessions, reading engine (`TODO-003`); pagination (`TODO-010`); cache invalidation (`TODO-011`).
- **Modular Split**: this is the first child of SPEC-002; the sibling is [SPEC-002b](SPEC-002b-master-data-admin-write.md).

## 9. Resolved Items
1. Read/write boundary, order, frontend placement and complexity: D11 (owner, 2026-11-25).
2. Order inside `uncategorized_situations` when both kinds exist (SPEC-002 left it open): regular uncategorized situations first, then love-derived ones: D11 item 6 (owner, 2026-11-25).
3. Contract review (D11 item 5): done 2026-11-25, recorded in D12. A write method on a known path returns `405` with `Allow`; an unknown path returns `404` for any method. Contract bumped to v0.2.1.
4. Scenario A1 fixture (found while planning, 2026-11-25): the fixture also deletes the three `love` suggestions, because the no-cascade foreign key rejects deleting the `love` category while they reference it. The expected result is unchanged.
5. Everything else inherited unchanged from SPEC-002 §9 (keep/drop table, situation keys, `409` with no cascade, Bangkok date, uncached admin reads, audit rounds 1–3).

## 10. Spectre Auditor Review & Sign-Off Checklist
- **Auditor**: Spectre (`.gb-sdlc/agents/spectre.md`)
- **Spectre Audit Verdict**: PASSED (split audit 2026-11-25). Traceability against SPEC-002: 32/32 requirements and 29/29 scenarios assigned to exactly one sub-spec (table in D11). One gap found and resolved with the owner: order inside `uncategorized_situations` (D11 item 6).
- [x] **Zero Assumptions Verified**
- [x] **Interrogation Audit**
- [x] **EARS Behavioral Syntax**
- [x] **Acceptance Testability**
- [x] **Scope Watchdog & Split-Spec Check**
- [x] **Security Pre-Check**
- **Notes for the owner's contract review (D11 item 5)** (resolved in D12, 2026-11-25): the client contract says writes on *any* path return `405` and unknown paths `404`; this spec requires `405` on master-data paths, so state which wins for a POST to an unknown path. Its `servers` list still names `api-staging.maemorkratai.com`; staging actually runs on `*.workers.dev`.
- **Auditor Sign-Off**: Spectre (`PASSED` on 2026-11-25)
- **Human Sign-Off**: Owner (`APPROVED` on 2026-11-25)
