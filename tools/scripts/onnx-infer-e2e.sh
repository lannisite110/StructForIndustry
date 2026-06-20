#!/usr/bin/env bash
# ONNX infer E2E: tiny-defect.onnx → ai-infer (ort) → bus → NG verdict.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

MODEL="$ROOT/tools/fixtures/models/tiny-defect.onnx"
if [[ ! -f "$MODEL" ]]; then
  python3 "$ROOT/tools/scripts/gen-tiny-onnx.py"
fi

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-onnx-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
INFER_SOCK="$RUN_DIR/infer.sock"
HTTP_ADDR="127.0.0.1:18187"
SHM_NAME="sfi.onnx.e2e"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-infer.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${INFER_PID:-}" ]] && kill "$INFER_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus
cargo build -q --manifest-path plugins/ai-infer/Cargo.toml

SFI_ONNX_MODEL="$MODEL" \
SFI_INFER_SOCKET="$INFER_SOCK" \
cargo run -q --manifest-path plugins/ai-infer/Cargo.toml &
INFER_PID=$!

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_BUS_HTTP="$HTTP_ADDR" \
SFI_INFER_SOCKET="$INFER_SOCK" \
SFI_VISION_PLUGIN_SOCKET="$INFER_SOCK" \
SFI_PROFILE="$PROFILE" \
SFI_SCHEDULER=1 \
cargo run -q -p sfi-core-bus --bin sfi-bus &
BUS_PID=$!

for _ in $(seq 1 80); do
  curl -sf "http://$HTTP_ADDR/health" >/dev/null 2>&1 && break
  sleep 0.1
done

SFI_BUS_SOCKET="$BUS_SOCK" \
SFI_LINE_SHM="$SHM_NAME" \
SFI_LINE_FRAMES=1 \
SFI_ONNX_E2E=1 \
cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

sleep 2

LAST=$(curl -sf "http://$HTTP_ADDR/results/last")
echo "$LAST" | grep -q '"verdict":"NG"' || { echo "expected NG: $LAST"; exit 1; }

echo "onnx infer E2E OK"
