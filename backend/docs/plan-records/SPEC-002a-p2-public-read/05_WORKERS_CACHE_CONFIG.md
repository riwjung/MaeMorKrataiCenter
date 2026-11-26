---
model: haiku
effort: low
reason: "Mechanical config edit plus one supplied test."
---

# Task 05: Enable Workers Cache on `client` (D13)

- **Executor / Coder Rule**:
  1. ALWAYS read [00_OVERVIEW.md](./00_OVERVIEW.md) first before starting execution.
  2. Execute ONLY this single task. When done and verified, terminate so a fresh Executor / Coder agent handles the next task.
  3. Apply exact code below without assumption or improvisation. Order: harness, tests, red, implementation, green, standards check.
  4. A compile, clippy or rustfmt error in the supplied code is a **plan defect**. Stop and report it as a deviation (workflow 03). Never silence it with `#[allow]`, never improvise a fix.

Depends on task 04 (the test file exists). All commands run from `backend/` with the lean env:
```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 PATH="$HOME/.cargo/bin:$PATH"
```

## 1. Action Items
- [ ] Append one test to `client/tests/spec_002a_scenarios.rs`
- [ ] Add three `cache` blocks to `client/wrangler.toml`
- [ ] Red, implement, green, standards check

Config key, checked against the Cloudflare docs on 2026-11-25 (`developers.cloudflare.com/workers/cache/configuration/`): a top-level `[cache]` table with `enabled = true` (Wrangler 4.69.0 or newer; the repo uses 4.168). The docs show a per-environment override as `[env.<name>.cache]` but do not say whether an environment without that block inherits the top-level value. Each environment therefore gets its own block. Whether staging really caches is checked on staging (see `00_OVERVIEW.md` §5, "verify on staging"). `cross_version_cache` stays unset, so each deploy starts with an empty cache (D13 item 4). No purge (D13 item 2).

Local tests cannot exercise the cache. This task only proves the config is present. The behavior is checked on staging by the owner/verifier.

## 2. Harness
None.

## 3. Tests

### File: `client/tests/spec_002a_scenarios.rs` (append at the end of the file)
```rust

// ---- Workers Cache config (task 05, D13) ------------------------------------------------

/// True when the non-comment line right after `header` (ignoring blank lines) is `line`.
fn section_starts_with(config: &str, header: &str, line: &str) -> bool {
    let lines: Vec<&str> = config
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    lines
        .iter()
        .position(|l| *l == header)
        .and_then(|i| lines.get(i + 1))
        .is_some_and(|next| *next == line)
}

#[test]
fn d13_workers_cache_is_enabled_for_every_environment() {
    let wrangler = include_str!("../wrangler.toml");
    for header in ["[cache]", "[env.staging.cache]", "[env.prod.cache]"] {
        assert!(
            section_starts_with(wrangler, header, "enabled = true"),
            "client/wrangler.toml needs `{header}` followed by `enabled = true`"
        );
    }
    assert!(!wrangler.contains("cross_version_cache"));
}
```

## 4. Red
```bash
cargo test -p backend-client --test spec_002a_scenarios d13_workers_cache
```
Expected: FAIL with `client/wrangler.toml needs `[cache]` followed by `enabled = true``.

## 5. Implementation

### File: `client/wrangler.toml` (exact diffs)
1. Replace:
```toml
[build]
command = "worker-build --release"

```
with:
```toml
[build]
command = "worker-build --release"

# D13: Workers Cache in front of the Worker. It stores only responses whose Cache-Control
# allows it (the four master-data GETs on 200, public, max-age=600); every other response is
# no-store. No purge (D05/TODO-011). Repeated per env below because inheritance is not documented.
[cache]
enabled = true

```
2. Replace:
```toml
[env.staging]
name = "maemorkratai-backend-client-staging"

```
with:
```toml
[env.staging]
name = "maemorkratai-backend-client-staging"

[env.staging.cache]
enabled = true

```
3. Replace:
```toml
[env.prod]
name = "maemorkratai-backend-client-prod"

```
with:
```toml
[env.prod]
name = "maemorkratai-backend-client-prod"

[env.prod.cache]
enabled = true

```
Nothing else changes. `scenario_1_wrangler_has_zero_private_storage_and_zero_migrations` in `spec_001_scenarios.rs` still passes (no `migrations_dir`, no `PRIVATE_STORAGE`).

## 6. Green + Standards Check
```bash
cargo test -p backend-client --test spec_002a_scenarios
cargo test -p backend-client --test spec_001_scenarios
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```
Expected: `spec_002a_scenarios` 13 passed; `spec_001_scenarios` 5 passed; fmt and clippy clean. Wrangler parses the dev config in task 08 (`wrangler dev`); the staging and prod blocks are parsed by `make cf-dry-run ENV=staging` in the verification checklist.
