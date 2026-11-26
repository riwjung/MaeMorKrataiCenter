# Battleground Debate Record

- **Plan**: `SPEC-002a-p1-schema-seed`
- **Task**: `01_SEED_DATA_CONVERSION`
- **Date**: 2026-11-25
- **Status**: RESOLVED

## 1. The Conflict / Decision Point
`seed/*.json` must be produced once, deterministically, from four frontend files (`reading-cards.json`, `daily-cards.json`, `reading.ts`, `public/cards/v3/manifest.json`). Every Thai string is copied unchanged; only the SPEC-002 section 4.2 keep/drop table, the section 4.3 English keys and the suggestion IDs `love-1` … `life-3` are applied. `reading.ts` is TypeScript and must be parsed and round-trip-checked. D07 says the backend "never reads `../frontend-client/` at runtime or in scripts", and `docs/deployment.md` forbids loose scripts. Where does the conversion code live?

Contenders ran on sonnet/medium, one per option.

## 2. Competing Options

### Option A: One-off Python script printed in the task file, run once from outside the repo
- **Advocate**: Contender A
- **Security (#1)**: Nothing that reads `../frontend-client` is committed, so D07 holds and no standing tool can overwrite `seed/` from the sibling tree later. Run with `python3 -I` (ignores environment and cwd imports) from a temp directory. The task records the SHA-256 of the script and of the four inputs and four outputs, so corrupted pasting or drifted sources fail before anything is committed. The script also fails if any dropped token (for example `chatgpt.com` or `"raw_file"`) reaches an output.
- **Technical Debt (#2)**: No new code in the repo; the Makefile stays the only entry point. The exact code is kept in `docs/plan-records/` after verification. The `tools/seed-build` crate stays decoupled from the frontend file layout, which D07 expects the frontend owner to delete.
- **Speed (#3)**: One file of about 250 lines, already dry-run by the planner into a scratch directory (all checks pass; output hashes recorded). Python string handling and regex make the `reading.ts` parse short.
- **Weakness**: Not reproducible from the repo alone. Mitigation: a future re-conversion is a new plan with a fresh review, because the frontend data would have changed.

### Option B: Committed `convert` subcommand in `tools/seed-build`
- **Advocate**: Contender B
- **Security (#1)**: Code goes through PR review and CI, and Rust avoids adding Python to the toolchain. However, it violates D07 as written. The contender proposes amending D07 to allow one documented exception.
- **Technical Debt (#2)**: Adds a TypeScript literal extractor in Rust and frontend-path coupling to a crate whose job is validate-and-emit. The input can disappear when the frontend owner deletes the JSON, so the code would rot. The contender proposes deleting it in a follow-up PR, which ends with the same "run once, remove" outcome as A.
- **Speed (#3)**: A few hundred lines of Rust plus tests. One to two days, against hours for A.

## 3. Outcome & Winning Rationale
- **Winner**: Option A.
- **Justification**:
  - **Security.** A keeps D07 intact and leaves no path in the repo that reads the frontend tree. B needs a decision amendment, and its code would keep the ability to overwrite `seed/`.
  - **Technical debt.** A adds no code to maintain. B adds a parser that rots once the frontend JSON is deleted, and its own advocate proposes deleting it later.
  - **Speed.** A is already written and dry-run.
- **Adopted from B**: the Rust seed build, not the conversion, is the reviewed and tested gate. `tools/seed-build` denies unknown fields, scans for dropped tokens, and checks cross-file consistency on every build, so any later hand edit to `seed/` is still validated.
