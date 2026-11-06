#!/bin/sh
set -e
cd "$(dirname "$0")"
OUT=${OUT:-../../../assets/models}
for name in ${@:-barrier hurdle gate train coin tree arch}; do
  gcad export "$name.gcad" "/tmp/prop_$name.raw.glb"
  python3 ../scale_glb.py "/tmp/prop_$name.raw.glb" "$OUT/prop_$name.glb" centre
  python3 ../../../scripts/glb_audit.py "$OUT/prop_$name.glb" 4000 | grep -v TEXCOORD || true
done
