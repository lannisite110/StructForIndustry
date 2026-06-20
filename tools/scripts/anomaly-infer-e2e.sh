#!/usr/bin/env bash
# OK-only anomaly E2E: calibrated model → ai-infer → bus.
#   defect frame  → NG verdict
#   OK frame      → OK verdict (no false positive)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RUN_DIR="${XDG_RUNTIME_DIR:-/tmp}/sfi-anomaly-e2e"
mkdir -p "$RUN_DIR"
BUS_SOCK="$RUN_DIR/bus.sock"
INFER_SOCK="$RUN_DIR/infer.sock"
HTTP_ADDR="127.0.0.1:18191"
SHM_NAME="sfi.anomaly.e2e"
MODEL="$RUN_DIR/anomaly-ok.json"
OK_FRAME="$RUN_DIR/ok.gray8"
NG_FRAME="$RUN_DIR/ng.gray8"
PROFILE="$ROOT/domains/industrial-inspection/profiles/line-infer.yaml"

cleanup() {
  [[ -n "${BUS_PID:-}" ]] && kill "$BUS_PID" 2>/dev/null || true
  [[ -n "${INFER_PID:-}" ]] && kill "$INFER_PID" 2>/dev/null || true
  rm -f "/dev/shm/$SHM_NAME" 2>/dev/null || true
}
trap cleanup EXIT

cargo build -q -p sfi-core-bus
cargo build -q --manifest-path plugins/ai-infer/Cargo.toml
cargo build -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml

# Fresh OK-only calibration + OK/NG sample frames.
cargo run -q --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  calibrate --ok 20 --out "$MODEL"
cargo run -q --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  dump --kind ok --out "$OK_FRAME"
cargo run -q --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- \
  dump --kind ng --out "$NG_FRAME"

SFI_ANOMALY_MODEL="$MODEL" \
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

publish_and_check() {
  local frame_file="$1" expect="$2"
  SFI_BUS_SOCKET="$BUS_SOCK" \
  SFI_LINE_SHM="$SHM_NAME" \
  SFI_LINE_FRAMES=1 \
  SFI_LINE_FRAME_FILE="$frame_file" \
  cargo run -q --manifest-path domains/industrial-inspection/hal-ext/line-publisher/Cargo.toml
  sleep 1.5
  local last
  last=$(curl -sf "http://$HTTP_ADDR/results/last")
  echo "  result: $last"
  echo "$last" | grep -q "\"verdict\":\"$expect\"" || {
    echo "expected $expect verdict but got: $last"; exit 1;
  }
}

echo "[1/2] defect frame should be NG"
publish_and_check "$NG_FRAME" "NG"
echo "[2/2] OK frame should be OK"
publish_and_check "$OK_FRAME" "OK"

echo "anomaly infer E2E OK"
