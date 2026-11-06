#!/bin/sh
set -e
cd "$(dirname "$0")"
OUT=${OUT:-../../../assets/models}
audit() { python3 ../../../scripts/glb_audit.py "$1" 4000 | grep -v TEXCOORD || true; }
for name in ${@:-barrier hurdle gate train coin tree arch road buildings}; do
  case $name in
    road)
      gcad export road.gcad /tmp/prop_road.raw.glb
      python3 ../scale_glb.py /tmp/prop_road.raw.glb "$OUT/prop_road.glb" centre metres
      audit "$OUT/prop_road.glb"
      ;;
    buildings)
      for v in 1:2 2:3 3:4 4:5 5:3; do
        style=${v%%:*} floors=${v##*:}
        gcad export building.gcad /tmp/prop_building$style.raw.glb --set style=$style --set floors=$floors
        python3 ../scale_glb.py /tmp/prop_building$style.raw.glb "$OUT/prop_building$style.glb" centre metres
        audit "$OUT/prop_building$style.glb"
      done
      ;;
    *)
      gcad export "$name.gcad" "/tmp/prop_$name.raw.glb"
      python3 ../scale_glb.py "/tmp/prop_$name.raw.glb" "$OUT/prop_$name.glb" centre
      audit "$OUT/prop_$name.glb"
      ;;
  esac
done
