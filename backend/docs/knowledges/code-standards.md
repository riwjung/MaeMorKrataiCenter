# Code Standards (Living)

> One way of writing code in this project, enforced by tools. Agents follow this file; the first plan creates the toolchain if it is missing (guardrail 05). Prefer adding a lint rule over adding prose.

## 1. Stack & Toolchain
| Stack | Linter / formatter | Type check | Check command |
| :--- | :--- | :--- | :--- |
| Rust (stable in CI, 1.100 locally), `worker` 0.8.7, axum 0.8, edition 2021 | `rustfmt`, `clippy` with `-D warnings` | `cargo check` for wasm32 | `make check` (see the command set below) |

The command set. All of these block the build in CI:

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo clippy --workspace --exclude create-user --exclude seed-build --exclude d1-local --target wasm32-unknown-unknown --locked -- -D warnings`
- `make check`
- `cargo test --workspace --locked`

On the 8 GB Mac, run commands with the lean environment:

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 2. Test Harness
- Runner: `cargo test`; command: `cargo test --workspace --locked`.
- Host tests use in-memory adapters (`MemoryClientStore`, `tower::ServiceExt::oneshot`).
- SQL tests run through `make test-d1` with the `tools/d1-local` crate (see `features/master-data.md`).
- Layout: unit tests sit next to the code, integration tests in `<crate>/tests/`. The D1 tests of `seed-build` are in `tools/seed-build/tests/d1/`.

## 3. Conventions
- Host-only tools live in `tools/<name>` and are excluded from the wasm32 commands (`create-user`, `seed-build`, `d1-local`).
- CLI arguments use the `--key=value` form, as in `create-user` and `seed-build`.
- Tool errors are `Result<_, String>` printed with the tool name, followed by exit code 1.
- Never use `#[allow]` or other lint suppression in production code, and never to get past a new lint. A lint is fixed in the code. The one documented exception is a module-level `#![allow(dead_code)]` with a reason comment in test-support modules that several test binaries of one crate share (`client/tests/support/`, `client/tests/seed_support/`, plan p2).
- Migrations are DDL only. `tools/seed-build/tests/migrations_ddl_only.rs` enforces this.
- Seed data changes go through `seed/*.json` and `seed-build`.

## 4. Learned Rules
> Added by each plan's documentation task. Note whether a lint rule now enforces it.
- 2026-11-26 SPEC-002a-p1-schema-seed: Write `format!` arguments inline (`{name}`). Clippy's `uninlined_format_args` rejects positional identifiers (enforced by: clippy).
- 2026-11-26 SPEC-002a-p1-schema-seed: Test helpers shared by test binaries of different crates live in a dev-only crate (`tools/d1-local`). Helpers used by several modules of one test binary live in that binary (`tests/d1/main.rs` plus modules), so no `dead_code` allowance is needed (enforced by: review only).
- 2026-11-26 SPEC-002a-p1-schema-seed: D1 tests are `#[ignore = "needs npx wrangler and local D1; run with make test-d1"]` (enforced by: review only).
- 2026-11-26 SPEC-002a-p1-schema-seed: The seed upsert is always `ON CONFLICT(<pk>) DO UPDATE`, never `INSERT OR REPLACE`. REPLACE deletes the row first, which fails on foreign keys and loses rows that are not in the seed (enforced by: `tools/seed-build/tests/d1/seed.rs`).

## Patterns from SPEC-002a (client read path)

- A port returns unsorted typed rows. Ordering and response shaping live in one pure module (`client/src/master_data.rs`) shared by the D1 and memory adapters.
- D1 statements are static `SELECT`s that return one `row_json` TEXT column built with `json_object`, parsed with serde_json into typed rows. Booleans use `sql_bool`. `"key"` is quoted.
- Every D1 call in an `async_trait` method goes through one `SendFuture::new(async move { ... })` block.
- Every adapter SQL constant is exercised against real local D1 (SQLite) in `make test-d1`, through the production parse and assembly functions, by `client/tests/d1_master_data_sql.rs`, which uses the shared `tools/d1-local` harness. `tools/seed-build` does not depend on `client`.
- Public cacheable responses set `Cache-Control` and `Vary: Origin` in the handler, and only on `200`. Errors use `ClientError::into_response_with_request_id` (`no-store`).
- Test helpers shared by several files under `client/tests/` live in a `support`-style module with a module-level `#![allow(dead_code)]` and a comment giving the reason (each test file is its own crate). This is the documented exception to the no-`#[allow]` rule in the Conventions section above.

No new lint rule: clippy has no rule for these patterns.
