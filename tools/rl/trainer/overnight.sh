#!/usr/bin/env bash
set -u
cd "$(dirname "$0")"
out=${OUT:-/tmp/kgrl/clash_kill.kgb}
log=${LOG:-/tmp/kgrl/clash_kill.log}
start=${START:-}
for attempt in 1 2 3 4 5; do
  resume=()
  [ -n "$start" ] && resume=(--resume "$start")
  [ -f "$out.safetensors" ] && resume=(--resume "$out.safetensors")
  echo "=== overnight attempt $attempt $(date) ${resume[*]}" >> "$log"
  ./target/release/kg-trainer --game clash --bf16 1 --league-servers 4 --pop 8 --envs 128 \
    --minibatch 4096 --threads 40 --steps 32 --gen-updates 40 --generations 400 \
    --patience 30 --eval-seeds 16 --out "$out" "${resume[@]}" >> "$log" 2>&1 && break
  sleep 10
done
echo "=== overnight done $(date)" >> "$log"
