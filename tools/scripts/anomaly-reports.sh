#!/usr/bin/env bash
# Generate the anomaly reports into docs/reports/.
# Uses bench fixtures when present; otherwise generates synthetic fixtures first.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OUT_DIR="$ROOT/docs/reports"
FIXTURE_ROOT="$ROOT/tools/fixtures/bench"
mkdir -p "$OUT_DIR"

if [[ ! -d "$FIXTURE_ROOT/ok" ]] || [[ -z "$(ls -A "$FIXTURE_ROOT/ok" 2>/dev/null || true)" ]]; then
  echo "generating bench fixtures ..."
  ./tools/scripts/bench-fixtures-generate.sh
fi

run() { cargo run -q --release --manifest-path plugins/ai-infer/Cargo.toml --bin sfi-anomaly -- "$@"; }

REPORT_ARGS=(report --fixture-root "$FIXTURE_ROOT")

echo "generating reports into $OUT_DIR (fixture-root=$FIXTURE_ROOT) ..."
run "${REPORT_ARGS[@]}" changeover > "$OUT_DIR/changeover.md"
run "${REPORT_ARGS[@]}" latency    > "$OUT_DIR/latency.md"
run "${REPORT_ARGS[@]}" illum      > "$OUT_DIR/illumination.md"
run "${REPORT_ARGS[@]}" errors     > "$OUT_DIR/error-rates.md"
run "${REPORT_ARGS[@]}" all        > "$OUT_DIR/anomaly-reports.md"

echo "wrote:"
ls -1 "$OUT_DIR"/*.md
