#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=art/gen/out
mkdir -p $OUT sound
NAMES=$(cut -d' ' -f1 art/words.txt)
ssh spark "/tmp/rmenv/bin/python - /tmp/kg-gen/spellw $(echo $NAMES | sed 's/\([a-z]*\)/w_\1/g')" <<'PY' 2>&1 | grep -v "%" || true
import sys, os
from PIL import Image
from rembg import new_session, remove
s = new_session("birefnet-general")
d = sys.argv[1]
for n in sys.argv[2:]:
    dst = f"{d}/{n}_cut.png"
    if os.path.exists(dst) or not os.path.exists(f"{d}/{n}.png"):
        continue
    im = remove(Image.open(f"{d}/{n}.png"), session=s)
    im.crop(im.getchannel("A").getbbox()).save(dst)
PY
scp -q "spark:/tmp/kg-gen/spellw/*_cut.png" $OUT/
scp -q "spark:/tmp/maya_words/*.wav" /tmp/maya_words/ 2>/dev/null || { mkdir -p /tmp/maya_words; scp -q "spark:/tmp/maya_words/*.wav" /tmp/maya_words/; }
python3 - <<'PY'
import os, re, subprocess
words = [l.split()[0] for l in open("art/words.txt")]
have = [w for w in words if os.path.exists(f"art/gen/out/w_{w}_cut.png")]
old = set(l.split()[0] for l in open("art/spell.txt") if not l.startswith(("bg", "tile")))
keep = [l for l in open("art/spell.txt").read().splitlines() if l.split()[0] in old or l.startswith(("bg", "tile"))]
lines = [l for l in keep if not l.startswith("w_")]
base = {l.split()[0] for l in lines}
for w in have:
    if w not in base:
        lines.append(f"{w:<10}| gen:w_{w}_cut.png | max=256")
open("art/spell.txt", "w").write("\n".join(lines) + "\n")
print(len(have), "pictures")
PY
python3 - <<'PY'
import subprocess
from pathlib import Path
words = []
for l in open("crates/core/src/games/spell_words.txt"):
    if not l.startswith("["):
        words += l.split()
Path("sound/words").mkdir(parents=True, exist_ok=True)
lines = [l for l in open("sound.txt").read().splitlines() if not l.startswith("word_")]
n = 0
for w in words:
    src = Path(f"/tmp/maya_words/{w}.wav")
    dst = Path(f"sound/words/{w}.ogg")
    if not dst.exists():
        if not src.exists():
            continue
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(src), "-ac", "1", "-c:a", "libvorbis", "-q:a", "2", str(dst)], check=True)
    lines.append(f"word_{w} | repo:sound/words/{w}.ogg | db=-15")
    n += 1
open("sound.txt", "w").write("\n".join(lines) + "\n")
print(n, "voices")
PY
python3 scripts/assets.py art sound
