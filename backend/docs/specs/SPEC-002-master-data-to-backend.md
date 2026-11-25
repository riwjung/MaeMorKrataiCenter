---
id: SPEC-002
title: "Move Frontend Master Data to the Backend"
status: APPROVED # DRAFT | SPECTRE_REVIEW | APPROVED | IMPLEMENTING | COMPLETED
spectre_audit: PASSED # PENDING | PASSED
complexity: complex # confirmed by developer 2026-11-21: 36 admin operations over 10 resources, reference checks, audit logging, seed
created_at: "2026-11-21"
updated_at: "2026-11-25"
parent_spec: ""
sub_specs: [SPEC-002a, SPEC-002b]
---

# Specification: Move Frontend Master Data to the Backend

> **Split on 2026-11-25 ([D11](../decisions/D11.md))** into [SPEC-002a](SPEC-002a-master-data-schema-seed-public-read.md) (schema, seed, public read) and [SPEC-002b](SPEC-002b-master-data-admin-write.md) (admin read and write). This file stays the normative definition of the data (§4) and the record of the original audit. Plan and execute the sub-specs, not this file.

> Single spec for both services (`client` and `admin`). Folds in the old SPEC-002 "cards catalog & daily reading" item from `docs/admin/specs/OUT_OF_SCOPE.md` and `docs/client/specs/OUT_OF_SCOPE.md`.

## 1. Intent & High-Level Overview
`frontend-client` bundles its master data as static JSON and TypeScript constants. This spec moves that data into the shared D1 database:
- `client` serves it through public, read-only, cached GET endpoints.
- `admin` lets authenticated admins create, update and delete it.

The frontend's current features are the business requirement: the backend reproduces the same data, with the same structure, minus internal provenance fields.

- **Customer** (anonymous): reads the data through `client`. No accounts.
- **Admin** (owner and reader, role `admin`): edits the data through `admin` with the existing SPEC-001 JWT auth. No admin UI exists yet; the endpoints prepare for it.

Card images stay in `frontend-client/public/`. The API returns the same relative art paths the frontend uses today (e.g. `/cards/v3/the-fool-e11e1ee168a4.webp`). No R2 work. Card shuffling and drawing stay in the browser.

See [actors-and-roles](../knowledges/actors-and-roles.md).

## 2. Research & Spikes (Linked)
- [Research: Workers cache invalidation](../researches/2026-11-20-workers-cache-invalidation.md)

## 3. Key Decisions (Linked)
- D01–D04 (SPEC-001): `docs/admin/decisions/`. D03 admin auth and roles; D04 public `client` security baseline.
- [D05: 10-minute public cache TTL, no invalidation yet](../decisions/D05.md)
- [D06: Admin writes are last-write-wins](../decisions/D06.md)
- [D07: Migrations hold schema only; seed loaded manually](../decisions/D07.md)
- [D08: English keys, Thai labels kept exactly per screen](../decisions/D08.md)
- [D09: Master-data integrity rules](../decisions/D09.md)
- [D10: Deleting a category leaves its items uncategorized](../decisions/D10.md)

## 4. Data in Scope

### 4.1 Sources (copied once into `backend/seed/`)
| Source in `frontend-client` | Contents |
|---|---|
| `src/lib/reading-cards.json` | 78 cards: name, keyword, art path, short love meanings and long love details per status |
| `src/lib/daily-cards.json` | Daily one-card content for 78 cards, plus file metadata and the love-status list |
| `public/cards/v3/manifest.json` | Art sizes for 74 cards (4 cards have no art) |
| `src/lib/reading.ts` | Categories, situations per category, suggested questions per category, 3-card positions, love statuses for the 3-card screen |

Excluded: `src/lib/cards.json` (unused pilot), `public/cards/manifest.json` (pilot images), `public/cards/deck.js` (scene iframe duplicate).

