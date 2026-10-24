#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-unknown-unknown
rm -rf dist
cp -r web dist
cp target/wasm32-unknown-unknown/release/kids_games.wasm dist/
echo "dist/ ready: serve it with  python3 -m http.server -d dist 8080"
