# Backend Deployment Runbook (Cloudflare)

Status: **staging deployed 2026-11-25** on `*.workers.dev` (D1 only; R2 is not enabled on the account yet, so `/health` returns 503). Prod is not deployed.

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
| Load master data (seed) | `make seed-staging` / `make seed-prod` (`make seed-local` for dev; `make seed-build` to only build) |
| Verify code | `make check`, `make test` |
| D1-level SQL tests | `make test-d1` (needs Node.js and `npx wrangler`) |
| Client SQL on real local D1 | make test-d1-read (also run by make test-d1) |
| Local Worker smoke test | make smoke-local (migrates and seeds admin/.wrangler/state, runs client with wrangler dev, curls the GETs) |
| Local dev | `make dev-admin`, `make dev-client` |

`seed/build/` holds the generated `seed.sql` and is ignored by git.

### Local D1 state

`make migrate-local`, `make seed-local` and (plan p2) `make smoke-local` use wrangler's default state `admin/.wrangler/state`. The client Worker reads that state only when it is started with `--persist-to ../admin/.wrangler/state`. `make test-d1` never touches it. It uses `target/tmp/d1/<name>`.

Prerequisite for `build-*`, `dev-*` and `deploy-*`: `cargo install worker-build@^0.8 --locked`. worker-build 0.9 requires the `worker` 0.9 crate and fails on this workspace (`worker` 0.8).

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
   - 5b. `make seed-<env>`. It confirms first. Re-running is safe, but it overwrites admin edits to seeded rows.
6. Custom domains/routes: not configured yet. Add `routes` to the env blocks once domains are decided (`ALLOWED_ORIGINS`, `JWT_ISSUER`, `JWKS_URL` already assume `admin-api[-staging].maemorkratai.com`).
7. `make create-user EMAIL=... ROLE=owner ENV=<env>`.
8. `make cf-dry-run ENV=<env>`, then `make deploy-<env>` (or run the `backend-deploy` workflow).

## GitHub Actions

- `.github/workflows/backend-ci.yml` (PRs and `main`, path-filtered to `backend/`): wasm `cargo check`, `cargo test`, `cargo fmt --check` and clippy with `-D warnings` on host (all targets) and wasm32. All steps block the build since 2026-11-24. CI uses the latest stable Rust, so a new clippy release can add lints; fix them in a small PR when that happens.
- Job `d1-tests` in `backend-ci.yml`: a separate job that runs `make test-d1` on local D1 only. It uses Node 22, a pinned `wrangler@4.168.0`, and no secrets. The owner should mark it as a required check.
- `.github/workflows/backend-deploy.yml`: **manual** (`workflow_dispatch`) with an environment choice. Applies migrations, then deploys admin, then client.
  - Repo secrets required: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.
  - Token scopes: Workers Scripts (edit), D1 (edit), Workers R2 Storage (edit), Account Settings (read).
  - Create GitHub Environments `staging` and `prod`; add required reviewers on `prod`.


## Client master-data read: local checks and staging verification

### Local checks

- `make test-d1-read` runs `client/tests/d1_master_data_sql.rs`: the Worker's own SQL constants and parse code on a real local D1. It uses an isolated state in `target/tmp/d1/client_master_data_sql` through the `d1-local` crate, and builds the seed first (`make seed-build`). `make test-d1` runs it as a prerequisite. Needs Node.js (`npx wrangler`).
- `make smoke-local` runs `scripts/smoke-master-data.sh`. It applies `make migrate-local` and `make seed-local` to the developer state `admin/.wrangler/state`, starts the client Worker with `wrangler dev --persist-to ../admin/.wrangler/state`, and checks status codes, headers, counts and types with curl. Needs Node.js, `worker-build` (see the prerequisite above) and the p1 seed targets. The first build can take minutes. It never touches staging or prod and does not exercise Workers Cache.

### Staging verification (owner)

Steps 1 to 3 change cloud state. Do not run them without the owner's approval.

1. Local, no cloud change: `make cf-dry-run ENV=staging` passes, which proves wrangler accepts `[env.staging.cache]`.
2. `[CLOUD]`: `make migrate-staging`, then `make seed-staging`.
3. `[CLOUD]`: `make deploy-staging`.

The checks below are read-only. Set `S=https://maemorkratai-backend-client-staging.maemorkratai.workers.dev` and `O=https://staging.maemorkratai.com`. The rate limit is 60 requests per minute per path and IP, so wait a minute between the CPU batches. These checks have not been run yet.

1. Hit including the 359 KB body. Run `curl -s -D h1 -o b1 $S/v1/daily-cards; curl -s -D h2 -o b2 $S/v1/daily-cards`. Expect:
   - both `200` with `cache-control: public, max-age=600` and `vary: Origin`
   - `h2` shows `cf-cache-status: HIT`
   - `cmp b1 b2` reports no difference, and `wc -c b2` is about 359,000
   - repeat for `/v1/cards` (about 255 KB)
2. `Age` header. `h2` carries `age: <seconds>`. If it does not, the worst-case staleness is about 20 minutes (edge plus browser), and the owner decides per D13 whether to accept it (open question Q1).
3. Variants per Origin. Run `curl -s -D - -o /dev/null -H "Origin: $O" $S/v1/cards` twice. The second is a `HIT`, and both carry `access-control-allow-origin: $O`. A following request without `Origin` must not carry `access-control-allow-origin`.
4. Errors and `/health` are not cached. Send each twice:
   - `curl -s -D - -o /dev/null -X POST $S/v1/cards`: `405`, `allow: GET,HEAD`, `cache-control: no-store`
   - `curl -s -D - -o /dev/null $S/v1/nope`: `404`, `no-store`
   - `curl -s -D - -o /dev/null $S/health`: `503` because staging has no R2, `no-store`

   No response shows `cf-cache-status: HIT`.
5. CPU on a miss. In one terminal run `cd client && npx wrangler tail --env staging --format json`. In another run `for i in $(seq 1 50); do curl -s -o /dev/null "$S/v1/daily-cards?cb=$(date +%s)$i"; done`, wait 60 s, then do the same for `/v1/cards`. A unique query string changes the cache key, so every request is a miss. Read `cpuTime` from the tail events, or use the CPU time percentiles in the dashboard. Pass: p99 under 10 ms and no request failed with error 1102. If p99 is above about 8 ms, the owner decides between P2-PS1 and the Paid plan.
6. Optional: D1 dashboard rows read for the period match about 1,014 per `/v1/cards` miss and about 546 per `/v1/daily-cards` miss, and are near zero for hits.
