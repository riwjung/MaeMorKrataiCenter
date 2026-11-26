#!/usr/bin/env bash
# Local smoke test of the client master-data GETs (SPEC-002a). Run it with `make smoke-local`.
# Applies migrations and the seed to local D1 (admin/.wrangler/state), runs the client Worker
# with `wrangler dev` on that same state, and checks status codes, headers and counts with curl.
# Local only: it never touches staging or prod. Workers Cache is not exercised locally (D13).
set -euo pipefail

BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${SMOKE_PORT:-8788}"
BASE="http://127.0.0.1:${PORT}"
ORIGIN="http://localhost:3000" # listed in ALLOWED_ORIGINS of client/wrangler.toml [vars] (dev)
STATE_DIR="${BACKEND_DIR}/admin/.wrangler/state"
STARTUP_TIMEOUT="${SMOKE_STARTUP_TIMEOUT:-600}"
PUBLIC_CACHE="public, max-age=600"
WORK_DIR="$(mktemp -d -t smoke-master-data.XXXXXX)"
DEV_LOG="${WORK_DIR}/wrangler-dev.log"
FAILS=0
DEV_PID=""
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

cleanup() {
  if [[ -n "$DEV_PID" ]]; then
    kill "$DEV_PID" 2>/dev/null || true
    wait "$DEV_PID" 2>/dev/null || true
  fi
  pkill -f "wrangler dev --ip 127.0.0.1 --port ${PORT}" 2>/dev/null || true
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

ok() { echo "ok    $*"; }
fail() {
  echo "FAIL  $*"
  FAILS=$((FAILS + 1))
}

# check LABEL EXPECTED ACTUAL
check() {
  if [[ "$2" == "$3" ]]; then ok "$1"; else fail "$1: expected '$2', got '$3'"; fi
}

# request METHOD PATH [ORIGIN]: headers to $WORK_DIR/h, body to $WORK_DIR/b; prints the status.
request() {
  local args=(-s -X "$1" -D "${WORK_DIR}/h" -o "${WORK_DIR}/b" -w '%{http_code}')
  if [[ -n "${3:-}" ]]; then args+=(-H "Origin: $3"); fi
  curl "${args[@]}" "${BASE}$2"
}

# header NAME: value of that header in the last response, empty if absent.
header() {
  grep -i "^$1:" "${WORK_DIR}/h" | head -n 1 | cut -d ' ' -f 2- | tr -d '\r' || true
}

# js EXPR: evaluates a JavaScript expression over the last body, parsed as `b`.
js() {
  node -e 'const b = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); console.log(new Function("b", "return (" + process.argv[2] + ");")(b));' "${WORK_DIR}/b" "$1"
}

cd "$BACKEND_DIR"
echo "== migrate and seed local D1"
make migrate-local
make seed-local

echo "== start the client Worker on ${BASE} (first build can take minutes)"
(cd client && exec npx wrangler dev --ip 127.0.0.1 --port "$PORT" --persist-to "$STATE_DIR") >"$DEV_LOG" 2>&1 &
DEV_PID=$!
ready=""
for _ in $(seq 1 "$STARTUP_TIMEOUT"); do
  if curl -s -o /dev/null "${BASE}/v1/card-art"; then
    ready=1
    break
  fi
  if ! kill -0 "$DEV_PID" 2>/dev/null; then break; fi
  sleep 1
done
if [[ -z "$ready" ]]; then
  echo "client Worker did not start; wrangler dev log:"
  cat "$DEV_LOG"
  exit 1
fi

echo "== GET master data, without and with Origin"
for path in /v1/cards /v1/daily-cards /v1/card-art /v1/reading-config; do
  for origin in "" "$ORIGIN"; do
    label="GET ${path}${origin:+ (Origin)}"
    check "$label status" 200 "$(request GET "$path" "$origin")"
    check "$label Cache-Control" "$PUBLIC_CACHE" "$(header Cache-Control)"
    check "$label Vary" "Origin" "$(header Vary)"
    if [[ -n "$origin" ]]; then
      check "$label Access-Control-Allow-Origin" "$origin" "$(header Access-Control-Allow-Origin)"
    else
      check "$label without Access-Control-Allow-Origin" "" "$(header Access-Control-Allow-Origin)"
    fi
    if grep -Eq 'chatgpt\.com|source_url|raw_file' "${WORK_DIR}/b"; then
      fail "$label body contains provenance"
    else
      ok "$label body has no provenance"
    fi
  done
done

echo "== counts and types"
request GET /v1/cards >/dev/null
check "cards count" 78 "$(js 'b.length')"
check "cards without art" 4 "$(js 'b.filter(c => c.art === null).length')"
request GET /v1/daily-cards >/dev/null
check "daily cards count" 78 "$(js 'b.cards.length')"
check "daily null complicated texts" 22 "$(js 'b.cards.filter(c => c.content.love_by_status.complicated === null).length')"
check "daily missing_fields entries" 22 "$(js 'b.cards.reduce((n, c) => n + c.missing_fields.length, 0)')"
check "daily approved_for_web is boolean" boolean "$(js 'typeof b.cards[0].approved_for_web')"
check "daily auto_fallback is boolean" boolean "$(js 'typeof b.auto_fallback_to_general_or_spread')"
check "daily love status order" "single,crush,talking,partnered,ex_partner,complicated" "$(js 'b.love_statuses.map(s => s.key).join()')"
request GET /v1/card-art >/dev/null
check "card-art count" 74 "$(js 'b.cards.length')"
request GET /v1/reading-config >/dev/null
check "categories" 4 "$(js 'b.categories.length')"
check "situations (9 + 6 love)" 15 "$(js 'Object.values(b.situations).flat().length')"
check "suggestions" 12 "$(js 'Object.values(b.suggestions).flat().length')"
check "positions" 3 "$(js 'b.positions.length')"
check "reading love status order" "single,crush,talking,complicated,partnered,ex_partner" "$(js 'b.love_statuses.map(s => s.key).join()')"
check "uncategorized lists empty" 0 "$(js 'b.uncategorized_situations.length + b.uncategorized_suggestions.length')"

echo "== errors"
for path in /v1/cards /health; do
  check "POST ${path} status" 405 "$(request POST "$path")"
  check "POST ${path} Allow" "GET,HEAD" "$(header Allow)"
  check "POST ${path} Cache-Control" "no-store" "$(header Cache-Control)"
  check "POST ${path} code" METHOD_NOT_ALLOWED "$(js 'b.code')"
done
for method in GET POST; do
  check "${method} /v1/nope status" 404 "$(request "$method" /v1/nope)"
  check "${method} /v1/nope Cache-Control" "no-store" "$(header Cache-Control)"
  check "${method} /v1/nope code" NOT_FOUND "$(js 'b.code')"
done

echo "== /health (status is informational; staging has no R2 and answers 503 there)"
health_status="$(request GET /health)"
echo "info  GET /health status ${health_status}"
check "GET /health Cache-Control" "no-store" "$(header Cache-Control)"

echo
if ((FAILS > 0)); then
  echo "smoke-local: ${FAILS} check(s) failed"
  exit 1
fi
echo "smoke-local: all checks passed"
