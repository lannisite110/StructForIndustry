#!/usr/bin/env bash
# PLC trigger E2E: TRIG socket → HAL frame → mock defect-detect → task.done
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-plc-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
PLC_SOCK="$RUN_DIR/plc.sock"
HTTP_ADDR="127.0.0.1:18181"
SHM_NAME="sfi.plc.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-realtime.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "$MOCK_PID" 2>/dev/null || true
  [[ -n "${PLC_PID:-}" ]] && kill "$PLC_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus -p sfi-plugin-host
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/plc-trigger/Cargo.toml

SFI_VISION_SOCKET="$VISION_SOCK" \
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

SFI_PLC_SOCKET="$PLC_SOCK" \
SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/plc-trigger/Cargo.toml &
PLC_PID=$!

for _ in $(seq 1 80); do
  [[ -S "$PLC_SOCK" ]] && break
  sleep 0.1
done
[[ -S "$PLC_SOCK" ]] || { echo "plc socket missing"; exit 1; }

python3 -c "
import socket, sys
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.connect('$PLC_SOCK')
s.sendall(b'TRIG')
ack = s.recv(4)
assert ack == b'ACK\\0', ack
print('PLC ACK OK')
"

sleep 1

STATS=$(curl -sf "http://$HTTP_ADDR/stats")
echo "$STATS" | grep -q '"task_done_published":1' || { echo "task not done: $STATS"; exit 1; }

LAST=$(curl -sf "http://$HTTP_ADDR/results/last")
echo "$LAST" | grep -q '"verdict":"NG"' || { echo "expected NG: $LAST"; exit 1; }

echo "PLC trigger E2E OK"
