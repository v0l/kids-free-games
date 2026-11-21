#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")"
old=${1:?usage: bench-compare.sh <old-binary> <new-binary>}
new=${2:?usage: bench-compare.sh <old-binary> <new-binary>}
gpu=${GPU:-1}
mkdir -p /tmp/kgrl/b951
run() {
  label=$1
  shift
  echo "== $label"
  ~/.agents/bin/gpu-run "$gpu" -- "$@" --game clash --bf16 1 --pop 8 --envs 128 \
    --minibatch 4096 --threads 40 --steps 32 --gen-updates 20 --generations 3 --eval-seeds 16 \
    --min-gain -100 --out /tmp/kgrl/bench-compare.kgb > "/tmp/kgrl/b951/$label.log" 2>&1
  grep -E "^=== generation|sps [0-9]+|phases" "/tmp/kgrl/b951/$label.log" | sed -E 's/^(round [0-9]+ upd +[0-9]+).*(sps [0-9]+).*/\1 \2/'
}
run old-4srv "$old" --league-servers 4
run new-server "$new" --league-servers 4 --league-mode server
run new-combined "$new" --league-mode combined
