#!/usr/bin/env bash
# AOI line demo — full Phase 3 path: bus + profile + mock vision + MES + line publisher
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-aoi-demo"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/sfi-bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="127.0.0.1:18080"
MES_ADDR="127.0.0.1:18090"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-realtime.yaml"
SHM_NAME="sfi.aoi.demo"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "$MOCK_PID" 2>/dev/null || true
  [[ -n "${MES_PID:-}" ]] && kill "$MES_PID" 2>/dev/null || true
  [[ -n "${LINE_PID:-}" ]] && kill "$LINE_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

echo "== mock defect-detect =="
SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" cargo run -q -p sfi-plugin-host --bin sfi-mock-defect-detect &
MOCK_PID=$!

echo "== MES stub =="
SFI_MES_ADDR="$MES_ADDR" cargo run -q --manifest-path domains/industrial-inspection/plugins/mes-reporter/Cargo.toml &
MES_PID=$!

echo "== sfi-bus (profile + MES) =="
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_MES_ENABLED=1 \
SFI_MES_ENDPOINT="http://$MES_ADDR/inspection/result" \
cargo run -q -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 60); do
  curl -sf "http://$HTTP_ADDR/health" >/dev/null 2>&1 && break
  sleep 0.1
done

echo "== line publisher (5 triggered frames) =="
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
SFI_LINE_FRAMES=5 \
SFI_LINE_INTERVAL_MS=200 \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml &
LINE_PID=$!

sleep 2

echo "== stats =="
curl -s "http://$HTTP_ADDR/stats" | head -c 500
echo ""

echo "== last MES result =="
curl -s "http://$MES_ADDR/inspection/last"
echo ""

echo "== last bus result =="
curl -s "http://$HTTP_ADDR/results/last"
echo ""

echo "Open http://$HTTP_ADDR/ for AOI preview UI"
echo "Phase 3 demo OK"
