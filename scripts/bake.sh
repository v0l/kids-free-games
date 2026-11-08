#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --release -p kids_games --example bake
cargo run --release -p kids_games --example bake -- --check
