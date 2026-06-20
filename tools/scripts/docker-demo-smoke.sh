#!/usr/bin/env bash
# Build and smoke-test AOI docker-compose demo (defect-detect + line-infer profiles).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEMO="$ROOT/domains/industrial-inspection/examples/aoi-line-demo"
cd "$DEMO"

cleanup() {
  docker compose down -v --remove-orphans 2>/dev/null || true
}
trap cleanup EXIT

wait_http() {
  local url=$1
  for _ in $(seq 1 90); do
    if curl -sf "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "timeout waiting for $url" >&2
  docker compose logs aoi-demo >&2 || true
  return 1
}

smoke_profile() {
  local profile=$1
  local expect_name=$2
  export SFI_PROFILE="/app/domains/industrial-inspection/profiles/${profile}"
  docker compose up -d --force-recreate aoi-demo
  wait_http "http://127.0.0.1:8080/health"
  curl -sf "http://127.0.0.1:8080/profile" | grep -q "\"name\":\"${expect_name}\""
  curl -sf "http://127.0.0.1:8080/metrics" | grep -q 'sfi_'
  for _ in $(seq 1 30); do
    LAST=$(curl -sf "http://127.0.0.1:8080/results/last" || echo "{}")
    if echo "$LAST" | grep -q '"verdict"'; then
      break
    fi
    sleep 1
  done
  echo "$LAST" | grep -q '"verdict":"NG"' || {
    echo "expected NG for $profile: $LAST" >&2
    return 1
  }
  docker compose stop aoi-demo
}

docker compose build aoi-demo
smoke_profile "line-realtime.yaml" "line-realtime"
smoke_profile "line-infer.yaml" "line-infer"
echo "docker demo smoke OK"
