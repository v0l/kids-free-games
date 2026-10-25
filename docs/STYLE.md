# Style guide

The rules every game and screen follows. The tests named here check them. How the art is made is in `docs/DESIGN.md`.

## Look

- Painted 3D-cartoon art in the style of Supercell, King and Rovio: glossy, saturated, soft light from the top. All of it comes from the FLUX pipeline in `docs/DESIGN.md`; no flat clipart and no shapes drawn in code where a painted sprite can go.
- Shapes are round and chunky. Nothing has a sharp corner a child could read as dangerous unless it is (spikes, red danger segments).
- Each game owns one accent colour (`Entry::color` in `src/games/mod.rs`) used by the curtain and launch wipe.
- No text inside images. Text is Fredoka SemiBold drawn in code: titles and numbers with `text_outline` (white fill, dark plum edge), body text with `text_center`.
- Menu icons are painted squares with 22% rounded corners and the game's name under them.
- Dialogs use the painted kit in `src/ui.rs`: a lilac panel, a red ribbon title and a green jelly button for the action.

## HUD

- The top band is for the HOME button (top left), stats and nothing else.
- Stats use `fx::chip`: a dark translucent pill with a small caps label and a gold number, `Side::Left` for the live stat, `Side::Right` for BEST.
- The HOME, music and sound buttons are `ui::painted_round` gold buttons.
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

- Effects are Kenney CC0 recordings listed in `sound.txt`, normalised by tier: Ui -24, Small -21, Medium -18, Large -15 dB (loudest 20 ms window), peaks under -1 dBFS (`tiers_land_at_their_loudness_with_headroom`).
- Each effect has two or three takes, picked at random, so repeats do not sound robotic.
- Big moments add a voice line with `sfx::say`: `Go` when a round starts, `Correct`/`Great` for right answers, `Best` on a new best, numbers in Count It. Game over plays `Sfx::Lose`.
- Music is three CC0 loops (menu, arcade, learn) that sit at least 6 dB under the quietest effect (`music_loops_are_long_and_sit_under_the_effects`) and duck 9 dB for 0.5 s under any Large sound or voice line.
- The menu has separate toggles for music (top left) and sound (top right).

## Menu

- Games are split into Play and Learn sections with a label each.
- Arrow keys move a focus ring, Enter or Space launches, Escape or Backspace goes home (`keyboard_alone_can_launch_a_game`).
