# RL training

GPU training for bot policies. The candle trainer in `tools/rl/trainer` trains on the GPU; the game runs the exported net in Rust (`crates/core/src/brain/conv.rs`, format `KGB2`) with no extra dependency.

## Paper Land

Paper Land is trained with the candle trainer below, the same way as Snake Clash:

```sh
cd tools/rl/trainer && GAME=paper START=<warm start>.safetensors OUT=/tmp/kgrl/paper_pop.kgb \
  LOG=/tmp/kgrl/paper_pop.log EVAL_SEEDS=40 EXTRA="--secs 300 --explore 0.05" ./overnight.sh
```

Each episode is one last-one-standing game (`games/paper/rl.rs`): the learner is the player, the field is the real one for a skill band (`skill_of(seed)`, `arena::bots_for`), half the games it gets player speed and turning and half bot speed, and the rivals are hand-written bots, ES bots or league seats. The reward genes are land, kill, death, idle, linger, win and outlive. Evaluation plays 40 games over every skill band against the hand-written bots and the league; the score is wins and players outlived (both scaled down by time spent circling at home), peak land and kills, minus circling. Without the circling penalty the population learns to hide inside its own land and win by waiting.

A warm start from an older net saves hours: the first one came from a PyTorch PPO trainer (removed, see git history before this commit), and its `.pt` maps onto the candle names (`trunk.0/2/4` to `conv0/1/2`, `actor`/`critic` to `a1/a2`/`c1/c2`, dropping removed channels and zero-padding new scalars).

Copy the champion to `assets/brains/paper_rl.kgb`. In the game every bot runs it at `arena::rl_effort(skill)`, with blunders and the rule that leaves a beginner alone kept in code.

Two tests pin the shipped brain's behaviour and a survival-trained champion can miss them even when it outlives it by a mile: `at_full_effort_the_rl_brain_outlives_the_hand_written_bot` needs the brain to also steal at least as much land as the hand-written bot (`stolen >= 0.032` on 20 seeds), and `a_strong_player_with_big_loops_climbs_past_the_bots` needs a strong player to still reach the top three in 3 of 4 runs, so the bots must not be so good they box a good player out. Training that only maximises outlived tends to camp and to steal less, and fails both. Reward `steal` hard (the score already weights `stolen` at 800) and check the guard tests before promoting a champion. A 2026-11 run that resumed from such a survivor for two extra hours (steal gene raised) never cleared the steal bar; the champion that shipped came from the earlier plain run and was checked against both guards.

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

### Picking the champion

The ranking pass scores every member on one fixed seed set, so its winner is the luckiest of eight
on those games, not the best net. Promotion therefore re-plays the top `--holdout-top` members (three
by default) plus the defending champion on a fresh seed block (`set_eval_offset`, a new block each
generation) and only accepts a challenger that beats the champion by `--min-gain` there. `--holdout 0`
restores the old same-games comparison. Without the held-out check a run promotes on noise: at 16
games one score is worth about +-12 points, which is wider than any real gain between neighbouring
generations.

Evaluation threads are pooled (`set_eval_threads`), so `--eval-seeds 64` costs little more wall time
than 16. Prefer more games over more generations: the per-member score noise falls as 1/sqrt(n).

The printed `score` is only comparable inside one generation: the league the members play against
changes whenever a champion is promoted, so a round 20 score next to a round 4 score says nothing.

### Speed

The trainer is launch-bound, not compute-bound: at pop 8 and 128 envs the policy net is 94k weights
and every forward is 24 small kernels, so pop, envs, steps, threads, minibatch size and bf16 barely
move the step rate (20k-22k steps/s on a desktop RTX PRO 6000 either way). Measured dead ends:
candle's cuDNN conv path gives wrong logits here (parity error 0.98 against `conv.rs`) and is slower;
candle's native im2col conv also fails the parity check in f32 and is slower than the hand-written
`Unfold` gather. Bigger `--envs` needs GPU memory for the rollout buffers: 128 envs is about 28 GB
with 8 members, 512 runs out of memory on a 96 GB card when the display is using it.

Practical config on the desktop: `THREADS=40 POP=8 ENVS=128 MINIBATCH=16384 EVAL_SEEDS=64`,
`CUDA_VISIBLE_DEVICES` set to the free card, `nice`. A generation is about 40 s of training plus 15 s
of selection.

## Game state for training

A game exposes what a model sees through `kg_core::brain::state::BrainState`: named layers drawn
on a near view (centred on the agent, rotated so ahead is up, `Viewer::span` wide) and a
minimap of the whole world, plus named scalars. `look()` builds the `Obs`. Near marks are cut to the
circle any screen shows, so build marks only from what the player's UI shows. Clash builds its
minimap dots, edge arrows and camera span in `games/clash/sight.rs`, which the UI draws from too.
