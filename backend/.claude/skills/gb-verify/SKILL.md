---
name: gb-verify
description: Verify an executed gb-sdlc backend plan against its spec, run the OWASP Top 10 audit, then archive the plan or route it back to planning. Use when the owner says "gb-verify", "verify the plan", or after gb-execute finishes all tasks. Not for planning or executing.
---

# gb-verify

Entry point for `.gb-sdlc/workflows/04-verification-audit.md`, using the preset `.gb-sdlc/agents/verifier.md` and `.gb-sdlc/guardrails/security-owasp.md`. Work inside `backend/`.

## 0. Before anything

1. Read `HANDOFF.md`, the spec, and every plan's `00_OVERVIEW.md`.
2. **Gate**: all tasks in all plans for this spec are ticked. If not, stop and point to `gb-execute`.

## 1. Verify (one agent)

Spawn one Agent with `models.verifier`: "Act as `.gb-sdlc/agents/verifier.md` for `docs/specs/<spec>.md`." It checks every acceptance scenario and every decision in `docs/decisions/` (and `docs/admin|client/decisions/` for D01–D04), and runs the OWASP Top 10 checklist.

Commands the verification must include:

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --exclude create-user --target wasm32-unknown-unknown --locked -- -D warnings
make check
```

**Project addition (2026-11-25):** host tests use the pure-Rust crypto path and the in-memory store, so they cannot see wasm-only or D1-only bugs (two such bugs reached `main` once). For any change to a Worker's runtime path, also run the Worker locally (`make migrate-local`, `make dev-admin` or `make dev-client`) and exercise the changed endpoints with `curl` against local D1. Record what was exercised.

## 2. Outcome

- **FAIL**: write `docs/plans/<plan>/failure-feedback.md`, set `HANDOFF.md` status `BLOCKED` with next step "re-plan", and tell the owner. Next is `gb-plan` for the failed part.
- **PASS**: write `docs/plans/<plan>/security-audit.md`; move each `docs/plans/<plan>/` to `docs/plan-records/<plan>/` (keep the estimate and audit files with them); log non-blocking suggestions in `TODO.md`; set `HANDOFF.md` status `IDLE` with the new capabilities; mark the spec `COMPLETED`.

## 3. Report

Tell the owner in Thai what passed, what was logged for later, and that the change is ready to deploy to staging. Deploying is a separate step the owner approves (`make deploy-staging`).