### 4.2 Field keep/drop table (confirmed 2026-11-21)
| File | Field | Keep? | Note |
|---|---|---|---|
| reading-cards | `id`, `name`, `keyword`, `art` | Keep | `art` is `null` for 4 cards |
| reading-cards | `meanings`, `details` | Keep | Map keys become English love-status keys (D08) |
| reading-cards | `source_url`, `source_message`, `source_line`, `review_status`, `details_status` | Drop | Confirmed: internal provenance and status |
| daily (top) | `schema_version`, `version`, `updated`, `status`, `scope`, `auto_fallback_to_general_or_spread`, `love_statuses` | Keep | Confirmed: "match that first" |
| daily (card) | `card_id`, `name`, `arcana`, `reading_mode`, `time_horizon`, `orientation`, `content_status`, `approved_for_web`, `content.*` | Keep | Confirmed |
| daily (card) | `missing_fields` | Keep | Lists gaps explicitly (e.g. 22 Major Arcana lack `complicated`) |
| daily (card) | `variants` (`summary_theme`, `overview_theme`) | Keep | Display content |
| daily (card) | `source` (ChatGPT URL, `raw_file`, `message_id`, ...) | Drop | Same kind of provenance as `source_*` |
| v3 manifest | `cards[].id`, `art`, `width`, `height`, `renderedWidth`, `renderedHeight` | Keep | `card-art.ts` uses `renderedWidth`/`renderedHeight` |
| v3 manifest | `cards[].sha256`, `bytes` | Keep | Image integrity info, harmless |
| v3 manifest | `cards[].sourceFile`, `cards[].sourceSha256` | Drop | Build-time provenance |
| v3 manifest (top) | `version`, `capturedAt`, `selection`, `duplicates`, `ignored` | Drop | Build-time provenance |

### 4.3 English keys (D08)
Love statuses (from `docs/context/glossary.md`):

| Key | Daily label | 3-card label |
|---|---|---|
| `single` | คนโสด | คนโสด |
| `crush` | คนแอบชอบ | แอบชอบ |
| `talking` | คนคุย | คุยกัน |
| `complicated` | ความสัมพันธ์ซับซ้อน | ซับซ้อน |
| `partnered` | คนมีแฟน | มีแฟน |
| `ex_partner` | คนรักเก่า | เลิกกัน |

Categories keep the frontend keys `love`, `work`, `money`, `life`. Positions: `past` (อดีต), `present` (ปัจจุบัน), `future` (อนาคต). Situations for `love` are the love statuses above (3-card label); they are managed through love statuses, not situations.

Situations for work, money and life (confirmed 2026-11-21):

| Category | Key | Label |
|---|---|---|
| work | `current_job` | งานปัจจุบัน |
| work | `job_seeking` | มองหางานใหม่ |
| work | `own_business` | ธุรกิจของตัวเอง |
| money | `income_expenses` | รายรับและรายจ่าย |
| money | `saving_plan` | วางแผนเก็บเงิน |
| money | `long_term_goals` | เป้าหมายระยะยาว |
| life | `in_transition` | กำลังเปลี่ยนแปลง |
| life | `facing_decision` | มีเรื่องต้องตัดสินใจ |
| life | `self_reflection` | อยากทบทวนตัวเอง |

Suggested questions are an ordered list per category (3 each, 12 total). The seed assigns stable IDs `<category>-<n>` (`love-1` … `life-3`) so re-seeding upserts instead of duplicating. Admin-created suggestions get a server-generated UUIDv4, so they never collide with seed IDs.

**Ordering.** The two screens order things differently, so order is stored per screen:
- Love statuses have `daily_sort_order` (daily file: single, crush, talking, partnered, ex_partner, complicated) and `reading_sort_order` (`reading.ts`: single, crush, talking, complicated, partnered, ex_partner).
- Cards (`reading-cards.json` order) and daily card entries (`daily-cards.json` order) each have their own `sort_order`; the two files list the 78 cards in different orders.

