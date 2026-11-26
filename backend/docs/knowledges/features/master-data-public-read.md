# Master data public read

> Permanent notes on the `client` read path for master data (SPEC-002a, plan `SPEC-002a-p2-public-read`). The schema and seed are in [master-data.md](master-data.md).

## 1. Overview

Four public, read-only, cached GETs on `client` serve master data from D1: `/v1/cards`, `/v1/daily-cards`, `/v1/card-art` and `/v1/reading-config`. The response shapes are defined by `docs/api/client/openapi.yaml` v0.2.1.

Request flow:

1. Workers Cache. A hit returns the stored response without running the Worker.
2. `security_pipeline_middleware`: request id, CORS, rate limit, body cap.
3. Router.
4. Handler in `client/src/routes/master_data.rs`.
5. `MasterDataPort` method.
6. One D1 batch of `SELECT` statements.
7. The `*_from_sets` function parses each `row_json` string into typed rows.
8. The `assemble_*` function in `client/src/master_data.rs` sorts and shapes the rows.
9. JSON response with cache headers.

## 2. Endpoints

| Path | Schema in `openapi.yaml` | D1 statements in the batch | Body size | Rows read per miss |
| :--- | :--- | :--- | :--- | :--- |
| `GET /v1/cards` | `ReadingCard` (array) | `READING_CARDS_BATCH`: `LOVE_STATUSES_SQL`, `CARDS_SQL` | about 255 KB | about 1,014 |
| `GET /v1/daily-cards` | `DailyCardsDocument` | `DAILY_CARDS_BATCH`: `DAILY_META_SQL`, `LOVE_STATUSES_SQL`, `DAILY_CARDS_SQL` | about 359 KB | about 546 |
| `GET /v1/card-art` | `CardArtManifest` | `CARD_ART_BATCH`: `CARD_ART_SQL` | about 18 KB | not measured |
| `GET /v1/reading-config` | `ReadingConfig` | `READING_CONFIG_BATCH`: `CATEGORIES_SQL`, `SITUATIONS_SQL`, `SUGGESTIONS_SQL`, `POSITIONS_SQL`, `LOVE_STATUSES_SQL` | about 4 KB | not measured |

The row counts come from the staging checklist in section 6 and have not been checked against the D1 dashboard yet.

## 3. Headers and errors

- `200` carries `Cache-Control: public, max-age=600` and `Vary: Origin`. Both are set in the handler (`public_json`). `Vary: Origin` is set even when the request has no `Origin`, because Workers Cache honors `Vary`. Without it, a stored no-Origin variant could be served to a browser request.
- Every error uses the standard envelope and `Cache-Control: no-store`:
  - `503` when D1 fails
  - `500` when a row does not decode
  - `429` when the rate limit is exceeded
  - `404` for an unknown path, for any method
  - `405` for a known path called with a method it does not serve
- `405` applies to every known path, including `/health`, and carries `Allow: GET,HEAD`. It comes from `Router::method_not_allowed_fallback` (D12).
- `OPTIONS` is handled by the middleware.
- A write method with a disallowed `Origin` gets `403` before routing.

## 4. Data rules

The shaping rules are in `client/src/master_data.rs`.

- Sort by `sort_order`, then by key or id, in Rust (`by_order_then_key`). The SQL has no `ORDER BY`.
- Null fill: `fill_text_map` writes one entry per current love-status key and uses `null` where no text is stored. Stored keys that are not current love statuses are dropped.
- `missing_fields` is computed from daily texts that are `NULL` or absent, in daily love-status order.
- `situations.love` is derived from the love statuses, using the reading labels and reading order.
- Uncategorized lists hold items with `category = null` first. While no category with key `love` exists, the love-derived situations follow them.
- `0` and `1` become booleans through `sql_bool`. Any other value is a decode error.
- `approved_for_web` is served whatever its value (D09 item 6).

## 5. Read-only guarantee

`MasterDataPort` has no write method, so no code path in `client` can write master-data tables. Every SQL constant is a single `SELECT json_object(...) AS row_json FROM ...`. The unit test `every_statement_is_a_single_read_only_select` in `client/src/adapters/d1_master_data.rs` checks the prefix, the absence of `;` and the absence of write keywords.

## 6. Workers Cache (D13)

- Config: `[cache] enabled = true` in `client/wrangler.toml` at top level, and again in `[env.staging.cache]` and `[env.prod.cache]`, because inheritance is not documented.
- Only responses whose `Cache-Control` allows it are stored: the four GETs on `200`. Every other response is `no-store`.
- There is no purge (D05, TODO-011). An admin edit can take up to 10 minutes to show.
- The cache key includes the Worker version (the default), so a deploy starts with an empty cache.
- A hit skips the Worker. The rate limiter, the request-id middleware and logging do not run, and the cached response carries the `x-request-id` of the request that filled the cache.

