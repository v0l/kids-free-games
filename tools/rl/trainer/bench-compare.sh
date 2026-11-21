#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")"
old=${1:?usage: bench-compare.sh <old-binary> <new-binary>}
new=${2:?usage: bench-compare.sh <old-binary> <new-binary>}
gpu=${GPU:-1}
for bin in "$old" "$new"; do
  echo "== $bin"
  ~/.agents/bin/gpu-run "$gpu" -- "$bin" --game clash --bf16 1 --league-servers 4 --pop 8 --envs 128 \
    --minibatch 4096 --threads 40 --steps 32 --gen-updates 20 --generations 3 --eval-seeds 16 \
    --min-gain -100 --out /tmp/kgrl/bench-compare.kgb 2>&1 | grep -oE "generation [0-9]+|sps [0-9]+"
done
