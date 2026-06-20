#!/usr/bin/env bash
# Julia + shm E2E: line-publisher → bus → defect-detect Julia sidecar
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

command -v julia >/dev/null || { echo "julia not installed"; exit 0; }

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-shm-julia-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
VISION_SOCK="$RUN_DIR/vision.sock"
HTTP_ADDR="127.0.0.1:18180"
SHM_NAME="sfi.julia.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-realtime.yaml"
SPC_STORE="$RUN_DIR/spc-trend.jsonl"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${JULIA_PID:-}" ]] && kill "$JULIA_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" "$SPC_STORE" 2>/dev/null || true
}
trap cleanup EXIT

echo "== build rust bins =="
cargo build -q -p sfi-core-bus
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/line-frame/Cargo.toml
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

echo "== julia defect-detect sidecar =="
SFI_VISION_SOCKET="$VISION_SOCK" \
  julia --project=domains/industrial-inspection/plugins/defect-detect \
  domains/industrial-inspection/plugins/defect-detect/server.jl &
JULIA_PID=$!

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
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

sleep 1

echo "== verify =="
STATS=$(curl -sf "http://$HTTP_ADDR/stats")
echo "$STATS" | grep -q '"task_done_published":1' || { echo "task not done"; exit 1; }

LAST=$(curl -sf "http://$HTTP_ADDR/results/last")
echo "$LAST" | grep -q '"verdict":"NG"' || { echo "expected NG: $LAST"; exit 1; }

TREND=$(curl -sf "http://$HTTP_ADDR/spc/trend?limit=1")
echo "$TREND" | grep -q 'ng_rate' || { echo "spc trend missing"; exit 1; }

echo "Julia shm E2E OK"
