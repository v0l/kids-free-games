# Design process

How the look and sound of Kids Free Games were made, and how to make more in the same way. `docs/STYLE.md` holds the rules; this file holds the method.

## What we tried, in order

1. **Shapes drawn in code.** Circles, rounded rectangles and gradients from `engine.rs`. Fast to iterate, but the user called it "real basic".
2. **Juice pass.** Easing, squash and stretch, particles, screen shake, hit-stop, a shared HUD chip and a menu with Play and Learn sections (`src/feel.rs`, `fx::chip`). Better feel, still basic to look at.
3. **Kenney CC0 packs.** Consistent flat clipart from kenney.nl for every game, plus recorded effects. The user said it looked worse: flat stock art reads as cheap next to App Store games.
4. **Painted art generated with FLUX.1-dev.** Glossy 3D-cartoon art in the style of Supercell, King and Rovio. The user approved it on one game (Flappy) and the menu before the other 15 were done.

What to take from this: show one finished screen in the target style and get a yes before rolling it out. A style the user names ("AAA, like the App Store games") is worth more than a style that is easy to get.

## The pipeline

```
art/gen/prompts/<game>.json --make.sh--> spark: FLUX.1-dev in ComfyUI --> rembg BiRefNet cut
        --> art/gen/out/*.png --art/<game>.txt + scripts/assets.py--> assets/art/<game>.kga (WebP pages)
        --> crate::sprites!("<game>") in the game's code
```

- **Generation** runs on `spark` (DGX Spark, GB10). Start ComfyUI with `cd ~/git/ComfyUI && .venv/bin/python main.py --listen 127.0.0.1 --port 8188`. The model is `flux1-dev.safetensors` in fp8 with the T5 fp8 and CLIP-L encoders and `flux_ae.safetensors`; `art/gen/flux.py` holds the workflow (euler, simple schedule, 22 steps, guidance 3.5). One image takes 40-70 s, and the GPU is a single queue, so parallel agents wait on each other.
- **Background removal** uses rembg with the `birefnet-general` model in `/tmp/rmenv` on spark (recreate it with `python3 -m venv /tmp/rmenv && /tmp/rmenv/bin/pip install "rembg[gpu]"`). `make.sh` cuts every job marked `"cut": true` and crops it to the object.
- **Packing**: `python3 scripts/assets.py art` reads each `art/<sheet>.txt`, packs the sprites into atlas pages and writes `assets/art/<sheet>.kga`. Pages are lossy WebP (quality 88, alpha 95), which shrank the art from about 15 MB as PNG to 2.6 MB. Solo backgrounds are resized to power-of-two sizes so WebGL1 can mipmap them; the index keeps their real aspect.
- **Runtime**: `src/art.rs` decodes pages with the `image` crate on first use and uploads mipmapped textures. `art::upload` is the one way to turn page bytes into a texture.

## Writing prompts

Every prompt ends with the shared style line:

> 3D rendered cartoon style like Supercell, King or Rovio mobile games, vibrant saturated colours, soft studio lighting, glossy, polished AAA mobile game art, no text, no letters

Then by kind of asset:

| Asset | Add to the prompt | Job settings |
|-------|-------------------|--------------|
| Character, object, UI piece | `single object centred and fully visible, isolated on a plain flat pure white background, no shadow, no ground` | `"cut": true`, 768-1024 square |
| Full background | a scene description, `no characters, no text`, an empty centre if UI goes over it | 1344x768 landscape, 768x1344 portrait |
| Parallax layer | `top half of the image is plain flat pure white`, `no buildings` if it must tile | `"cut": true`, 1344x512 |
| Animation frames | the same seed and wording; change only the pose words | Flappy's `bird_up` and `bird_down` share seed 41 |
| App icon | `square mobile game app icon illustration filling the whole frame`, `no border, no frame` | 768 square |

Rules learned on the way:

- **No text in images.** FLUX writes garbled letters. Draw every word and number in code with `text_outline`.
- **Look at every image before using it.** Regenerate on wrong anatomy, stray text, a white border baked into an icon, or a style that does not match its neighbours. Rerun with a new seed rather than fighting a bad one with edits.
- **Batch a game's jobs in one file.** The GPU queue is shared, so one file per game keeps a run predictable.
- **Keep the prompts json that produced the shipped files.** A hand-run re-roll that is not saved cannot be reproduced.

## Making generated art work in a game

- **Tiling strips** (sky, hills, ground): mirror the strip with `art/gen/mirror.py` so the right edge meets its own reflection. Crossfading the edges (`art/gen/seam.py`) left ghosted houses; mirroring has no seam.
- **Stretchy pieces**: a pipe body is one averaged row repeated (no highlight smear), with the painted cap on the end. Buttons and panels are 9-sliced with `nine()` so one painted button fits any label.
- **Hitboxes follow the art.** When a sprite's shape changes, change the collision to match and add a test (Flappy's pipes, the Breakout paddle, Road Hopper's cars, Sky Jumper's springs).
- **Menu icons** get rounded corners baked in (22% radius) so they read as App Store icons.
- **Shared UI kit** (`src/ui.rs`): `painted_panel`, `painted_button`, `painted_round`, `ribbon`, `hint`, `icon`, plus `p_coin`, `p_star` and `p_medal`. Every game's game-over screen goes through `overlay()` and so uses the painted panel.

## Sound

- **Effects** are Kenney CC0 recordings mapped in `sound.txt`, two or three takes per sound, normalised by tier (`docs/STYLE.md`) by `scripts/assets.py sound` into `assets/sound.kgs`. Streak sounds get an eight-step ladder rendered offline.
- **Voice lines** ("Go!", "Correct!", the numbers 1-10) are from Kenney's CC0 voiceover pack.
- **Music** is three CC0 loops from OpenGameArt (URLs in `sound.txt`), picked by tempo, brightness and whether they end where they start, then normalised to sit at least 6 dB under the effects and duck under big moments.
- Everything is Ogg Vorbis decoded with `lewton` in Rust, so Safari's patchy WebAudio Ogg support does not matter.

## Working in parallel

The 15 repaints ran as five agent sessions, three games each, in separate worktrees. What made that work:

- One finished reference game and a written brief (`art/README.md`, this file) before starting.
- A strict file boundary: each session touches only its games' `.rs`, manifest, `.kga` and prompts. Shared files (`ui.rs`, `fx.rs`, `art.rs`, `scripts/`) stay with the coordinator, so merges never conflict.
- A screenshot port per session (`PORT=` for `scripts/shots.sh`).
- After merging: rebuild every sheet with `scripts/assets.py art`, then review every game in both orientations (`scripts/shots.sh 1024 768` and `768 1024`; game index plus 100 runs it idle until game over).

## Checking the result

- `cargo test --lib`: every manifest's sprites exist, every page decodes, every sound decodes and sits at its tier's loudness.
- `scripts/shots.sh` in both orientations, looked at by a person (or the agent) every time.
- On the Pixel and the Android emulator: frame time logged from `run()` while playing.
