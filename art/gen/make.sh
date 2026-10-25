#!/usr/bin/env bash
set -euo pipefail
JOBS=${1:?usage: art/gen/make.sh art/gen/prompts/<game>.json}
NAME=$(basename "$JOBS" .json)
HERE=$(cd "$(dirname "$0")" && pwd)
REMOTE=/tmp/kg-gen/$NAME
ssh spark "mkdir -p $REMOTE"
python3 - "$JOBS" "$REMOTE" > /tmp/kg-$NAME-jobs.json <<'EOF'
import json, sys
jobs = json.load(open(sys.argv[1]))
for j in jobs:
    j["out"] = f"{sys.argv[2]}/{j['name']}.png"
json.dump(jobs, sys.stdout)
EOF
scp -q "$HERE/flux.py" "spark:$REMOTE/flux.py"
scp -q /tmp/kg-$NAME-jobs.json "spark:$REMOTE/jobs.json"
ssh spark "cd $REMOTE && python3 flux.py jobs.json"
CUT=$(python3 -c "import json,sys; print(' '.join(j['name'] for j in json.load(open(sys.argv[1])) if j.get('cut')))" "$JOBS")
if [ -n "$CUT" ]; then
  ssh spark "/tmp/rmenv/bin/python - $REMOTE $CUT" <<'EOF' 2>&1 | grep -v "%" || true
import sys
from PIL import Image
from rembg import new_session, remove
s = new_session("birefnet-general")
d = sys.argv[1]
for n in sys.argv[2:]:
    im = remove(Image.open(f"{d}/{n}.png"), session=s)
    im.crop(im.getchannel("A").getbbox()).save(f"{d}/{n}_cut.png")
EOF
fi
mkdir -p "$HERE/out"
scp -q "spark:$REMOTE/*.png" "$HERE/out/"
echo "art/gen/out: $(python3 -c "import json,sys; print(' '.join(j['name'] + ('_cut' if j.get('cut') else '') + '.png' for j in json.load(open(sys.argv[1]))))" "$JOBS")"
