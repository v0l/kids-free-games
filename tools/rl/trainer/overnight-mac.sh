#!/usr/bin/env bash
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
[ -x target/release/kg-trainer ] || cargo build --release --no-default-features --features metal || exit 1
export DEVICE=${DEVICE:-metal}
export BF16=${BF16:-0}
export POP=${POP:-4}
export ENVS=${ENVS:-32}
export THREADS=${THREADS:-6}
export SERVERS=${SERVERS:-1}
export MINIBATCH=${MINIBATCH:-2048}
export EVAL_SEEDS=${EVAL_SEEDS:-8}
exec nice -n 10 ./overnight.sh
