#!/usr/bin/env bash
# 1080p synthetic frame latency benchmark (mock defect-detect)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
cargo test -p sfi-core-bus bench_1080p_pipeline_under_budget -- --nocapture
