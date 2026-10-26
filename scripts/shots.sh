#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
W=${1:-1024}
H=${2:-768}
OUT=${OUT:-/tmp/kgshots}
PORT=${PORT:-8765}
WAIT=${WAIT:-6000}
SITE=${SITE:-/tmp/kgshots-site}
if [ -z "${NO_BUILD:-}" ]; then
  PROFILE=${PROFILE:-release} OUT="$SITE" scripts/web.sh >/dev/null
fi
mkdir -p "$OUT"
rm -f "$OUT"/[0-9]*.png
python3 -m http.server "$PORT" -d "$SITE" >/dev/null 2>&1 &
srv=$!
trap 'kill $srv' EXIT
sleep 1
SCENES=$(grep -oE '^    \(([0-9]+), "' crates/app/src/shots.rs | grep -oE '[0-9]+')
for i in ${ONLY:-$SCENES}; do
  node scripts/shot.js "http://localhost:$PORT/index.html?shot=$i&inset=${INSET:-0}" "$OUT/$i.png" "$W" "$H" "$WAIT" &
  while [ "$(jobs -r | wc -l)" -ge "${JOBS:-4}" ]; do sleep 0.2; done
done
wait $(jobs -p | grep -v "^$srv$") 2>/dev/null || true
python3 - "$OUT" <<'EOF'
import sys, glob
from PIL import Image
out = sys.argv[1]
files = sorted(glob.glob(f"{out}/[0-9]*.png"), key=lambda f: int(f.split("/")[-1][:-4]))
ims = [Image.open(f) for f in files]
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
