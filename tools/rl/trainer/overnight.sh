#!/usr/bin/env bash
set -u
cd "$(dirname "$0")"
game=${GAME:-clash}
out=${OUT:-/tmp/kgrl/${game}_kill.kgb}
log=${LOG:-/tmp/kgrl/${game}_kill.log}
start=${START:-}
for attempt in 1 2 3 4 5; do
  resume=()
  [ -n "$start" ] && resume=(--resume "$start")
  [ -f "$out.safetensors" ] && resume=(--resume "$out.safetensors")
  echo "=== overnight attempt $attempt $(date) ${resume[*]}" >> "$log"
  ./target/release/kg-trainer --game "$game" --bf16 1 --league-servers 4 --pop 8 --envs 128 \
    --minibatch 4096 --threads 40 --steps 32 --gen-updates 40 --generations 400 \
    --patience 30 --eval-seeds ${EVAL_SEEDS:-16} ${EXTRA:-} --out "$out" "${resume[@]}" >> "$log" 2>&1 && break
  sleep 10
done
echo "=== overnight done $(date)" >> "$log"
