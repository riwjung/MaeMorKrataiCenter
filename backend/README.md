# backend

Single backend workspace for MaeMorKratai (Rust + Axum on Cloudflare Workers, D1, R2). Maintained by the project owner (alone since 2026-11-24).

| Folder | Purpose |
|---|---|
| `client/` | Public API Worker for `frontend-client`. Contract: `../docs/api/client/` |
| `admin/` | Admin API Worker (auth, JWKS issuer) for `frontend-admin`. Contract: `../docs/api/admin/` |
| `migrations/` | Shared D1 migrations (canonical, applied through the admin Worker) |
| `tools/create-user/` | CLI to create admin users |
| `docs/specs/`, `docs/decisions/` | Shared specs (SPEC-002 on) and decisions (D05 on) |
| `docs/client/`, `docs/admin/` | SPEC-001 era specs, plans, decisions (D01–D04) per service |
| `.gb-sdlc/` | Spec-driven development framework (shared) |

Each of `client/` and `admin/` is its own Worker with its own `wrangler.toml`. `make help` lists every command; [`docs/deployment.md`](docs/deployment.md) is the Cloudflare runbook.

Start here: [`HANDOFF.md`](HANDOFF.md) (current state and next steps) and [`TODO.md`](TODO.md) (deferred ideas, one register for both Workers).
