#!/usr/bin/env bash
# Emit 1080p bench latency as JSON for CI artifacts
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
OUT="${1:-/tmp/sfi-bench-1080p.json}"
LOG=$(mktemp)
cargo test -p sfi-core-bus bench_1080p_pipeline_under_budget -- --nocapture 2>&1 | tee "$LOG"
LAT=$(grep -oP '1080p pipeline latency: \K[0-9.]+[a-z]+' "$LOG" | tail -1 || echo "unknown")
python3 - <<PY
import json, sys
lat = "$LAT"
ms = None
if lat.endswith("ms"):
    ms = float(lat.replace("ms",""))
elif lat.endswith("s"):
    ms = float(lat.replace("s","")) * 1000
print(json.dumps({"latency_raw": lat, "latency_ms": ms, "budget_ms": 500}, indent=2))
open("$OUT","w").write(json.dumps({"latency_raw": lat, "latency_ms": ms, "budget_ms": 500}, indent=2))
PY
echo "Wrote $OUT"
