# MaeMorKratai Center

Monorepo for MaeMorKratai (แม่หมอกระต่าย), a Thai tarot reading website and its admin tools.

Since 2026-11-24 one person (the project owner) maintains every folder. The folders still separate the parts so each can be built, tested and deployed on its own.

| Folder | Part | Purpose |
|---|---|---|
| [`frontend-client/`](frontend-client) | Frontend | Customer tarot website. Next.js static export on Cloudflare Workers. Does not call the backend yet. |
| [`frontend-admin/`](frontend-admin) | Frontend | Admin dashboard: site overview, reader scoring, competitor clip research, content automation control. Not started. |
| [`backend/`](backend) | Backend | Rust + axum on Cloudflare Workers with D1 and R2. Two Workers: `client/` (public API) and `admin/` (auth and admin API). See [`backend/README.md`](backend/README.md). |
| [`docs/`](docs) | Shared | Project context, API contracts (OpenAPI) and reading rules. |

## Rules

- Frontend and backend talk only through the API contracts in [`docs/api/`](docs/api). Never import code across folders.
- A contract change goes in the same pull request as the code that implements it, or before it. The contract stays the source of truth.
- Work on a branch and open a pull request into `main`. Pushing `main` deploys `frontend-client` automatically.
- Never commit secrets. `.env*` and `.dev.vars` are ignored; commit `.env.example` / `.dev.vars.example` with placeholder values instead.
- Never commit real reader answers, audio or holdout data.

The content automation program runs only on the owner's machine and talks to the backend through the admin API.
