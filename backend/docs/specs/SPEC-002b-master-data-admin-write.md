---
id: SPEC-002b
title: "Master Data: Admin Read and Write API"
status: APPROVED # DRAFT | SPECTRE_REVIEW | APPROVED | IMPLEMENTING | COMPLETED
spectre_audit: PASSED # PENDING | PASSED
complexity: complex # inherited from SPEC-002: 36 admin operations over 10 resources, reference checks, audit logging
created_at: "2026-11-25"
updated_at: "2026-11-25"
parent_spec: "SPEC-002"
sub_specs: []
---

# Specification: Master Data – Admin Read and Write API

> Second half of [SPEC-002](SPEC-002-master-data-to-backend.md), split by [D11](../decisions/D11.md). Starts after [SPEC-002a](SPEC-002a-master-data-schema-seed-public-read.md) is verified; it uses the schema and seed SPEC-002a delivers.

## 1. Intent & High-Level Overview
Authenticated admins (the owner and the reader, role `admin` or higher) read and change the master data through `admin`, with every change validated, checked against references inside the same database operation, and audited. Customers see the changes through the SPEC-002a GETs after the 10-minute cache (D05).

No admin UI is part of this spec; the endpoints prepare for `frontend-admin`. Actors: [actors-and-roles](../knowledges/actors-and-roles.md).

## 2. Research & Spikes (Linked)
- [Research: Workers cache invalidation](../researches/2026-11-20-workers-cache-invalidation.md)

## 3. Key Decisions (Linked)
- D01–D04 (SPEC-001): `docs/admin/decisions/`. D03 admin auth and roles.
- [D05: 10-minute public cache TTL, no invalidation yet](../decisions/D05.md)
- [D06: Admin writes are last-write-wins](../decisions/D06.md)
- [D08: English keys, Thai labels kept exactly per screen](../decisions/D08.md)
- [D09: Master-data integrity rules](../decisions/D09.md)
- [D10: Deleting a category leaves its items uncategorized](../decisions/D10.md)
- [D11: Split into SPEC-002a / SPEC-002b](../decisions/D11.md)

