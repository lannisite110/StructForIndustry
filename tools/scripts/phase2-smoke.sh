#!/usr/bin/env bash
# Phase 2 smoke: workspace tests + sfi-cli domain list
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "== validate schemas =="
./core/contracts/scripts/validate-schemas.sh

echo "== cargo test =="
cargo test --workspace

echo "== sfi-cli domain list =="
cargo run -p sfi-cli --quiet -- domain list

echo "Phase 2 smoke OK"
