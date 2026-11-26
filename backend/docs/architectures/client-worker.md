# Client Worker architecture

> The public `client` Worker (`client/`), as of plan `SPEC-002a-p2-public-read`. No library and no environment variable were added by that plan.

## Module map

| Path | Role |
| :--- | :--- |
| `client/src/lib.rs` | Builds the axum router, applies `security_pipeline_middleware`, and defines the Worker `fetch` entry point. |
| `client/src/ports.rs` | Port traits (`HealthPort`, `RateLimitPort`, `MasterDataPort`, and the secret and clock traits) and the `ClientPorts` bundle. `MasterDataPort` has read methods only. |
| `client/src/adapters/worker_env.rs` | `WorkerClientAdapter`: implements the ports on Cloudflare bindings (D1 `DB`, R2 `PUBLIC_ASSETS`, the rate limiter). |
| `client/src/adapters/d1_master_data.rs` | Static `SELECT` constants, the batched D1 call, and the `*_from_sets` parse functions. |
| `client/src/adapters/memory.rs` | `MemoryClientStore`: in-memory implementation of the ports for host tests, with a D1-failure switch and a read counter. |
| `client/src/master_data.rs` | Typed rows, typed response structs, and the pure `assemble_*` functions. Shared by the D1 and memory adapters. |
| `client/src/routes/health.rs` | `/health` handler and the `404` and `405` fallbacks. |
| `client/src/routes/master_data.rs` | The four master-data GET handlers and the public cache headers. |
| `client/src/middleware/security.rs` | Request id, CORS, `403` for disallowed write origins, rate limit, body cap, security headers, `no-store` on everything except a handler's own `Cache-Control`. |
| `client/src/error.rs` | `ClientError` and the standard error envelope. |

## Cloudflare features used

- D1 binding `DB`. `client` only reads master data through it.
- R2 binding `PUBLIC_ASSETS`, used only by `/health`. Staging has no R2 binding yet, so `/health` reports `503` there.
- `[[ratelimits]]` binding `PUBLIC_RATE_LIMITER`: 60 requests per 60 seconds, keyed by path and client IP.
- Workers Cache (`[cache] enabled = true` at top level, staging and prod), since D13.

## Routes

| Method and path | Handler | Success response |
| :--- | :--- | :--- |
| `GET /health` | `get_health` | `200` JSON, `no-store`. `503` when D1 or R2 fails. |
| `GET /v1/cards` | `get_cards` | `200`, `Cache-Control: public, max-age=600`, `Vary: Origin` |
| `GET /v1/daily-cards` | `get_daily_cards` | same |
| `GET /v1/card-art` | `get_card_art` | same |
| `GET /v1/reading-config` | `get_reading_config` | same |
| any other method on a path above | `fallback_method_not_allowed` | `405`, `Allow: GET,HEAD`, `no-store` |
| any method on an unknown path | `fallback_not_found` | `404`, `no-store` |

Details of the master-data path are in [master-data-public-read.md](../knowledges/features/master-data-public-read.md).