Local runs do not exercise Workers Cache. These checks are not done yet; the owner runs them on staging after the `[CLOUD]` deploy and seed load:

1. Hit including the 359 KB body. Request `/v1/daily-cards` twice. Both are `200` with `cache-control: public, max-age=600` and `vary: Origin`. The second shows `cf-cache-status: HIT`. `cmp` of the two bodies reports no difference and the size is about 359,000 bytes. Repeat for `/v1/cards` (about 255 KB).
2. `Age` header. The second response carries `age: <seconds>`. If it does not, the worst-case staleness is about 20 minutes (edge plus browser), and the owner decides per D13 whether to accept it (open question Q1).
3. Variants per Origin. Two requests with `Origin: https://staging.maemorkratai.com`: the second is a `HIT`, and both carry `access-control-allow-origin` with that origin. A following request without `Origin` must not carry `access-control-allow-origin`.
4. Errors and `/health` are not cached. Send each twice and expect no `cf-cache-status: HIT`:
   - `POST /v1/cards`: `405`, `allow: GET,HEAD`, `cache-control: no-store`
   - `GET /v1/nope`: `404`, `no-store`
   - `GET /health`: `503` because staging has no R2, `no-store`
5. CPU on a miss: see section 8.
6. Optional: D1 dashboard rows read match about 1,014 per `/v1/cards` miss and about 546 per `/v1/daily-cards` miss, and are near zero for hits.

The full commands are in `docs/deployment.md`, section "Client master-data read: local checks and staging verification".

## 7. Testing

| Command | Covers |
| :--- | :--- |
| `cargo test -p backend-client` | Unit tests, `spec_001_scenarios`, `spec_002a_scenarios` (memory adapter, tiny fixture), `spec_002a_seed_scenarios` (full seed golden values). `d1_master_data_sql` is ignored here. |
| `make test-d1-read` | `client/tests/d1_master_data_sql.rs`: every adapter SQL constant on real local D1 (SQLite), through the production parse and assembly functions. It uses the shared `tools/d1-local` harness. `make test-d1` runs it first. |
| `make smoke-local` | The real client Worker under `wrangler dev` with local D1: status codes, headers, counts, types, 405 and 404 behavior. |
| `make check` and the two clippy commands | Format, lint and wasm32 build, as listed in `docs/knowledges/code-standards.md`. |

Host tests use the memory adapter, so they cannot find bugs that exist only in D1 or only in wasm. `make test-d1-read` and `make smoke-local` exist for those bugs.

## 8. CPU budget

- Battleground B1 chose Option B: SQL returns one `json_object` string per row, Rust parses each into typed rows with serde_json, and one shared pure assembly shapes the response. This makes about 78 boundary crossings per endpoint instead of about 1,000, keeps the contract in typed Rust and keeps the rules in one place.
- Option C (whole document in SQL) has the lowest CPU but puts the rules in SQL twice and ships unvalidated strings. It is the fallback, recorded as proto-spec P2-PS1 (pre-rendered body for hot endpoints), if misses on staging exceed about 8 ms.
- Measure on staging, not locally. Run `cd client && npx wrangler tail --env staging --format json` in one terminal. In another, send 50 requests to `/v1/daily-cards` with a unique query string each (it changes the cache key, so every request is a miss), wait 60 s, then do the same for `/v1/cards`. Read `cpuTime` from the tail events, or use the CPU time percentiles in the dashboard. The rate limit is 60 requests per minute per path and IP.
- Pass: p99 under 10 ms and no request failed with error 1102. If p99 is above about 8 ms, the owner decides between P2-PS1 and the Paid plan.
- This measurement has not been done yet.
- Debate record: `docs/battlegrounds/SPEC-002a-p2-public-read/01-master-data-assembly/debate.md`.

## 9. Linked decisions

- [D04](../../client/decisions/D04.md): public client security baseline (`docs/client/decisions/D04.md`)
- [D05](../../decisions/D05.md): 10-minute public cache, no invalidation
- [D07](../../decisions/D07.md): migrations hold schema only, seed is loaded manually
- [D08](../../decisions/D08.md): English keys, Thai labels per screen
- [D09](../../decisions/D09.md): master-data integrity rules
- [D10](../../decisions/D10.md): deleting a category leaves its items uncategorized
- [D11](../../decisions/D11.md): split into read (SPEC-002a) and write (SPEC-002b)
- [D12](../../decisions/D12.md): client contract review, 405 and 404 rules
- [D13](../../decisions/D13.md): Workers Cache on `client`
