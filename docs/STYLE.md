# Style guide

The rules every game and screen follows. The tests named here check them.

## Look

- Shapes are round and chunky. Corners are about 14% of a tile's width. Nothing has a sharp corner a child could read as dangerous unless it is (spikes, red danger segments).
- Each game owns one accent colour (`Entry::color` in `src/games/mod.rs`). Panels, the curtain and the launch wipe use it.
- Text is Fredoka SemiBold. Big numbers and titles use `text_outline` (white fill, `fx::INK` edge). Body text uses `text_center`.
- Light comes from the top: `gloss` on the upper third, a darker lip under tiles and buttons, `soft_shadow` under things that stand on the ground.

## HUD

- The top band is for the HOME button (top left), stats and nothing else.
- Stats use `fx::chip`: a dark pill with a small caps label and a big number, `Side::Left` for the live stat, `Side::Right` for BEST.
- Games where the score is the whole point (Flappy, Stack, Helix, Road Hopper) put it centre stage with `fx::big_score`.
- The score number kicks (`Bump::kick`) when it changes.

## Feel

Every event belongs to one tier. Use `feel::impact` for Medium and Large; Small is sound and particles only.

| Tier | Sound | Shake | Hit-stop | Flash | Examples |
|------|-------|-------|----------|-------|----------|
| Small | `Tier::Small`/`Ui` | 0-0.2 | none | none | flap, jump, tap, tick |
| Medium | `Tier::Medium` | 0.2-0.5 | 50 ms | none | combo, row clear, big merge, eat a rival |
| Large | `Tier::Large` | 0.6-1.0 | 120 ms | 0.35, at most 3 a second | death, bomb, level up, round won |

- Hit-stop runs on real time and the newest stop never extends an older one (`feel::tests`).
- Streaks climb a major scale with `sfx::play_step`: breakout combos, perfect stacks, merges, matched pairs, spelled letters, correct answers in a row.

## Sound

- Sounds are synthesised in `src/sfx.rs` and normalised by tier: Ui -24, Small -21, Medium -18, Large -15 dB (loudest 20 ms window), peaks under -1 dBFS (`tiers_land_at_their_loudness_with_headroom`).
- Each sound has three takes at 0.95, 1.0 and 1.05 pitch so repeats do not sound robotic.
- Music is three four-bar loops built from chords in code: Menu (108 bpm), Arcade (120 bpm, with hats), Learn (92 bpm, softer and lower). It sits at least 6 dB under the quietest game sound (`music_sits_well_under_effects`) and ducks 9 dB for 0.5 s under any Large sound.
- The menu has separate toggles for music (top left) and sound (top right).

## Menu

- Games are split into Play and Learn sections with a label each.
- Arrow keys move a focus ring, Enter or Space launches, Escape or Backspace goes home (`keyboard_alone_can_launch_a_game`).
