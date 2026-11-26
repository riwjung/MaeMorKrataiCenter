# Master data

Master data is the content the tarot app reads but visitors never change: the 78 cards, the daily card content, the card art sizes, and the reading taxonomy (categories, situations, suggestions, positions and love statuses). It lives in the shared D1 database. The `client` Worker serves it read-only (SPEC-002a). The `admin` Worker will edit it (SPEC-002b).

## Schema

Source: `migrations/0002_master_data.sql`. The migration is DDL only. It is applied by the existing `make migrate-*` targets through the `admin` Worker's D1 binding.

The 12 tables, in `TABLE_ORDER` order (parents before children, the order the seed loads them):

| Table | Primary key | Foreign keys | Nullable columns |
| :--- | :--- | :--- | :--- |
| `love_statuses` | `key` | none | none |
| `categories` | `key` | none | none |
| `situations` | `key` | `category` to `categories.key` | `category` |
| `suggestions` | `id` | `category` to `categories.key` | `category` |
| `positions` | `key` | none | none |
| `cards` | `id` | none | `art` |
| `card_meanings` | `(card_id, love_status)` | `card_id` to `cards.id`, `love_status` to `love_statuses.key` | `text` |
| `card_details` | `(card_id, love_status)` | `card_id` to `cards.id`, `love_status` to `love_statuses.key` | `text` |
| `card_art` | `card_id` | `card_id` to `cards.id` | none |
| `daily_meta` | `id` | none | none |
| `daily_cards` | `card_id` | `card_id` to `cards.id` | `summary_theme`, `overview_theme` |
| `daily_love_texts` | `(card_id, love_status)` | `card_id` to `daily_cards.card_id`, `love_status` to `love_statuses.key` | `text` |

Rules:

- Foreign keys take no delete or update action (D09). D1 enforces them on every statement, so deleting a parent that a child still references fails.
- `category` is nullable on `situations` and `suggestions`. NULL means uncategorized (D10).
- Love situations are not stored. They are derived from `love_statuses` (the key and the `reading_label`).
- `missing_fields` is not stored. It is computed from NULL `daily_love_texts.text` values or from absent rows.
- Booleans are INTEGER 0/1 with CHECK constraints (`daily_meta.auto_fallback_to_general_or_spread`, `daily_cards.approved_for_web`).
- `daily_meta` has exactly one row, enforced by `CHECK (id = 1)`.
- `love_statuses` has two orders, `daily_sort_order` and `reading_sort_order`, one for each screen.
- Every column is NOT NULL unless the table above lists it as nullable.
- Indexes cover the foreign-key columns that are not the leading primary-key column: `idx_situations_category`, `idx_suggestions_category`, `idx_card_meanings_love_status`, `idx_card_details_love_status`, `idx_daily_love_texts_love_status`.

Art consistency (D09 item 5, an art entry's `art` equals its card's `art`) is checked by the seed build and, later, by SPEC-002b validation. The schema does not enforce it.

## Seed

Source: `seed/*.json`. Each file has the exact shape of one public `client` response in `docs/api/client/openapi.yaml` (v0.2.1):

| File | Mirrors |
| :--- | :--- |
| `seed/cards.json` | `GET /v1/cards` |
| `seed/daily.json` | `GET /v1/daily-cards` |
| `seed/card-art.json` | `GET /v1/card-art` |
| `seed/reading-config.json` | `GET /v1/reading-config` |

The files carry two derived fields, `missing_fields` and `situations.love`. They are checked by `seed-build` and not stored.

`seed/` is a one-off stripped copy of the frontend data (D07). It was produced by the conversion script in `docs/plan-records/SPEC-002a-p1-schema-seed/01_SEED_DATA_CONVERSION.md`. Never regenerate `seed/` from `../frontend-client`. A new conversion is a new plan.

`tools/seed-build` reads the four files and writes `seed/build/seed.sql`. The `seed/build/` directory is generated and ignored by git.

What `seed-build` rejects:

- Forbidden tokens anywhere in a seed file (`FORBIDDEN_TOKENS`): `chatgpt.com`, and the keys `"source_url"`, `"source_message"`, `"source_line"`, `"review_status"`, `"details_status"`, `"source"`, `"raw_file"`, `"sourceFile"`, `"sourceSha256"`, `"capturedAt"`, `"selection"`, `"duplicates"` and `"ignored"`.
- Unknown fields.
- Control characters other than newline in any string.
- Cross-file rules in `tools/seed-build/src/validate.rs`:
  - duplicate love status, category, position, situation, suggestion, card, art entry or daily card keys;
  - love statuses that differ between `reading-config.json` and `daily.json`, or a `daily.json` `source_label` that differs from the `daily_label` in `reading-config.json`;
  - non-empty `uncategorized_situations` or `uncategorized_suggestions`;
  - situations or suggestions that use an unknown category;
  - `situations.love` that is not equal to `love_statuses` (key and `reading_label`, in order);
  - a card whose `meanings` or `details` keys differ from the love statuses;
  - an art entry with no card, or whose `art` differs from its card's `art`;
  - a daily card with no card, or whose `love_by_status` keys differ from the love statuses;
  - `missing_fields` that differ from the null daily love texts.