## 4. Data in Scope
[SPEC-002 §4.1–4.5](SPEC-002-master-data-to-backend.md#4-data-in-scope) is normative, including the integrity rules (§4.4) and the validation limits (§4.5). The tables come from the SPEC-002a migration. A schema change needed here requires a new decision and a new migration; it never edits the SPEC-002a migration.

## 5. API Surface (contract in `docs/api/`)
- `../docs/api/admin/openapi.yaml` (v0.2.0): `/v1/admin/cards`, `/v1/admin/daily-cards`, `/v1/admin/daily-metadata`, `/v1/admin/card-art`, `/v1/admin/reading-config`, `/v1/admin/categories`, `/v1/admin/situations`, `/v1/admin/suggestions`, `/v1/admin/positions`, `/v1/admin/love-statuses`.
- Role `admin` or higher, `Cache-Control: no-store`.
- Create, update (full replace) and delete for: cards, daily card entries, card art entries, categories, situations, suggested questions, positions, love statuses; update for daily metadata.
- Uncached reads that return the same shapes the writes accept (including sort orders and suggestion categories), so a record can be read, edited and sent back.
- The owner reviews the contract against the planned admin screens while planning (D11).

## 6. Behavioral Requirements (EARS Syntax)

**Admin read**
- **UBIQUITOUS**: THE SYSTEM SHALL serve admin master-data GETs uncached (`Cache-Control: no-store`) in the same shapes the corresponding writes accept, so a GET body can be sent back unchanged in a PUT.

**Admin write** (unchanged from SPEC-002 §6)
- **EVENT-DRIVEN**: WHEN an authenticated admin creates, updates or deletes a master-data record THE SYSTEM SHALL persist the change and write one `audit_events` row in one atomic D1 batch.
- **UBIQUITOUS**: THE SYSTEM SHALL set `audit_events.event_type` to `master_data.<entity>.<action>` (`action` is `create`, `update` or `delete`) and `target_id` to the record key.
- **UBIQUITOUS**: THE SYSTEM SHALL record in `audit_events.metadata_json` the entity, record key, action and the names of changed fields computed against the stored row (all fields on create, none on delete, an empty list on an unchanged PUT, which is still audited), never the text values.
- **UBIQUITOUS**: THE SYSTEM SHALL apply concurrent writes to the same record as last-write-wins (D06).
- **UBIQUITOUS**: THE SYSTEM SHALL enforce every reference and key uniqueness (SPEC-002 §4.4) inside the same atomic D1 operation as the write, and SHALL map a violation to `409` (duplicate key or referenced delete) or `400` `INVALID_REFERENCE` (missing reference), never to a `5xx`.
- **EVENT-DRIVEN**: WHEN an admin creates, updates or deletes a daily card entry, updates daily metadata, or creates, updates or deletes a love status THE SYSTEM SHALL in the same batch set daily `updated` to the current date in `Asia/Bangkok` (`YYYY-MM-DD`) and set the trailing `YYYY-MM-DD` of `version` to that date, appending `-YYYY-MM-DD` if `version` has no trailing date (e.g. `daily-v1-draft-2026-10-20` becomes `daily-v1-draft-2026-11-21`).
- **UNWANTED BEHAVIOR**: IF the request has no valid access token THE SYSTEM SHALL return `401` and change nothing.
- **UNWANTED BEHAVIOR**: IF the user's role is below `admin` or the account is inactive THE SYSTEM SHALL return `403` and change nothing. (Logging of denied requests is inherited from SPEC-001.)
- **UNWANTED BEHAVIOR**: IF the payload fails validation (SPEC-002 §4.5: unknown field, wrong type, missing required field, text length, pattern, integer bounds, control characters) THE SYSTEM SHALL return `400` with the error envelope and change nothing.
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
- **UNWANTED BEHAVIOR**: IF D1 is unavailable or any statement of an admin write batch fails THE SYSTEM SHALL return `503` with the error envelope and `Cache-Control: no-store`, and persist none of the change, the audit row or daily `updated`/`version`.
- **UBIQUITOUS**: THE SYSTEM SHALL assign a server-generated UUIDv4 id to admin-created suggestions (SPEC-002 §4.3).
- **UBIQUITOUS**: THE SYSTEM SHALL ignore `missing_fields` in daily-entry writes (it is computed, SPEC-002 §4.4), so a GET body can be PUT back unchanged.

## 7. Acceptance Criteria
Scenario numbers are the SPEC-002 numbers.

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
- [ ] Scenario 29: GIVEN card `the-fool` with an art entry WHEN an admin PUTs the card with `art` `null` or `/cards/v3/the-fool-new.webp` THEN the response is `409`, and the card and its art entry are unchanged.

## 8. Scope Boundaries
- **In Scope**: all admin master-data endpoints in the admin contract (reads and writes), validation, reference and key errors, audit rows, daily metadata bump, D10 category delete; contract updates to `docs/api/admin/openapi.yaml`.
- **Out of Scope**: schema, seed and public GETs (SPEC-002a); admin UI; image hosting and R2; reading engine (`TODO-003`); pagination (`TODO-010`); cache invalidation (`TODO-011`).
- **Modular Split**: second child of SPEC-002; the sibling is [SPEC-002a](SPEC-002a-master-data-schema-seed-public-read.md).

## 9. Resolved Items
1. Read/write boundary and order: D11 (owner, 2026-11-25).
2. Everything else inherited unchanged from SPEC-002 §9.

## 10. Spectre Auditor Review & Sign-Off Checklist
- **Auditor**: Spectre (`.gb-sdlc/agents/spectre.md`)
- **Spectre Audit Verdict**: PASSED (split audit 2026-11-25). Traceability against SPEC-002: 32/32 requirements and 29/29 scenarios assigned to exactly one sub-spec (table in D11). One gap found and resolved with the owner: order inside `uncategorized_situations` (D11 item 6).
- [x] **Zero Assumptions Verified**
- [x] **Interrogation Audit**
- [x] **EARS Behavioral Syntax**
- [x] **Acceptance Testability**
- [x] **Scope Watchdog & Split-Spec Check**
- [x] **Security Pre-Check**
- **Notes**: requirements and scenarios are unchanged from SPEC-002 except two EARS lines made explicit from SPEC-002 §4.3/§4.4 (server-generated suggestion ids, `missing_fields` ignored on write) and the admin-read requirement taken from §5.
- **Auditor Sign-Off**: Spectre (`PASSED` on 2026-11-25)
- **Human Sign-Off**: Owner (`APPROVED` on 2026-11-25)
