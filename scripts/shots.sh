#!/usr/bin/env bash
set -euo pipefail
W=${1:-1024}
H=${2:-768}
OUT=${OUT:-/tmp/kgshots}
PORT=${PORT:-8765}
cd "$(dirname "$0")/.."
cargo build --example shots --target wasm32-unknown-unknown --release
rm -rf "$OUT"
mkdir -p "$OUT"
cp web/mq_js_bundle.js target/wasm32-unknown-unknown/release/examples/shots.wasm "$OUT/"
cat > "$OUT/index.html" <<'EOF'
<!DOCTYPE html><html><head><meta charset="utf-8"><style>html,body,canvas{margin:0;padding:0;width:100%;height:100%;overflow:hidden;position:fixed}</style></head>
<body><canvas id="glcanvas" tabindex="1"></canvas><script>var register_plugin;</script><script src="mq_js_bundle.js"></script>
<script>
miniquad_add_plugin({register_plugin: function(o){ o.env.kg_save_len = function(){ return 0; }; o.env.kg_save_read = function(){}; o.env.kg_save_write = function(){}; o.env.kg_http_start = function(){ return 1; }; o.env.kg_http_status = function(){ return -1; }; o.env.kg_http_len = function(){ return 0; }; o.env.kg_http_read = function(){}; o.env.kg_http_free = function(){}; o.env.kg_jev_len = function(){ return 0; }; o.env.kg_jev_read = function(){}; o.env.shot_index = function(){ return parseInt(location.hash.slice(1)||"0"); }; o.env.shot_inset = function(){ return parseInt(new URLSearchParams(location.search).get("inset")||"0"); }; }, name: "shots", version: 1});
load("shots.wasm");
</script></body></html>
EOF
python3 -m http.server "$PORT" -d "$OUT" >/dev/null 2>&1 &
srv=$!
trap 'kill $srv' EXIT
sleep 1
COUNT=$(grep -c "^        Entry {" src/games/mod.rs)
for i in ${ONLY:-$(seq 0 "$COUNT")}; do
  google-chrome --headless=new --no-sandbox --use-angle=swiftshader --enable-unsafe-swiftshader --hide-scrollbars \
    --window-size="$W,$H" --virtual-time-budget=4000 --screenshot="$OUT/$i.png" "http://localhost:$PORT/index.html?inset=${INSET:-0}#$i" 2>/dev/null
done
python3 - "$OUT" <<'EOF'
import sys
from PIL import Image
out = sys.argv[1]
import glob
ims = [Image.open(f) for f in sorted(glob.glob(f"{out}/[0-9]*.png"), key=lambda f: int(f.split("/")[-1][:-4]))]
w, h = ims[0].size
cols = 3
rows = (len(ims) + cols - 1) // cols
sheet = Image.new("RGB", (w * cols, h * rows), "white")
for i, im in enumerate(ims):
    sheet.paste(im, ((i % cols) * w, (i // cols) * h))
sheet.thumbnail((1800, 1800 * rows))
sheet.save(f"{out}/sheet.png")
print(f"{out}/sheet.png")
EOF