What it emits:

- One upsert per row: `INSERT ... ON CONFLICT(<pk>) DO UPDATE SET` of every non-key column.
- Parents before children, in `TABLE_ORDER`.
- Sort orders `10, 20, 30, ...`, in file order, which leaves gaps for later inserts.
- Suggestion IDs `love-1` through `life-3`.
- Love-status texts that are `null` are stored as rows with `text = NULL`, so a re-seed resets them.
- 1,669 statements in total.
- No `DELETE`, no `INSERT OR REPLACE` and no writes to the audit table.

Re-seeding returns seeded rows to the seed values, so admin edits to seeded rows are lost. It leaves other rows untouched, such as admin-created suggestions with UUIDv4 IDs.

A local load run twice gives identical counts: 78 cards, 74 art entries, 22 null daily love texts, 12 suggestions and no seed audit rows.

## How to use

- `make migrate-local && make seed-local` creates the schema and loads the seed into the local D1 state `admin/.wrangler/state`.
- `make seed-build` alone validates the seed and writes `seed/build/seed.sql`. It fails and removes a stale `seed.sql` when the seed is invalid.
- `make seed-staging` and `make seed-prod` are `[CLOUD]` targets. They ask for confirmation and need the matching `make migrate-staging` or `make migrate-prod` first.
- To change seed data, edit `seed/*.json`, run `cargo test -p seed-build`, then run `make test-d1`.

## Testing

`make test-d1` runs `#[ignore]`d Rust tests with `--ignored --test-threads=1` on a real local D1 engine. It covers:

- the schema and seed tests in `tools/seed-build/tests/d1/` (scenario A2, scenario 27 and the seed load equality);
- once plan p2 is merged, the `client` crate's `client/tests/d1_master_data_sql.rs`, which runs the Worker's own SQL constants and parse code. `make test-d1` reaches it through the prerequisite line `test-d1: test-d1-read`.

The full run takes about 2 minutes and needs Node.js and `npx wrangler`.

Both suites use the dev-only crate `tools/d1-local` (`D1Local`). Each test owns `backend/target/tmp/d1/<name>`, passed to `npx wrangler d1 execute --local --persist-to`, run from `admin/`. The harness panics on any argument list that contains `--remote`. This state is separate from the developer's `admin/.wrangler/state`, which `make migrate-local`, `make seed-local` and `make smoke-local` use.

The `D1Local` API:

- `fresh(name)` and `migrated(name)` create an empty or a migrated database.
- `migrate()` applies `migrations/`.
- `state_dir()` returns the `--persist-to` directory.
- `exec(sql)` and `exec_file(path)` run SQL and return the rows.
- `query(sql)` and `count(table)` read rows.
- `snapshot(tables)` captures table contents for comparison.
- `assert_rejected(sql, needle)` checks that a statement fails with an error containing `needle`.

Test names are letters, digits and underscores and are unique per test.

To add a test to `seed-build`, add a file in `tools/seed-build/tests/d1/` and one `mod` line in `tools/seed-build/tests/d1/main.rs`. Mark each test `#[ignore = "needs npx wrangler and local D1; run with make test-d1"]`.

CI runs `make test-d1` in the job `d1-tests` (Node 22, `wrangler@4.168.0`).

The choice of wrangler `--local` over `rusqlite` and the system `sqlite3` CLI is recorded in the [B3 debate](../../battlegrounds/SPEC-002a-p1-schema-seed/02_D1_TEST_HARNESS/debate.md).

## Public read API

See [master-data-public-read.md](master-data-public-read.md) (written by plan SPEC-002a-p2-public-read).

## Linked decisions

- [D05](../../decisions/D05.md)
- [D07](../../decisions/D07.md)
- [D08](../../decisions/D08.md)
- [D09](../../decisions/D09.md)
- [D10](../../decisions/D10.md)
- [D11](../../decisions/D11.md)
- [D12](../../decisions/D12.md)
- [D13](../../decisions/D13.md)
- [SPEC-002 sections 4.1 to 4.4](../../specs/SPEC-002-master-data-to-backend.md)
- [SPEC-002a](../../specs/SPEC-002a-master-data-schema-seed-public-read.md)
