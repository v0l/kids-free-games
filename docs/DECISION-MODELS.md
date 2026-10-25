# Decision-model players (research notes)

Can a decision model (a "System One" model such as TypeSafe's Jev) play our games? Measured 2026-10-25 with `tools/play/bench.py`.

## The bench

`bench.py` generates game moments from the real rules (Flappy physics: gravity 1500, flap -520; Road Hopper: car positions, speed and direction over the hop time) where the right action is known, balanced half "act" and half "wait" so always giving one answer scores 50%. Each moment is shown two ways:

- **raw**: the numbers (`bird_y`, `bird_vy`, `gap_top`, car x positions, speed).
- **described**: the outcome of each action worked out in code (`if_flap_now: passes the gap through the middle`, `if_wait: hits the bottom pipe`).

```
N=80 python3 tools/play/bench.py <name> <url> [model]
```

`url` is a `/v1/systemone` endpoint (Jev and Laya format, `{state, questions}`) or a `/v1/decide` endpoint (AutoTrust format, one question per call). `KEY` is sent as a Bearer token.

## Results (80 moments per game)

| Model | Where | Flappy described | Road Hopper described | Raw numbers | Latency | Cost |
|---|---|---|---|---|---|---|
| TypeSafe Jev 1.13 | Yalr `/v1/systemone`, model `openrouter/typesafe/jev-1.13` (Yalr key in `~/.pi/agent/models.json`) | 100% | 100% | 50-85% | 330-450 ms | about $0.000015 per call |
| AutoTrust JEV-9B (open, unofficial) | spark, vLLM | 100% | 50% (always hops) | about 50% | 155 ms | free |
| Laya 0.4B (open) | local GPU, `pip install "laya[serve]"`, `laya-serve` | about 50% | about 50% | about 50% | 8 ms | free |

## What we learned

- **Do the physics in code.** Every model is near coin-flip on raw positions and speeds. Turn each moment into described outcomes per action, as the DOOM harness does (System1Doom, `github.com/PanikParty/System1Doom`) and as TypeSafe's guide says: bearings, distances and named buckets, never raw coordinates.
- **State the goal, not just safety.** Jev 1.13 fell to 50% on Road Hopper when told "if you wait, the chicken stays safe": waiting always looked safest. "Cross as many rows as possible; only wait if hopping now would get you hit" brought it back to 100%.
- **Option order matters for JEV-9B.** Swapping the order of the options flipped its answers, and it hopped into cars even when told it would be hit.
- **Latency.** At 0.3-0.45 s per call, a real-time game must pause while the model decides (step the game headless) or hold the last action between answers (the DOOM approach at about 10 Hz).

## Running the open models again

- **JEV-9B** on spark: `~/.venv-jev` has a current vLLM (the model's server needs `vllm.entrypoints.launchers`, newer than the other spark venvs). Start it with `source ~/.venv-jev/bin/activate; cd ~/models && PORT=8190 MAX_MODEL_LEN=8192 EXTRA_ARGS="--gpu-memory-utilization 0.3 --host 127.0.0.1" bash JEV-9B/vl/serve.sh`, then tunnel with `ssh -L 8190:127.0.0.1:8190 spark`.
- **Laya**: `python3 -m venv /tmp/laya-env && /tmp/laya-env/bin/pip install "laya[serve]"`, then `LAYA_DEVICE=cuda LAYA_PRELOAD=1 LAYA_PORT=8188 /tmp/laya-env/bin/laya-serve`.
- Other open checkpoints on Hugging Face: `autotrust/JEV-27B`, `autotrust/JEV-35B(-NVFP4)`, `autotrust/JEV-27B-VL`, `alibiserikbay/JevK5`. Not yet benched.

## The harness to build (not started)

Step the real game code headless frame by frame. At each decision point turn the state into described outcomes per action, ask Jev 1.13 in one batched call while the game waits, and apply the answer. Run a perfect code bot (simulates ahead) alongside as the reference that proves every stage can be beaten, and a "kid" bot (slow, makes mistakes) to check onboarding. Report per game: how far each player gets on attempts 1, 2, 3 and on (saves carry over), what ended each run, and calls and cost. Real Jev runs only from a separate command; `cargo test` uses a stub.
