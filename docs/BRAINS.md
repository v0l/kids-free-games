# Trained bot brains

Bots in Snake Clash, Paper Land and Happy Hotel (the hired staff) are driven by small neural nets trained against the real game code. The runtime and trainer live in `crates/core/src/brain/`, are plain Rust with no extra dependencies, and run on every platform the app ships on.

## How a brain decides

A brain never outputs a raw action. At each decision the game lists its **options** (headings to steer, rooms to clean, guests to serve) and describes each one as a fixed-length feature vector, the same length every time. The net gives each option one score and the brain picks:

```rust
let mut brain = Brain::load(include_bytes!("../../../../assets/brains/clash.kgb"), Effort::new(0.6)).unwrap();
let options: Vec<Vec<f32>> = headings.iter().map(|h| features(world, me, *h)).collect();
let k = brain.choose(&options);
```

Option scoring keeps nets tiny (a few hundred weights), lets the option count change between decisions, and works the same for steering and for task picking.

Features are what the bot could see from its seat, scaled to roughly -1..1: distances divided by a sensible reach, levels as differences, booleans as 0/1. Never feed absolute world coordinates.

## Effort

`Effort` runs from 0.0 (a lazy beginner) to 1.0 (the trained best). The same net plays every difficulty:

- `temperature`: 0 at full effort (always the top score), softer picks as effort drops.
- `stubborn`: chance of keeping the last choice instead of reconsidering.
- `think_every(fastest)`: seconds between decisions; low effort reacts up to 4 times slower. Games call `brain.choose` on that cadence.

Lobby difficulty later maps straight onto effort. Each game must show, in a test, that fitness rises with effort (the trainer's calibration report gives the numbers to pin).

## Training

Implement `brain::train::Env` for the game: one `episode` plays a full run from a seed, calls `policy(&options)` at each decision, and returns a fitness (higher is better). Rewards come from the game's own goals: land share, level reached, guests served and money earned, minus deaths.

Add an example `crates/core/examples/train_<game>.rs`:

```rust
use kg_core::brain::train::{self, Config};
fn main() {
    let report = train::train(|| MyEnv::new(), FEATURES, &Config::from_env());
    train::save(&report, "assets/brains/<game>.kgb");
}
```

Run it with `cargo run --release -p kg_core --example train_<game>`. Tune with env vars: `GENS`, `POP`, `SEEDS`, `SIGMA`, `RATE`, `HIDDEN=16,16`, `THREADS`, `SEED`. Training uses every core and logs mean and best fitness per generation, then prints fitness at efforts 0, 0.25, 0.5, 0.75 and 1.

The trainer is OpenAI-style evolution strategies with mirrored sampling, rank shaping and Adam. Episodes must be deterministic for a seed (`crate::rng::srand` is called before each one), so keep all randomness on `crate::rng`.

## Shipping a brain

- Commit the trained `assets/brains/<game>.kgb` (`KGB1` magic, layer sizes, little-endian f32 weights) and load it with `include_bytes!`.
- Keep the hand-written bot as the fallback when loading fails, and delete nothing the tests rely on.
- Tests per game: the trained brain beats the old hand-written bot at full effort; fitness rises with effort; a low-effort brain still plays legally (no stuck bots); the existing weak and strong player tests still pass.
- Training is not run by `cargo test`. Tests load the committed `.kgb`.