### 4.4 Integrity rules (D09)
- **References.** Daily entry → card; art entry → card; situation → category or `null`; suggestion → category or `null`; every key of `meanings`, `details` and `love_by_status` → love status. Each is enforced by the database inside the write itself (love-status texts are stored as child rows with foreign keys, no cascade), so concurrent requests cannot leave a dangling reference.
- **Keys are immutable.** A PUT cannot rename a record; renaming is create new, move dependents, delete old. A situation or suggestion may move to another existing non-`love` category, or to `null` (uncategorized, D10).
- **Love-status maps.** `meanings`, `details` and `love_by_status` always return every current love-status key. A key with no stored text returns `null` (e.g. after a new love status is created, or the 22 Major Arcana `complicated` daily texts). Writes may omit a key or send `null` to mean "no text".
- **`missing_fields`** is computed by the server from `null` daily texts (`content.love_by_status.<key>`); it is optional and ignored if sent in an admin write (so a GET body can be PUT back unchanged).
- **Art consistency.** An art entry's `art` must equal its card's `art`; a card with `art: null` cannot have an art entry.
- **Uncategorized (D10).** Deleting a category (including `love`) sets `category = null` on its situations and suggestions in the same batch. Uncategorized items stay visible publicly in `uncategorized_situations` / `uncategorized_suggestions`. While no `love` category exists, the love situations derived from love statuses are listed as uncategorized. Both uncategorized lists are ordered by `sort_order`, then key/id; love-derived situations use `reading_sort_order`.
- **Approval flag.** The public daily endpoint serves entries regardless of `approved_for_web` (all 78 are `false` today), as the frontend does now.

### 4.5 Validation limits (admin writes)
- Request body: existing admin cap `MAX_BODY_BYTES` = 65,536 bytes (SPEC-001), answered with `413 PAYLOAD_TOO_LARGE`. A daily entry with every long field at the maximum can exceed it; that is accepted (largest real entry is about 9.6 KB).
- "Character" means Unicode code point. "Not empty" means non-empty after trimming whitespace.
- Short text (name, keyword, label, icon, hint, colors, status strings, love `meanings`): 1–200 characters.
- Long text (`details`, daily `content` fields, `love_by_status`): 1–2,000 characters, or `null` where a love-status map allows it (largest today: 640).
- Text is stored and returned verbatim as plain text; control characters other than newline (`\n`) are rejected. Clients must render it as text, never as HTML.
- `art`: `null` or matches `^/cards/v3/[a-z0-9-]+\.webp$`, at most 200 characters.
- Integers: `width`, `height`, `renderedWidth`, `renderedHeight`, `bytes` ≥ 1; `sort_order`, `daily_sort_order`, `reading_sort_order` 0–10,000; `schema_version` ≥ 1.
- Keys: `^[a-z][a-z0-9_]{0,39}$`; card IDs: `^[a-z0-9]+(-[a-z0-9]+)*$`, at most 64 characters; `sha256`: 64 lowercase hex characters.
- Unknown JSON fields in a write body are rejected with `400`.

## 5. API Surface (contract in `docs/api/`)
Contract v0.2.0, drafted 2026-11-21. Both files need frontend review in a PR.
- `../docs/api/client/openapi.yaml`: `GET /v1/cards`, `/v1/daily-cards`, `/v1/card-art`, `/v1/reading-config`.
- `../docs/api/admin/openapi.yaml`: `/v1/admin/cards`, `/v1/admin/daily-cards`, `/v1/admin/daily-metadata`, `/v1/admin/card-art`, `/v1/admin/reading-config`, `/v1/admin/categories`, `/v1/admin/situations`, `/v1/admin/suggestions`, `/v1/admin/positions`, `/v1/admin/love-statuses`.

**`client` (public, no auth, D04 rate limit, `Cache-Control: public, max-age=600`):**
- Cards: the reading-cards array (all 78, no pagination).
- Daily cards: the daily-cards document (metadata + 78 cards, no pagination).
- Card art manifest: art sizes per card.
- Reading config: categories, situations, suggested questions, positions, love statuses with both label sets.

