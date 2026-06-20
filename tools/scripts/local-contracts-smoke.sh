#!/usr/bin/env bash
# Local smoke matching .github/workflows/contracts.yml validate-and-test job.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "== validate schemas =="
./core/contracts/scripts/validate-schemas.sh

echo "== cargo test =="
cargo test --workspace --all-targets

echo "== cargo fmt =="
cargo fmt --all -- --check

echo "== cargo clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== 1080p bench =="
./tools/scripts/bench-1080p.sh
./tools/scripts/bench-1080p-report.sh /tmp/sfi-bench-1080p.json

echo "== E2E scripts =="
chmod +x tools/scripts/*-e2e.sh tools/scripts/bench-1080p-report.sh 2>/dev/null || true
./tools/scripts/ai-infer-e2e.sh
./tools/scripts/onnx-infer-e2e.sh
./tools/scripts/gige-capture-e2e.sh
./tools/scripts/mindvision-capture-e2e.sh
./tools/scripts/modbus-plc-e2e.sh
./tools/scripts/opcua-plc-e2e.sh

if [[ -e "${SFI_V4L2_DEVICE:-/dev/video42}" ]]; then
  ./tools/scripts/v4l2-capture-e2e.sh
  ./tools/scripts/v4l2-trigger-e2e.sh
  ./tools/scripts/modbus-v4l2-e2e.sh
else
  echo "skip v4l2 E2E: ${SFI_V4L2_DEVICE:-/dev/video42} not present"
fi

echo "local contracts smoke OK"
