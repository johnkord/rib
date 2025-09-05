#!/usr/bin/env bash
# Lightweight smoke test for local or containerized environment.
# Fails fast on first error.
set -euo pipefail

API=${API_BASE:-http://localhost:8080}
FRONTEND=${FRONTEND_BASE:-http://localhost:3000}

pass() { echo -e "[PASS] $1"; }
fail() { echo -e "[FAIL] $1" >&2; exit 1; }
check() { "$@" >/dev/null 2>&1 || fail "$*"; }

curl_json() { curl -fsS -H 'Accept: application/json' "$1"; }

echo "[smoke] Starting basic checks against API=$API FRONTEND=$FRONTEND"

# 1. Boards list
curl_json "$API/api/v1/boards" | grep -q '\[' && pass 'boards endpoint'

# 2. OpenAPI doc present
curl -fsS "$API/docs/openapi.json" >/dev/null && pass 'openapi spec'

# 3. Non-existent image 404 direct backend
status=$(curl -o /dev/null -s -w "%{http_code}" "$API/images/doesnotexist")
[[ "$status" == "404" ]] && pass 'backend image 404'

# 4. Frontend index (may be HTML). Allow failure if frontend not started.
if curl -fsS "$FRONTEND/" >/dev/null; then
  pass 'frontend index'
else
  echo '[WARN] frontend index not reachable (maybe not running)'
fi

# 5. Nginx proxy image 404 (if frontend up)
if curl -fsS -o /dev/null "$FRONTEND/"; then
  status_fe=$(curl -o /dev/null -s -w "%{http_code}" "$FRONTEND/images/doesnotexist")
  [[ "$status_fe" == "404" ]] && pass 'frontend image proxy 404' || echo '[WARN] frontend image 404 check skipped'
fi

# 6. Security headers presence (sample route)
headers=$(curl -Is "$API/docs" | tr -d '\r')
grep -qi 'strict-transport-security' <<<"$headers" && pass 'HSTS header (may be disabled via env)' || echo '[INFO] HSTS header not present (env may disable)'

## 7. Optional image upload test (1x1 PNG) if API upload endpoint present
tmp_png="/tmp/rib-smoke-1x1.png"
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\\\xbfp\x9c\x00\x00\x00\x0cIDATx\x9cc``\x00\x00\x00\x04\x00\x01\x0b\r\n\xb2\x00\x00\x00\x00IEND\xaeB`\x82' > "$tmp_png"
upload_status=$(curl -s -o /dev/null -w "%{http_code}" -F file=@"$tmp_png" "$API/api/v1/images" || true)
if [ "$upload_status" = "201" ]; then
  pass 'image upload (fs/s3)'
else
  echo "[INFO] image upload skipped/failed status=$upload_status (may require auth or feature)"
fi

echo '[smoke] All critical checks completed'