**`admin` (role `admin` or higher, `Cache-Control: no-store`):**
- Create, update (full replace) and delete for: cards, daily card entries, card art entries, categories, situations, suggested questions, positions, love statuses; update for daily metadata.
- Uncached reads that return the same shapes the writes accept (including sort orders and suggestion categories), so a record can be read, edited and sent back.

## 6. Behavioral Requirements (EARS Syntax)

**Public read (`client`)**
- **UBIQUITOUS**: THE SYSTEM SHALL serve master data from D1 with the same structure as the source files, using English keys (D08) and without dropped fields (4.2).
- **UBIQUITOUS**: THE SYSTEM SHALL return `art` paths exactly as stored, and `null` where a card has no art.
- **UBIQUITOUS**: THE SYSTEM SHALL return every current love-status key in `meanings`, `details` and `love_by_status`, with `null` where no text is stored.
- **EVENT-DRIVEN**: WHEN a public master-data GET succeeds THE SYSTEM SHALL return `200` with `Cache-Control: public, max-age=600` (D05).
- **UNWANTED BEHAVIOR**: IF a public request fails (rate limit, D1 unavailable, unknown route) THE SYSTEM SHALL return the standard error envelope with `Cache-Control: no-store`.
- **UNWANTED BEHAVIOR**: IF a POST, PUT, PATCH or DELETE request reaches a `client` master-data path THE SYSTEM SHALL return `405` with the standard error envelope and `Cache-Control: no-store`, without touching D1. (`OPTIONS` preflight keeps the SPEC-001 CORS behavior.)
- **UBIQUITOUS**: THE `client` Worker SHALL contain no code path that writes master-data tables (it shares the D1 binding with `admin`, so read-only is enforced in code).

**Admin write (`admin`)**
- **EVENT-DRIVEN**: WHEN an authenticated admin creates, updates or deletes a master-data record THE SYSTEM SHALL persist the change and write one `audit_events` row in one atomic D1 batch.
- **UBIQUITOUS**: THE SYSTEM SHALL set `audit_events.event_type` to `master_data.<entity>.<action>` (`action` is `create`, `update` or `delete`) and `target_id` to the record key.
- **UBIQUITOUS**: THE SYSTEM SHALL record in `audit_events.metadata_json` the entity, record key, action and the names of changed fields computed against the stored row (all fields on create, none on delete, an empty list on an unchanged PUT, which is still audited), never the text values.
- **UBIQUITOUS**: THE SYSTEM SHALL apply concurrent writes to the same record as last-write-wins (D06).
- **UBIQUITOUS**: THE SYSTEM SHALL enforce every reference and key uniqueness (4.4) inside the same atomic D1 operation as the write, and SHALL map a violation to `409` (duplicate key or referenced delete) or `400` `INVALID_REFERENCE` (missing reference), never to a `5xx`.
- **EVENT-DRIVEN**: WHEN an admin creates, updates or deletes a daily card entry, updates daily metadata, or creates, updates or deletes a love status THE SYSTEM SHALL in the same batch set daily `updated` to the current date in `Asia/Bangkok` (`YYYY-MM-DD`) and set the trailing `YYYY-MM-DD` of `version` to that date, appending `-YYYY-MM-DD` if `version` has no trailing date (e.g. `daily-v1-draft-2026-10-20` becomes `daily-v1-draft-2026-11-21`).
- **UNWANTED BEHAVIOR**: IF the request has no valid access token THE SYSTEM SHALL return `401` and change nothing.
- **UNWANTED BEHAVIOR**: IF the user's role is below `admin` or the account is inactive THE SYSTEM SHALL return `403` and change nothing. (Logging of denied requests is inherited from SPEC-001.)
- **UNWANTED BEHAVIOR**: IF the payload fails validation (4.5: unknown field, wrong type, missing required field, text length, pattern, integer bounds, control characters) THE SYSTEM SHALL return `400` with the error envelope and change nothing.
- **UNWANTED BEHAVIOR**: IF the body exceeds `MAX_BODY_BYTES` THE SYSTEM SHALL return `413` with code `PAYLOAD_TOO_LARGE` and change nothing.
- **UNWANTED BEHAVIOR**: IF a create or update names a card, category or love-status key that does not exist THE SYSTEM SHALL return `400` with code `INVALID_REFERENCE` and change nothing.
- **UNWANTED BEHAVIOR**: IF a situation create or update has `category` `love` THE SYSTEM SHALL return `400` and change nothing.
- **UNWANTED BEHAVIOR**: IF an art entry's `art` differs from its card's `art`, or its card has `art: null`, THE SYSTEM SHALL return `400` and change nothing.
- **UNWANTED BEHAVIOR**: IF a card update changes `art` (including to `null`) while the card has an art entry THE SYSTEM SHALL return `409` and change nothing; the art entry must be deleted first.
- **UNWANTED BEHAVIOR**: IF a PUT body's `id`, `card_id` or `key` differs from the path key THE SYSTEM SHALL return `400` and change nothing.
- **UNWANTED BEHAVIOR**: IF a create uses a key that already exists THE SYSTEM SHALL return `409` and change nothing.
- **UNWANTED BEHAVIOR**: IF an update or delete targets a key that does not exist THE SYSTEM SHALL return `404`.
- **UNWANTED BEHAVIOR**: IF a delete targets a card with a daily or art entry, or a love status with any stored text, THE SYSTEM SHALL return `409` and change nothing; dependents must be deleted first.
- **EVENT-DRIVEN**: WHEN an admin deletes a category THE SYSTEM SHALL in the same atomic batch set `category` to `null` on its situations and suggestions and write one `audit_events` row per moved record (`update`, changed field `category`) besides the category's `delete` row (D10).
- **STATE-DRIVEN**: WHILE no category `love` exists THE SYSTEM SHALL list the situations derived from love statuses in `uncategorized_situations`.
- **UNWANTED BEHAVIOR**: IF D1 is unavailable or any statement of an admin write batch fails THE SYSTEM SHALL return `503` with the error envelope and `Cache-Control: no-store`, and persist none of the change, the audit row or daily `updated`/`version`.

