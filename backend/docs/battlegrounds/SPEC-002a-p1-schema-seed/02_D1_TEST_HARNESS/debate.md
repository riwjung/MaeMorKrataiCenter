# Battleground Debate Record

- **Plan**: `SPEC-002a-p1-schema-seed`
- **Task**: `02_D1_TEST_HARNESS` (estimate battleground B3)
- **Date**: 2026-11-25
- **Status**: RESOLVED

## 1. The Conflict / Decision Point
`MemoryClientStore` cannot enforce foreign keys. Scenario A2 (constraint rejections leave tables unchanged), scenario 27 (re-seed restores seeded rows, keeps an extra admin row, writes no `audit_events`) and plan p2's adapter `SELECT` queries must run on a real SQL engine. Which engine, and how do tests drive it?

Facts probed by the planner on the owner's Mac (wrangler 4.168.0):
- `npx wrangler d1 execute DB --local --persist-to <dir> --json --command|--file` works from `admin/`; about 1.5 s per call; a 1,700-statement file loads in about 3.4 s.
- Errors print `{"error":{"text":"FOREIGN KEY constraint failed: SQLITE_CONSTRAINT (extended: SQLITE_CONSTRAINT_FOREIGNKEY)"}}` on stdout and exit 1. `UNIQUE constraint failed: p.k` likewise.
- `PRAGMA foreign_keys` returns 1 in local D1 without any setup.
- A `--file` whose second statement fails rolls back the first one too.
- Strings with `;`, newlines, `''` and `--` round-trip byte for byte.
- `--persist-to` keeps test state out of the developer's `admin/.wrangler/state`.

Contenders ran on sonnet/medium, one per option.

## 2. Competing Options

### Option A: Rust integration tests that shell out to `npx wrangler d1 execute --local`
- **Advocate**: Contender A
- **Security (#1)**: Only `--local` with an isolated `--persist-to` directory; no credentials, no remote calls. The harness hard-codes `--local` and refuses any argument list containing `--remote`. New supply-chain surface is the npm `wrangler` package in CI, the same package the deploy workflow already installs. Mitigation: pin the exact version (`wrangler@4.168.0`) and give the job read-only permissions and no secrets.
- **Technical Debt (#2)**: No new Rust dependency; `Cargo.lock` gains only the `seed-build` entry. Tests run the same wrangler, miniflare and workerd stack the Workers use, including the `--file` load path that `make seed-local` uses. Adds Node to CI. Weakness: `#[ignore]` tests are skipped by plain `cargo test`; mitigated by a required CI job that runs `make test-d1`.
- **Speed (#3)**: Helper of about 150 lines (spawn, capture, parse JSON). No C compile on the 8 GB Mac. Suite cost about 30 wrangler calls, roughly 60 to 90 s.

### Option B: `rusqlite` with `bundled` as a dev-dependency
- **Advocate**: Contender B
- **Security (#1)**: Dev-only, no network. Supply chain is `libsqlite3-sys` compiling the vendored SQLite amalgamation through `cc` in a build script; pinned by `--locked`. Smaller package tree than the npm wrangler install.
- **Technical Debt (#2)**: Real SQLite semantics for FK, UNIQUE, CHECK, UPSERT and JSON1, but not the D1 runtime and not the `wrangler --file` splitter. `foreign_keys` must be switched on by hand. The contender itself recommends keeping a wrangler smoke test as a release gate, which means two harnesses.
- **Speed (#3)**: Millisecond tests inside `cargo test`. First build compiles `sqlite3.c` (20 to 40 s, tens of MB in `target/`); cached in CI.

### Option C: Rust tests that shell out to the system `sqlite3` CLI
- **Advocate**: Contender C
- **Security (#1)**: Smallest surface: no new crates, no npm. Trusts the runner's distro binary.
- **Technical Debt (#2)**: Version drift (Mac 3.43.2, ubuntu's own build, D1's build). `foreign_keys` is OFF by default in the CLI, so every call must prepend the pragma. `-json` output differs from D1 result shapes. Does not exercise the `--file` path. The contender also recommends an optional wrangler smoke target, again two harnesses.
- **Speed (#3)**: About half a day, no CI change, fast runs.

## 3. Outcome & Winning Rationale
- **Winner**: Option A.
- **Justification**: Security is a near tie. A uses only `--local`, has no secrets, and pins the npm package; B and C are smaller in supply chain, but none of the three touches production or user input. On technical debt A wins clearly. It is the only option that tests what ships: D1's FK default, its error texts, and the `wrangler d1 execute --file` path that `seed-local`, `seed-staging` and `seed-prod` use. B and C both concede they need a wrangler smoke test as a backstop, which would leave two harnesses to maintain. A adds no Rust dependency and needs no C compile on the 8 GB Mac. A is the slowest at run time (about 1.5 s per call), which is acceptable for about 30 calls.
- **Adopted mitigations**:
  - Every D1 test runs in its own `--persist-to target/tmp/d1/<test>` directory, wiped at start.
  - The harness always passes `--local` and panics if any argument is `--remote`.
  - Tests are `#[ignore = "..."]` so plain `cargo test` stays Node-free. `make test-d1` runs them with `--ignored --test-threads=1`, which caps memory on the 8 GB Mac.
  - CI runs `make test-d1` in a separate required job `d1-tests` that pins `wrangler@4.168.0` and sets `WRANGLER_SEND_METRICS=false`.
  - A missing `npx` fails loudly with an install hint. It never skips.
