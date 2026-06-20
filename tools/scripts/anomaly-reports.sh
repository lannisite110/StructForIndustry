#!/usr/bin/env bash
# Generate the three anomaly reports (changeover / latency / illumination)
# into docs/reports/. Release build for representative latency numbers.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OUT_DIR="$ROOT/docs/reports"
mkdir -p "$OUT_DIR"

run() { cargo run -q --release --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- "$@"; }

echo "generating reports into $OUT_DIR ..."
run report changeover > "$OUT_DIR/changeover.md"
run report latency    > "$OUT_DIR/latency.md"
run report illum      > "$OUT_DIR/illumination.md"
run report errors     > "$OUT_DIR/error-rates.md"
run report all        > "$OUT_DIR/anomaly-reports.md"

echo "wrote:"
ls -1 "$OUT_DIR"/*.md
