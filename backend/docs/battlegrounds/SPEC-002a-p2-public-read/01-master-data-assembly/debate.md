# Battleground Debate Record

- **Plan**: `SPEC-002a-p2-public-read`
- **Task**: `01-master-data-assembly` (estimate battleground B1; used by tasks 01, 02, 03)
- **Date**: 2026-11-25
- **Status**: RESOLVED

## 1. The Conflict / Decision Point
How the `client` Worker turns D1 rows into the four public responses within the Workers Free plan limit of 10 ms CPU per request, and how that CPU is measured. Workers Cache (D13) serves most requests, but every cache miss (first request per data center and cache key, first request after a deploy) runs the Worker. Sizes on a miss: `GET /v1/cards` reads about 1,014 rows (78 cards, 936 love-status texts) and returns about 255 KB; `GET /v1/daily-cards` reads about 546 rows (78 daily rows with about 20 text columns, 468 love texts) and returns about 359 KB. Time spent waiting on D1 (including SQLite's own work) is not Worker CPU. Work done in wasm (crossing the JS/wasm boundary, parsing, serializing) is.

Contenders ran on `sonnet` / medium, one per option, each arguing for its option and listing its weaknesses.

## 2. Competing Options

### Option A: Row-by-row typed structs
Plain `SELECT`s per table (`cards`, `card_meanings`, `card_details`, ...). Each row becomes a typed struct through `D1Result::results::<T>()` (serde_wasm_bindgen). Grouping, null fill and `missing_fields` happen in Rust.
- **Advocate**: Contender A
- **Security (#1)**: Static SQL only, typed structs fail on unexpected columns, and the public contract (null fill, ordering, `missing_fields`) is type-checked Rust. No SQL builds output text.
- **Technical Debt (#2)**: The usual workers-rs path. The memory adapter returns the same structs, so host tests cover the production assembly. A schema change touches one struct field and one column.
- **Speed (#3)**: Fastest to write. Highest CPU risk, as the advocate conceded: about 1,000 JS objects per `/v1/cards` miss and about 1,560 property reads for daily rows, each crossing the boundary through serde_wasm_bindgen, then a full serde_json serialization of the body. Unmeasured and possibly over 10 ms.

### Option B: One JSON text per row from SQL, typed parse, Rust assembly
Each statement returns one TEXT column `row_json` built with `json_object(...)`. Child love-status texts are folded into the row with a correlated `json((SELECT json_group_object(love_status, text) ...))`. About 78 strings cross the boundary per endpoint. Rust parses each with serde_json into the same typed row structs that the memory adapter returns. Ordering, null fill, `missing_fields` and uncategorized lists stay in one pure Rust module.
- **Advocate**: Contender B
- **Security (#1)**: Static, parameterless SQL that only gathers data. SQLite's `json_object` escapes the text, and Rust parses it again into typed structs, so a malformed or drifted row fails with a 500 instead of reaching a client. The public contract is still Rust types.
- **Technical Debt (#2)**: The business rules live once, in Rust, shared by the D1 and memory adapters, so host tests exercise production logic. Cost: each `json_object` key list must match its struct. This drift is caught by running the real SQL through the real parse and assembly code against real SQLite (task 07) and by `smoke-local` (task 08). Booleans arrive as 0/1 and need one `sql_bool` deserializer.
- **Speed (#3)**: A small step from A. On a miss, 78 boundary crossings per endpoint instead of about 1,000, plus a linear serde_json parse of about 255 to 359 KB and one serialization. The advocate estimated low single-digit milliseconds, not measured.

### Option C: Whole response document rendered in SQL
SQL builds the final document (`json_group_array` of `json_object`, ordering, null fill, `missing_fields`, booleans) and the Worker passes one string through as the body.
- **Advocate**: Contender C
- **Security (#1)**: No deserialization in the Worker and one string forwarded. But the body is never checked by Rust: a schema drift or a wrong aggregate goes straight to clients, which Contenders A and B both flagged.
- **Technical Debt (#2)**: The weakest, as the advocate conceded. Ordering, null fill and `missing_fields` live in SQL, the memory adapter needs a second implementation, and the two drift unless every host test runs SQLite (`rusqlite` with bundled C sources, which the 8 GB build machine and the estimate argue against). `json_group_array(... ORDER BY ...)` needs SQLite 3.44 or newer. Older versions do not guarantee aggregate order.
- **Speed (#3)**: Lowest Worker CPU: one string copy and no parse or serialize step. Slowest to write and to change correctly.

## 3. Outcome & Winning Rationale
- **Winner**: Option B.
- **Justification**:
  - Security. B and A keep the public contract in typed Rust and fail closed on bad rows. C ships an unvalidated SQL string. That rules out C first.
  - Technical debt. B keeps one implementation of every rule (in `client/src/master_data.rs`) shared by both adapters. C duplicates the rules. A and B tie here, and B's extra cost (key lists in SQL) is covered by the real-SQLite test in task 07, which runs the exact SQL constants through the exact parse and assembly functions.
  - Speed. B removes most of A's boundary cost, which is the largest unmeasured CPU risk, for little extra code.
- **Decisions that follow**:
  - Typed structs everywhere, no `serde_json::Value` in production code. Response structs serialize directly; key order in maps is kept with a small `OrderedMap` serializer.
  - Sorting happens in Rust (`sort_order`, then key or id), not through `ORDER BY`, so the memory adapter needs no ordering logic and SQL aggregate order does not matter.
  - Each endpoint sends one `D1Database::batch` (1 to 5 statements, far under the 50-query limit).
- **How CPU is measured** (owner/verifier, not an executor step, see `00_OVERVIEW.md` §5):
  - Local runs (`wrangler dev`) do not report real CPU. They prove correctness and sizes only (task 08).
  - On staging, after the owner-approved deploy and seed load, force misses with a unique query string (`?cb=<n>`, which changes the cache key) and read `cpuTime` from `npx wrangler tail --env staging --format json`, or the CPU time percentiles in the Workers dashboard. Pass if the p99 of 50 misses per endpoint is under 10 ms and no request fails with error 1102 (CPU limit exceeded).
  - Fallback if `/v1/daily-cards` or `/v1/cards` misses exceed about 8 ms: move that endpoint to Option C for the body only (proto-spec candidate P2-PS1), or move to the Workers Paid plan. Both need an owner decision.
