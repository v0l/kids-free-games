#!/bin/sh
set -e
cd "$(dirname "$0")"
OUT=${1:-../../../assets/models/hero.glb}
gcad export hero.gcad /tmp/hero_raw.glb
python3 scale_glb.py /tmp/hero_raw.glb "$OUT" 31
python3 ../../../scripts/glb_audit.py "$OUT" 5000