**Seed (D07)**
- **UBIQUITOUS**: THE SYSTEM SHALL keep table definitions in `backend/migrations/` and seed data in `backend/seed/`, with no data in migrations.
- **EVENT-DRIVEN**: WHEN the developer runs the seed command against an environment THE SYSTEM SHALL upsert every seed row by key, returning seeded rows to their seed values and leaving rows not in the seed untouched.
- **UBIQUITOUS**: THE seed SHALL write no `audit_events` rows.
- **UNWANTED BEHAVIOR**: IF the seed source contains a dropped field THE SYSTEM SHALL fail the seed build before touching D1.

## 7. Acceptance Criteria
- [ ] Scenario 1: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/cards` THEN it returns 78 cards whose fields and values equal `reading-cards.json` after the keep/drop table and English-key mapping, in file order, AND the response has `Cache-Control: public, max-age=600`.
- [ ] Scenario 2: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/daily-cards` THEN it returns the metadata and 78 daily cards equal to `daily-cards.json` after the keep/drop table, in file order, with 22 `null` `complicated` texts and matching `missing_fields`, AND no response contains `chatgpt.com`, `source_url` or `raw_file`.
- [ ] Scenario 3: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/card-art` THEN it returns 74 entries whose `art`, `width`, `height`, `renderedWidth`, `renderedHeight`, `sha256` and `bytes` equal `v3/manifest.json`, AND `six-of-wands`, `nine-of-wands`, `page-of-pentacles`, `king-of-pentacles` are absent and have `art: null` in `GET /v1/cards`.
- [ ] Scenario 4: GIVEN a seeded D1 WHEN a visitor requests `GET /v1/reading-config` THEN it returns 4 categories; 9 non-love situations (3 each for work, money, life) plus 6 love situations; 12 suggestions with IDs `love-1` … `life-3`; 3 positions; 6 love statuses with both labels in `reading.ts` order; empty `uncategorized_situations` and `uncategorized_suggestions`, AND `GET /v1/daily-cards` `love_statuses` keep the daily file's order.
- [ ] Scenario 5: GIVEN D1 is unavailable WHEN a visitor requests any master-data endpoint THEN the response is `503` with the error envelope and `Cache-Control: no-store`.
- [ ] Scenario 6: GIVEN the client Worker WHEN a `POST` is sent to `/v1/cards` THEN the response is `405` with the error envelope and `Cache-Control: no-store`, and D1 is unchanged.
- [ ] Scenario 7: GIVEN an admin access token WHEN the admin updates a card's keyword THEN the change is stored, one `audit_events` row has `event_type` `master_data.card.update`, `target_id` the card id, actor the admin, changed fields `["keyword"]`, AND the response has `Cache-Control: no-store`.
- [ ] Scenario 8: GIVEN no token WHEN a write is sent to an admin master-data endpoint THEN the response is `401` and D1 is unchanged.
- [ ] Scenario 9: GIVEN a token with role `user` WHEN a write is sent to an admin master-data endpoint THEN the response is `403` and D1 is unchanged.
- [ ] Scenario 10: GIVEN an inactive admin account WHEN it sends a write THEN the response is `403` and D1 is unchanged.
- [ ] Scenario 11: GIVEN an existing card id WHEN an admin creates a card with the same id THEN the response is `409` and D1 is unchanged.
- [ ] Scenario 12: GIVEN invalid payloads (empty name, a `details` text of 2,001 characters, an unknown JSON field, `art` `https://evil.example/x.webp`, a name containing `\u0007`, `sort_order` 10,001) WHEN an admin sends each THEN each response is `400` and D1 is unchanged.
- [ ] Scenario 13: GIVEN a body of 65,537 bytes WHEN an admin sends it THEN the response is `413` `PAYLOAD_TOO_LARGE` and D1 is unchanged.
- [ ] Scenario 14: GIVEN no card `x-card` WHEN an admin creates a daily entry with `card_id` `x-card` THEN the response is `400` `INVALID_REFERENCE`, AND the daily entry count and daily `version` are unchanged. The same holds for an art entry for `x-card`, a situation with category `nope`, and a card whose `meanings` has key `widowed`.
- [ ] Scenario 15: GIVEN a situation create with `category` `love` WHEN an admin sends it THEN the response is `400` and D1 is unchanged.
- [ ] Scenario 16: GIVEN category `work` WHEN an admin PUTs `/v1/admin/categories/work` with `key` `career` THEN the response is `400`, `work` still exists and `career` does not.
- [ ] Scenario 17: GIVEN card `the-fool` with `art` `/cards/v3/the-fool-e11e1ee168a4.webp` WHEN an admin PUTs its art entry with `art` `/cards/v3/other.webp` THEN the response is `400` and D1 is unchanged.
- [ ] Scenario 18: GIVEN no category `nope` WHEN an admin PUTs or DELETEs `/v1/admin/categories/nope` THEN the response is `404`.
- [ ] Scenario 19: GIVEN a card that has a daily entry WHEN an admin deletes the card THEN the response is `409` and the card and its daily entry are unchanged, AND after deleting its daily entry and art entry the card delete returns `204`.
- [ ] Scenario 20: GIVEN love status `single` with stored texts WHEN an admin deletes it THEN the response is `409` and D1 is unchanged.
- [ ] Scenario 21: GIVEN category `work` with 3 situations and 3 suggestions WHEN an admin deletes it THEN the response is `204`, `GET /v1/reading-config` has no `work` category, its 3 situations are in `uncategorized_situations` and its 3 suggestions in `uncategorized_suggestions`, AND 7 `audit_events` rows were written (1 delete, 6 updates).
- [ ] Scenario 22: GIVEN a seeded D1 WHEN an admin deletes category `love` THEN the 6 love situations appear in `uncategorized_situations`, cards still return their love meanings, AND after creating category `love` again they appear under `situations.love`.
- [ ] Scenario 23: GIVEN a seeded D1 WHEN an admin creates love status `widowed` THEN every card in `GET /v1/admin/cards` has `meanings.widowed` `null`, and the daily `version` date is bumped.
- [ ] Scenario 24: GIVEN daily `version` `daily-v1-draft-2026-10-20` WHEN an admin updates any daily card on 2026-11-21 (Bangkok time) THEN daily `updated` is `2026-11-21` and `version` is `daily-v1-draft-2026-11-21`. GIVEN `version` `daily-v1` THEN it becomes `daily-v1-2026-11-21`.
- [ ] Scenario 25: GIVEN the audit insert is forced to fail WHEN an admin updates a card's keyword THEN the response is `503`, the keyword is unchanged and there is no new `audit_events` row.
- [ ] Scenario 26: GIVEN an admin reads a record from an admin GET WHEN they PUT the same body back unchanged THEN the response is `200`, the stored row is unchanged, and the audit row lists no changed fields.
- [ ] Scenario 27: GIVEN a seeded D1 plus one admin-created suggestion WHEN the developer runs the seed again THEN seeded rows equal the seed values, the admin-created suggestion still exists, and no `audit_events` rows were added by the seed.
- [ ] Scenario 28: GIVEN `backend/migrations/` WHEN it is inspected THEN it contains DDL only, no seed INSERTs.
- [ ] Scenario 29: GIVEN card `the-fool` with an art entry WHEN an admin PUTs the card with `art` `null` or `/cards/v3/the-fool-new.webp` THEN the response is `409`, and the card and its art entry are unchanged.

