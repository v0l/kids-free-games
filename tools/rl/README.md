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
