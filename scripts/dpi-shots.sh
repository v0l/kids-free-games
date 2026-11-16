#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=${BIN:-target/release/kids_games}
OUT=${OUT:-/tmp/kgdpi}
WAIT=${WAIT:-8}
SIZES=${SIZES:-"1024x768 1280x720 844x390 390x844"}
DPIS=${DPIS:-"1 1.25 1.5 2 3"}
SCENES=${ONLY:-$(grep -oE '^    \(([0-9]+), "' crates/app/src/shots.rs | grep -oE '[0-9]+')}
mkdir -p "$OUT"
rt=$(mktemp -d /tmp/agent-wl.XXXX)
chmod 700 "$rt"
printf 'xwayland disable\noutput HEADLESS-1 resolution %s\ndefault_border none\nfor_window [app_id=".*"] floating enable, move position 0 0\n' "${OUTPUT:-3200x3200}" > "$rt/config"
E="env -i HOME=$HOME PATH=$PATH XDG_RUNTIME_DIR=$rt"
$E WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 sway -c "$rt/config" >"$rt/sway.log" 2>&1 &
sway=$!
trap 'kill $sway 2>/dev/null; rm -rf "$rt"' EXIT
sleep 2
for size in $SIZES; do
  for dpi in $DPIS; do
    for n in $SCENES; do
      f="$OUT/${n}_${size}_x${dpi}.png"
      $E WAYLAND_DISPLAY=wayland-1 KG_LOG_VIEW=1 KG_SAVE_DIR="$rt/save" "$BIN" --shot "$n" --size "$size" --dpi "$dpi" ${EXTRA:-} >"$rt/app.log" 2>&1 &
      app=$!
      sleep "$WAIT"
      $E WAYLAND_DISPLAY=wayland-1 grim "$f"
      kill $app 2>/dev/null || true
      wait $app 2>/dev/null || true
      echo "$f $(grep -m1 'view logical' "$rt/app.log" || true)"
    done
  done
done