## 8. Scope Boundaries
- **In Scope**: D1 schema for the data in Section 4; the seed command and seed data in `backend/seed/`; public GET endpoints on `client`; admin create/update/delete and read endpoints on `admin` with audit logging; contract updates to `docs/api/client/openapi.yaml` and `docs/api/admin/openapi.yaml`, including the 10-minute cache note.
- **Out of Scope**:
  - Image hosting and R2 uploads (art stays in `frontend-client/public/`).
  - Server-side shuffle, draw, 3+1 sessions, reading engine and knowledge releases (`docs/context/09`–`11`).
  - Pagination (`TODO-010`).
  - Cache invalidation on edit (`TODO-011`).
  - Admin UI, customer accounts, payments.
  - Frontend changes (frontend owner switches imports to API calls and maps English keys).
- **Modular Split**: [SPEC-002a](SPEC-002a-master-data-schema-seed-public-read.md) and [SPEC-002b](SPEC-002b-master-data-admin-write.md) (D11, 2026-11-25). The developer first chose one spec (2026-11-21).

## 9. Resolved Items
1. Keep/drop table confirmed.
2. Situation keys confirmed.
3. Delete of a referenced record: `409`, no cascade.
4. Daily metadata auto-updated, date in `Asia/Bangkok`.
5. Uncached admin reads included.
6. Audit round 1 (2026-11-21): 6 blocking and 6 minor gaps; developer accepted all recommendations (D09).
7. Audit round 2 (2026-11-21): card PUT vs art entry fixed (409); category delete falls back to uncategorized, shown publicly (D10).

## 10. Spectre Auditor Review & Sign-Off Checklist
- **Auditor**: Spectre (`.gb-sdlc/agents/spectre.md`)
- **Spectre Audit Verdict**: PASSED (3 rounds: 12 gaps, then 5, then 3 minor, all resolved)
- [x] **Zero Assumptions Verified**
- [x] **Interrogation Audit**
- [x] **EARS Behavioral Syntax**
- [x] **Acceptance Testability**
- [x] **Scope Watchdog & Split-Spec Check**
- [x] **Security Pre-Check**
- **Auditor Sign-Off**: Spectre (`PASSED` on 2026-11-21)
- **Human Sign-Off**: Backend owner (`APPROVED` on 2026-11-21)
