# RL training

GPU training for bot policies. The candle trainer in `tools/rl/trainer` trains on the GPU; the game runs the exported net in Rust (`crates/core/src/brain/conv.rs`, format `KGB2`) with no extra dependency.

## Paper Land

Paper Land is trained with the candle trainer below, the same way as Snake Clash:

```sh
cd tools/rl/trainer && GAME=paper START=<warm start>.safetensors OUT=/tmp/kgrl/paper_pop.kgb \
  LOG=/tmp/kgrl/paper_pop.log EVAL_SEEDS=40 EXTRA="--secs 300 --explore 0.05" ./overnight.sh
```

Each episode is one last-snake-standing game (`games/paper/rl.rs`): the learner is the player, the field is the real one for a skill band (`skill_of(seed)`, `arena::bots_for`), half the games it gets player speed and turning and half bot speed, and the rivals are hand-written bots, ES bots or league seats. The reward genes are land, kill, death, idle, linger, win and outlive. Evaluation plays 40 games over every skill band against the hand-written bots and the league; the score is wins and snakes outlived (both scaled down by time spent circling at home), peak land and kills, minus circling. Without the circling penalty the population learns to hide inside its own land and win by waiting.

A warm start from an older net saves hours: the first one came from a PyTorch PPO trainer (removed, see git history before this commit), and its `.pt` maps onto the candle names (`trunk.0/2/4` to `conv0/1/2`, `actor`/`critic` to `a1/a2`/`c1/c2`, dropping removed channels and zero-padding new scalars).

Copy the champion to `assets/brains/paper_rl.kgb`. In the game every bot runs it at `arena::rl_effort(skill)`, with blunders and the rule that leaves a beginner alone kept in code.

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
