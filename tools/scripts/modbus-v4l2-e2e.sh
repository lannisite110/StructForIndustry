#!/usr/bin/env bash
# Modbus + V4L2 E2E — coil rising edge captures from loopback camera.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

DEVICE="${SFI_V4L2_DEVICE:-/dev/video42}"
chmod +x tools/scripts/setup-v4l2loopback.sh
SFI_V4L2_LOOPBACK_NR="${DEVICE#/dev/video}" tools/scripts/setup-v4l2loopback.sh || true

if [[ ! -e "$DEVICE" ]]; then
  echo "skip modbus+v4l2 E2E: $DEVICE not present"
  exit 0
fi

export SFI_V4L2_DEVICE="$DEVICE"
export SFI_V4L2_WIDTH=320
export SFI_V4L2_HEIGHT=240
export SFI_V4L2_SETUP_LOOPBACK=0

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-modbus-v4l2-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="127.0.0.1:18188"
SHM_NAME="sfi.modbus.v4l2.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-realtime.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "$MOCK_PID" 2>/dev/null || true
  [[ -n "${MODBUS_PID:-}" ]] && kill "$MODBUS_PID" 2>/dev/null || true
  pkill -f "ffmpeg.*${DEVICE}" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus -p sfi-plugin-host
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/modbus-plc-trigger/Cargo.toml

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

SFI_MODBUS_MOCK=1 \
SFI_MODBUS_POLL_MS=50 \
SFI_V4L2_DEVICE="$DEVICE" \
SFI_V4L2_WIDTH=320 \
SFI_V4L2_HEIGHT=240 \
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/modbus-plc-trigger/Cargo.toml &
MODBUS_PID=$!

for _ in $(seq 1 60); do
  STATS=$(curl -sf "http://$HTTP_ADDR/stats" || echo "{}")
  if echo "$STATS" | grep -q '"task_done_published":1'; then
    break
  fi
  sleep 0.1
done

STATS=$(curl -sf "http://$HTTP_ADDR/stats")
echo "$STATS" | grep -q '"task_done_published":1' || { echo "task not done: $STATS"; exit 1; }

echo "modbus+v4l2 E2E OK ($DEVICE)"
