#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
PROFILE=${PROFILE:-small}
OUT=${OUT:-dist}
start=$(date +%s)
export KG_ASSET_VERSION=${KG_ASSET_VERSION:-$(cat assets/art/*.kga assets/*.kgs | sha1sum | cut -c1-10)}
RUSTFLAGS="${RUSTFLAGS:-} --cfg getrandom_backend=\"wasm_js\" -C link-arg=--allow-undefined -C target-feature=+simd128" \
  cargo build -p kids_games --lib --profile "$PROFILE" --target wasm32-unknown-unknown
rm -rf "$OUT"
mkdir -p "$OUT"
cp web/index.html web/manifest.json web/icon.png web/_headers web/kg-mixer.js "$OUT/"
mkdir -p "$OUT/assets/art"
cp assets/art/*.kga "$OUT/assets/art/"
cp assets/sound.kgs assets/sound_words.kgs "$OUT/assets/"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name kids_games \
  "target/wasm32-unknown-unknown/$PROFILE/kids_games.wasm"
if command -v wasm-opt >/dev/null && [ "$PROFILE" != dev ]; then
  wasm-opt -Oz --enable-simd --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types \
    "$OUT/kids_games_bg.wasm" -o "$OUT/kids_games_bg.wasm"
fi
raw=$(stat -c %s "$OUT/kids_games_bg.wasm")
gz=$(gzip -9 -c "$OUT/kids_games_bg.wasm" | wc -c)
echo "wasm: $((raw / 1024)) KiB raw, $((gz / 1024)) KiB gzipped, built in $(( $(date +%s) - start )) s"
for f in "$OUT"/assets/*.kgs "$OUT"/assets/art/spell.kga; do echo "$(basename "$f"): $(( $(stat -c %s "$f") / 1024 )) KiB"; done
echo "$OUT/ ready: serve it with  python3 -m http.server -d $OUT 8080"
