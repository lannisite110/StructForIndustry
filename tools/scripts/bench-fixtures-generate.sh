#!/usr/bin/env bash
# Populate tools/fixtures/bench/ with synthetic Gray8 frames (64×48).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

cargo run -q --release --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- gen-fixtures
echo "bench fixtures ready: tools/fixtures/bench/ok + defect"
