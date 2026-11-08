#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --release -p kids_games --example bake
cargo run --release -p kids_games --example bake -- --check
if ! ls assets/gcad-baked/*.bin >/dev/null 2>&1; then
  echo "bake failed: assets/gcad-baked has no models" >&2
  exit 1
fi
