# RL training

GPU training for bot policies. Python and PyTorch train; the game runs the exported net in Rust (`crates/core/src/brain/conv.rs`, format `KGB2`) with no extra dependency.

## Setup

```sh
python3 -m venv --system-site-packages ~/.venvs/kgrl
~/.venvs/kgrl/bin/pip install maturin
cd tools/rl/kgrl && VIRTUAL_ENV=~/.venvs/kgrl PATH=~/.venvs/kgrl/bin:$PATH maturin develop --release
```

`kgrl` is a pyo3 module outside the cargo workspace. It steps many copies of the Rust game on all cores and hands batched observations to Python as numpy arrays. Rebuild it after changing the game or `rl.rs`.

## Paper Land

```sh
~/.venvs/kgrl/bin/python tools/rl/train_paper.py --envs 256 --steps 32 --updates 4000 --out /tmp/kgrl/paper.kgb
```

PPO with self-play: every snake in each arena runs the learning policy. Every `--eval-every` updates it exports the net, checks that Rust and PyTorch give the same logits, and plays 16 games against the hand-written bots, printing peak land, deaths, kills, time on own land and circling on own land. The best snapshot (land minus deaths and circling) is saved to `--out`; `--resume <out>.pt` continues a run. Copy the result to `assets/brains/paper_rl.kgb` to ship it, then watch it with `kids_games --brain paper --model rl --speed 8`.

Throughput on the RTX PRO 6000 with 256 arenas is about 108,000 decisions a second on about 9 cores.

## Candle trainer (`tools/rl/trainer`)

Population-based PPO on the GPU, no Python:

```sh
cd tools/rl/trainer && CUDA_COMPUTE_CAP=120 cargo build --release
./target/release/kg-trainer --game clash --pop 6 --envs 128 --gen-updates 40 \
  --generations 150 --patience 12 --out /tmp/kgrl/clash_pop.kgb
```

Each generation every member trains `--gen-updates` PPO updates, then plays 16 games against the
hand-written bots and 16 against the league (the last `--league` champions). The worst quarter copy
a top member's weights and jiggle its reward weights, learning rate and entropy bonus. A new
champion goes to `<out>`, `<out>.safetensors` and `<out>.roundN`. `<out>.live` holds the leader and
`<out>.live.round` the generation; the watcher restarts on each new generation:

```sh
target/release/kids_games --brain clash --watch /tmp/kgrl/clash_pop.kgb.live --speed 2
```

## Game state for training

A game exposes what a model sees through `kg_core::brain::state::BrainState`: named layers drawn
on a near view (centred on the agent, rotated so ahead is up, `Viewer::span` wide) and a
minimap of the whole world, plus named scalars. `look()` builds the `Obs`. Near marks are cut to the
circle any screen shows, so build marks only from what the player's UI shows. Clash builds its
minimap dots, edge arrows and camera span in `games/clash/sight.rs`, which the UI draws from too.
