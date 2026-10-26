#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE=${PROFILE:-small}
OUT=${OUT:-dist}
start=$(date +%s)
RUSTFLAGS="${RUSTFLAGS:-} --cfg getrandom_backend=\"wasm_js\"" \
  cargo build -p kids_games --lib --profile "$PROFILE" --target wasm32-unknown-unknown
rm -rf "$OUT"
mkdir -p "$OUT"
cp web/index.html web/manifest.json web/icon.png web/_headers "$OUT/"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name kids_games \
  "target/wasm32-unknown-unknown/$PROFILE/kids_games.wasm"
if command -v wasm-opt >/dev/null && [ "$PROFILE" != dev ]; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types \
    "$OUT/kids_games_bg.wasm" -o "$OUT/kids_games_bg.wasm"
fi
raw=$(stat -c %s "$OUT/kids_games_bg.wasm")
gz=$(gzip -9 -c "$OUT/kids_games_bg.wasm" | wc -c)
echo "wasm: $((raw / 1024)) KiB raw, $((gz / 1024)) KiB gzipped, built in $(( $(date +%s) - start )) s"
echo "$OUT/ ready: serve it with  python3 -m http.server -d $OUT 8080"
