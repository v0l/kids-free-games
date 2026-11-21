#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")"
old=${1:?usage: bench-compare.sh <old-binary> <new-binary>}
new=${2:?usage: bench-compare.sh <old-binary> <new-binary>}
gpu=${GPU:-1}
run() {
  label=$1
  shift
  echo "== $label"
  ~/.agents/bin/gpu-run "$gpu" -- "$@" --game clash --bf16 1 --pop 8 --envs 128 \
    --minibatch 4096 --threads 40 --steps 32 --gen-updates 20 --generations 3 --eval-seeds 16 \
    --min-gain -100 --out /tmp/kgrl/bench-compare.kgb 2>&1 | grep -E "^=== generation|sps [0-9]+|phases" | sed -E 's/^.*(sps [0-9]+).*/\1/'
}
run "old, 4 league servers" "$old" --league-servers 4
run "new, 4 league servers" "$new" --league-servers 4
run "new, 2 league servers" "$new" --league-servers 2
run "new, 1 lockstep league server" "$new" --league-servers 1
