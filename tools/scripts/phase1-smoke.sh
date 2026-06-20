#!/usr/bin/env bash
# Phase 1 smoke test: sfi-bus + synthetic HAL frames → /stats
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RUNTIME="${XDG_RUNTIME_DIR:-/tmp}"
SOCKET="${SFI_BUS_SOCKET:-$RUNTIME/sfi-bus-sandbox.sock}"
HTTP="${SFI_BUS_HTTP:-127.0.0.1:18080}"
CAPTURE_FRAMES=20

export SFI_BUS_SOCKET="$SOCKET"
export SFI_BUS_HTTP="$HTTP"
export SFI_CAPTURE_FRAMES="$CAPTURE_FRAMES"

cd "$ROOT"
cargo build -p sfi-core-bus --bin sfi-bus
cargo build -p sfi-hal-capture --bin sfi-capture

BUS_PID=""
cleanup() {
  if [[ -n "$BUS_PID" ]]; then
    kill "$BUS_PID" 2>/dev/null || true
    wait "$BUS_PID" 2>/dev/null || true
  fi
  rm -f "$SOCKET"
}
trap cleanup EXIT

cargo run -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 50); do
  if [[ -S "$SOCKET" ]]; then
    break
  fi
  sleep 0.05
done
[[ -S "$SOCKET" ]] || { echo "bus socket not ready"; exit 1; }

# Rust synthetic HAL capture (replaces the former Zig sfi-capture)
cargo run -q -p sfi-hal-capture --bin sfi-capture

FRAMES="$(curl -sf "http://$HTTP/stats" | grep -o '"frames_received":[0-9]*' | cut -d: -f2)"
echo "frames_received=$FRAMES"
[[ "${FRAMES:-0}" -ge 1 ]] || { echo "expected frames on /stats"; exit 1; }

echo "Phase 1 smoke test OK"
