#!/usr/bin/env bash
# Measure E2E: line-publisher → bus → vision sidecar (Julia default, mock Rust fast path fallback)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-measure-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="127.0.0.1:18181"
SHM_NAME="sfi.measure.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-measure-e2e.yaml"
FRAME_FILE="$RUN_DIR/edge-frame.raw"
SPC_STORE="$RUN_DIR/spc-trend.jsonl"
EDGE_X=32
SCAN_Y=24

USE_JULIA=0
if command -v julia >/dev/null 2>&1; then
  USE_JULIA=1
fi

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${VISION_PID:-}" ]] && kill "$VISION_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" "$FRAME_FILE" "$SPC_STORE" 2>/dev/null || true
}
trap cleanup EXIT

echo "== build rust bins =="
cargo build -q -p sfi-core-bus
cargo build -q -p sfi-plugin-host
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/line-frame/Cargo.toml
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

echo "== synthetic edge frame (edge_x=$EDGE_X row=$SCAN_Y) =="
python3 - "$FRAME_FILE" "$EDGE_X" "$SCAN_Y" <<'PY'
import sys
path, edge_x, scan_y = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
w, h = 64, 48
buf = bytearray(w * h)
for y in range(h):
    for x in range(w):
        buf[y * w + x] = 30 if x < edge_x else 220
if 0 < edge_x < w:
    buf[scan_y * w + edge_x] = 125
open(path, "wb").write(buf)
PY

if [[ "$USE_JULIA" -eq 1 ]]; then
  echo "== julia defect-detect sidecar (pure Julia measure) =="
  SFI_VISION_SOCKET="$VISION_SOCK" \
    julia --project=domains/industrial-inspection/plugins/defect-detect \
    domains/industrial-inspection/plugins/defect-detect/server.jl &
else
  echo "== mock defect-detect sidecar (Rust shm_gray8 fast path) =="
  SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
    cargo run -q -p sfi-plugin-host --bin sfi-mock-defect-detect &
fi
VISION_PID=$!

echo "== sfi-bus =="
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_VISION_PLUGIN_SOCKET="$VISION_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_SCHEDULER=1 \
SFI_SPC_STORE="$SPC_STORE" \
cargo run -q -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 80); do
  curl -sf "http://$HTTP_ADDR/health" >/dev/null 2>&1 && break
  sleep 0.1
done

echo "== publish one shm frame =="
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
SFI_LINE_FRAMES=1 \
SFI_LINE_FRAME_FILE="$FRAME_FILE" \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

sleep 1

echo "== verify =="
STATS=$(curl -sf "http://$HTTP_ADDR/stats")
echo "$STATS" | grep -q '"task_done_published":1' || { echo "task not done: $STATS"; exit 1; }

LAST=$(curl -sf "http://$HTTP_ADDR/results/last")
echo "$LAST" | grep -q '"verdict":"OK"' || { echo "expected OK measure: $LAST"; exit 1; }

SPC=$(curl -sf "http://$HTTP_ADDR/spc/metrics")
echo "$SPC" | grep -q 'edge_position_px' || { echo "spc metrics missing edge: $SPC"; exit 1; }

echo "Measure E2E OK (julia=$USE_JULIA)"
