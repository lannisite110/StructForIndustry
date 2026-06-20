#!/usr/bin/env bash
# MindVision mock capture E2E — mock backend → HAL → mock defect-detect.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-mindvision-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="127.0.0.1:18190"
SHM_NAME="sfi.mindvision.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/lab-batch.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "$MOCK_PID" 2>/dev/null || true
  [[ -n "${MV_PID:-}" ]] && kill "$MV_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus -p sfi-plugin-host
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/mindvision-capture/Cargo.toml

SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
  cargo run -q -p sfi-plugin-host --bin sfi-mock-defect-detect &
MOCK_PID=$!

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_SCHEDULER=1 \
cargo run -q -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 80); do
  curl -sf "http://$HTTP_ADDR/health" >/dev/null 2>&1 && break
  sleep 0.1
done

SFI_MINDVISION_MOCK=1 \
SFI_MINDVISION_FRAMES=2 \
SFI_MINDVISION_FPS=5 \
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/mindvision-capture/Cargo.toml &
MV_PID=$!

sleep 2

STATS=$(curl -sf "http://$HTTP_ADDR/stats")
echo "$STATS" | grep -q '"task_done_published":' || { echo "no task done: $STATS"; exit 1; }

echo "mindvision mock E2E OK"
