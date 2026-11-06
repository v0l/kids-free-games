#!/bin/sh
set -e
cd "$(dirname "$0")"
OUT=${OUT:-../../../assets/models}
audit() { python3 ../../../scripts/glb_audit.py "$1" 4000 | grep -v TEXCOORD || true; }
for name in ${@:-barrier hurdle gate train coin arch road buildings}; do
  case $name in
    road)
      gcad export road.gcad "$OUT/prop_road.glb" --scale 0.01 --centre
      audit "$OUT/prop_road.glb"
      ;;
    buildings)
      for v in 1:2 2:3 3:4 4:5 5:3; do
        style=${v%%:*} floors=${v##*:}
        gcad export building.gcad "$OUT/prop_building$style.glb" --set style=$style --set floors=$floors --scale 0.01 --centre
        audit "$OUT/prop_building$style.glb"
      done
      ;;
    *)
      gcad export "$name.gcad" "$OUT/prop_$name.glb" --height 1 --centre
      audit "$OUT/prop_$name.glb"
      ;;
  esac
done
