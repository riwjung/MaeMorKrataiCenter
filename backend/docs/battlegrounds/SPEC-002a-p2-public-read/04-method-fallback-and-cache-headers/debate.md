# Battleground Debate Record

- **Plan**: `SPEC-002a-p2-public-read`
- **Task**: `04-method-fallback-and-cache-headers`
- **Date**: 2026-11-25
- **Status**: RESOLVED

## 1. The Conflict / Decision Point
Two small choices in task 04, each with more than one valid implementation. The planner wrote both cases (no contender agents; the choices are local to one task).

1. How D12 is implemented: a known path with another method returns `405` with `Allow`, the error envelope and `no-store`. An unknown path returns `404` for any method. This covers every route, including `/health`.
2. Where the `200` header `Cache-Control: public, max-age=600` is set, given that `security_pipeline_middleware` forces `no-store` unless the response already has a `Cache-Control` header. A related point is whether `Vary: Origin` is set when the request has no `Origin`.

## 2. Competing Options

### Option A: `Router::method_not_allowed_fallback` plus header set in the handler
- **Security (#1)**: One router-level fallback applies to every registered path, so a new route cannot forget it. axum 0.8.9 keeps the `Allow` header from the route's method router (`RouteFuture::allow_header`, `routing/route.rs`), so `Allow: GET,HEAD` is correct by construction. The handler sets the public `Cache-Control` and `Vary: Origin` only when the status is `200`. Every error goes through `ClientError::into_response_with_request_id`, which sets `no-store`. Workers Cache honors `Vary` (Cloudflare docs, Workers Cache configuration). Without `Vary: Origin` on a response to a request that had no `Origin`, the cache would serve that response, with no `Access-Control-Allow-Origin`, to browser requests. So `Vary: Origin` is always set on cacheable responses.
- **Technical Debt (#2)**: Uses the existing middleware rule ("keep a `Cache-Control` the handler set") with no middleware change. One helper (`public_json`) holds both headers.
- **Speed (#3)**: About 30 lines.

### Option B: Per-route `MethodRouter::fallback` and header set in the middleware by path
- **Security (#1)**: Each route needs its own fallback, so a missed route regresses to axum's bare `405`. A path list in the middleware duplicates the router and can mark a non-master-data path cacheable by mistake.
- **Technical Debt (#2)**: Two places know which paths are public and cacheable.
- **Speed (#3)**: Similar size, more places to test.

## 3. Outcome & Winning Rationale
- **Winner**: Option A.
- **Justification**: It is safer, because one fallback covers all routes and caching is opt-in per handler on `200` only. It also has less debt, because there is no path list and no middleware change. Error code `METHOD_NOT_ALLOWED`; the contract names no other.
