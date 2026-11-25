# Backend Deployment Runbook (Cloudflare)

Status: **prepared, not deployed.** No Cloudflare resources have been created and no Worker has been published.

## What is deployed

| Service | Worker name (staging / prod) | Bindings |
| :--- | :--- | :--- |
| `admin/` | `maemorkratai-backend-admin-{staging,prod}` | D1 `DB`, R2 `PUBLIC_ASSETS` + `PRIVATE_STORAGE`, rate limiter `AUTH_RATE_LIMITER` |
| `client/` | `maemorkratai-backend-client-{staging,prod}` | D1 `DB`, R2 `PUBLIC_ASSETS` only (D02), rate limiter `PUBLIC_RATE_LIMITER` |

Both share one D1 database and the R2 buckets per environment (D02). Migrations live in `migrations/` and are applied through the admin Worker's D1 binding.

## Command guideline (Makefile is the only entry point)

Run every backend command through `make` from `backend/`. There are no standalone scripts. `make help` lists all targets with their usage. Targets tagged `[CLOUD]` change real Cloudflare state.

| Goal | Command |
| :--- | :--- |
| Check login | `make cf-whoami` (log in first with `npx wrangler login`) |
| Create D1 + R2 for an env | `make cf-bootstrap ENV=staging` (confirms before creating) |
| Set a secret | `make cf-secret SERVICE=admin NAME=PASSWORD_PEPPER ENV=staging` |
| Apply migrations | `make migrate-staging` / `make migrate-prod` (`make migrate-local` for dev) |
| Bundle without deploying | `make cf-dry-run ENV=staging` |
| Deploy (admin, then client) | `make deploy-staging` / `make deploy-prod` |
| Create first owner | `make create-user EMAIL=... ROLE=owner ENV=staging` |
| Verify code | `make check`, `make test` |
| Local dev | `make dev-admin`, `make dev-client` |

Rules: `ENV` is `dev`, `staging` or `prod`; `SERVICE` is `admin` or `client`. If a command is missing, add it to the Makefile with a `## description` comment (that comment is what `make help` prints) and a row here. Do not add loose scripts.

## One-time setup per environment (staging first, then prod)

1. `npx wrangler login`, then `make cf-whoami`.
2. `make cf-bootstrap ENV=<env>`.
3. Paste the printed D1 `database_id` into `admin/wrangler.toml` and `client/wrangler.toml` for that env. The current `00000000-0000-0000-0000-00000000000N` values are placeholders and must be replaced before any deploy.
4. Secrets (never committed):
   - `make cf-secret SERVICE=admin NAME=PASSWORD_PEPPER ENV=<env>`
   - `make cf-secret SERVICE=admin NAME=KEY_ENCRYPTION_KEY ENV=<env>`
   - `make cf-secret SERVICE=client NAME=TURNSTILE_SECRET_KEY ENV=<env>`
5. `make migrate-<env>`.
6. Custom domains/routes: not configured yet. Add `routes` to the env blocks once domains are decided (`ALLOWED_ORIGINS`, `JWT_ISSUER`, `JWKS_URL` already assume `admin-api[-staging].maemorkratai.com`).
7. `make create-user EMAIL=... ROLE=owner ENV=<env>`.
8. `make cf-dry-run ENV=<env>`, then `make deploy-<env>` (or run the `backend-deploy` workflow).

## GitHub Actions

- `.github/workflows/backend-ci.yml` (PRs and `main`, path-filtered to `backend/`): wasm `cargo check`, `cargo test`, `cargo fmt --check` and clippy with `-D warnings` on host (all targets) and wasm32. All steps block the build since 2026-11-24. CI uses the latest stable Rust, so a new clippy release can add lints; fix them in a small PR when that happens.
- `.github/workflows/backend-deploy.yml`: **manual** (`workflow_dispatch`) with an environment choice. Applies migrations, then deploys admin, then client.
  - Repo secrets required: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.
  - Token scopes: Workers Scripts (edit), D1 (edit), Workers R2 Storage (edit), Account Settings (read).
  - Create GitHub Environments `staging` and `prod`; add required reviewers on `prod`.

