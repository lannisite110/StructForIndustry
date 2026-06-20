#!/usr/bin/env bash
# Validate all Cap'n Proto schemas in schema/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCHEMA_DIR="$ROOT/schema"

if ! command -v capnp >/dev/null 2>&1; then
  echo "capnp not found. Install Cap'n Proto: https://capnp.org/install.html"
  exit 1
fi

cd "$SCHEMA_DIR"
for f in *.capnp; do
  capnp compile -o- -I . "$f"
  echo "OK $f"
done

capnp compile -o c++ -I . sfi.capnp
echo "All schemas valid."
