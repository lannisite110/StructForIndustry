#!/usr/bin/env bash
# ai-infer profile E2E: line-infer.yaml → mock ai-infer → frame archive
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-ai-infer-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
INFER_SOCK="$RUN_DIR/infer.sock"
HTTP_ADDR="127.0.0.1:18182"
SHM_NAME="sfi.ai.infer.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-infer.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "$MOCK_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus -p sfi-plugin-host

SFI_INFER_SOCKET="$INFER_SOCK" cargo run -q -p sfi-plugin-host --bin sfi-mock-ai-infer &
MOCK_PID=$!

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_INFER_SOCKET="$INFER_SOCK" \
SFI_VISION_PLUGIN_SOCKET="$INFER_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_SCHEDULER=1 \
SFI_DATA_DIR="$RUN_DIR/data" \
cargo run -q -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 80); do
  curl -sf "http://$HTTP_ADDR/health" >/dev/null 2>&1 && break
  sleep 0.1
done

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
SFI_LINE_FRAMES=1 \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

sleep 1

LAST=$(curl -sf "http://$HTTP_ADDR/results/last")
echo "$LAST" | grep -q '"verdict":"NG"' || { echo "expected NG: $LAST"; exit 1; }
echo "$LAST" | grep -q 'image_path' || { echo "missing image_path: $LAST"; exit 1; }

echo "ai-infer E2E OK"
