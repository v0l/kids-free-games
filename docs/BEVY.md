# Porting a game to Bevy

The `bevy` branch splits the project into two crates. Flappy Bird is the worked example: read `crates/core/src/games/flappy.rs` and `crates/app/src/games/flappy.rs` side by side before you start. The macroquad code from `main` is kept under `legacy/` as a reference only. It is not built.

## Layout

```
crates/core   kg_core: everything with no drawing. glam, quad-rand, serde_json, fontdue, lewton.
  engine.rs   View (safe area, 960x720 minimum), Input, InputTracker (mm, first finger, swipe lock), Game trait
  geom.rs     Rect, Color (sRGB, 0..1), WHITE/GOLD/..., hsv, circle_hits_rect
  fx.rs       easings, Particles, Shake, Popups, Bump, chip_rect (state only, no drawing)
  feel.rs     hit-stop and flash timing
  sfx.rs      Sfx, Say, Mood, the cue queue, pack decoding
  mixer.rs    the whole audio mixer: buses in dB, ducking, per-effect limit, voice takes, music moods
  save.rs profile.rs mastery.rs learn.rs net/ jev.rs font.rs outline.rs art.rs
  games/      the game logic, one file per game, with its tests
crates/app    kids_games: the Bevy app, desktop, web, Android and iOS
  lib.rs      the App, one frame system, input gathering, scenes (menu, shots, jev)
  paint.rs    Pen: the immediate-mode batcher, turned into Mesh2d layers each frame
  draw.rs     shapes, sprites, nine-slice and outlined text on Pen
  ui.rs       the painted UI kit: panels, buttons, ribbons, chips, hints, results board
  art.rs      embedded .kga sheets, page decode with mips (ahead of time on a thread)
  audio.rs    cpal output that runs kg_core::mixer::Mixer
  menu.rs profiles.rs link.rs shots.rs jev.rs platform.rs
  games/      one renderer per game, implementing games::Play
```

`cargo test` runs both crates headless. Nothing in a test opens a window or touches the GPU: the app tests run `update` and `draw` into a `Pen` and inspect the batches.

## How state and drawing split

A game is two files.

1. **Logic, `crates/core/src/games/<key>.rs`.** Move the game file from `legacy/src/games/` as it is. Keep the struct, `update`, the tuning, every test. Then:
   - Replace `use macroquad::prelude::*` with `use crate::geom::*; use glam::{vec2, Vec2}; use quad_rand as rand;`.
   - Delete `draw`, `draw_*`, `icon` and the `mod art { sprites!(..) }` block. Delete `fn draw` from the `Game` impl (the trait only has `update` now).
   - Make the fields the renderer needs `pub` (`t`, `fx`, `shake`, `popups`, `bump`, ...). If drawing needs a derived value, add a small `pub fn` for it (Flappy has `bird_frame`, `bird_angle`, `goal_progress`, `shows_results`) and test it there.
   - Text measuring is `crate::font::measure(s, size)`. Time is `crate::clock::now()`.
   - Add `pub mod <key>;` to `crates/core/src/games/mod.rs`. The tile already exists in `CATALOG`.
2. **Renderer, `crates/app/src/games/<key>.rs`.** A struct that owns the logic struct and implements `Play`:

```rust
pub struct FlappyPlay { pub g: Flappy, results_age: f32 }

impl Play for FlappyPlay {
    fn update(&mut self, input: &Input, view: &View, dt: f32) { self.g.update(input, view, dt); }
    fn draw(&self, pen: &mut Pen, view: &View) { draw(&self.g, pen, view, self.results_age); }
    fn score(&self) -> u32 { self.g.score }
    fn as_any(&mut self) -> &mut dyn std::any::Any { self }
}
```

Port the old `draw_*` functions here, one for one. The coordinate system is the same as `main`: view units, y down, `(0,0)` at the top-left of the safe area, `view.full()` for full-bleed backgrounds. So most draw code moves over by renaming calls:

| macroquad (`legacy/`) | Bevy branch (`pen: &mut Pen`) |
| --- | --- |
| `set_view_camera(view)` | nothing, the app does it |
| `draw_rectangle(x, y, w, h, c)` | `pen.rect(Rect::new(x, y, w, h), c)` |
| `draw_circle(x, y, r, c)` | `pen.circle(vec2(x, y), r, c)` |
| `draw_ellipse(..)` | `pen.ellipse(c, rx, ry, rot, col)` |
| `draw_line`, `draw_triangle` | `pen.line(a, b, w, c)`, `pen.tri(a, b, c, col)` |
| `round_rect`, `sky`, `fill_screen`, `soft_shadow`, `gloss` | same names on `pen` |
| `art::sprite(name, rect, params)` | `pen.sprite("<sheet>", name, rect, Draw { .. })` |
| `art::sprite_at(name, c, h, p)` | `pen.sprite_at("<sheet>", name, c, h, Draw { .. })` |
| `text_outline`, `text_center`, `text_shadow`, `big_score` | same names on `pen` |
| `ui::painted_panel/painted_button/ribbon/hint/chip/results` | same names on `pen` |
| `fx.draw()`, `popups.draw()` | `pen.particles(&g.fx)`, `pen.popups(&g.popups)` |
| `shake.offset()` applied to the camera | `pen.offset = g.shake.offset()` then reset to `Vec2::ZERO` before the HUD |

`Draw` carries `tint`, `rotation`, `flip_x`, `flip_y` and `pivot`. Draw order is call order; textures switch batches, so group sprites from one sheet together where you can.

Then register it in `crates/app/src/games/mod.rs`:

```rust
fn maker(key: &str) -> Option<fn() -> Box<dyn Play>> {
    match key {
        flappy::KEY => Some(|| Box::new(flappy::FlappyPlay::new())),
        hotel::KEY => Some(|| Box::new(hotel::HotelPlay::new())),
        _ => None,
    }
}
```

A tile with no maker shows greyed and says "Coming soon". Once it has one it opens like Flappy.

## 3D games (Happy Hotel first)

Real 3D goes beside the 2D layer, not through `Pen`. The paint camera is a `Camera2d` with `order: 10` that clears the screen. For a 3D game:

- Spawn a `Camera3d` with `order: 0` when the game opens and despawn it, with every mesh it owns, when it closes. Tag them with one marker component so `App::go_home` (via `Play`) can clean up.
- Set the paint camera's `clear_color` to `ClearColorConfig::None` while the 3D game is open, so the HUD and the painted UI kit draw over the 3D scene.
- Keep the simulation in `kg_core` (`new-hotel:src/games/hotel/sim.rs` is already free of drawing apart from `Rect`/`Vec2`, which map to `kg_core::geom` and `glam`). The renderer syncs `Sim` into entity transforms each frame; a `Play` impl that needs the ECS gets it by adding a Bevy system for its own components. Leave `Pen` for the HUD.
- Replace `new-hotel:src/three.rs` (a software projector) with Bevy meshes, `StandardMaterial` and one `DirectionalLight` with shadows. Compute-shader features are off on WebGL2: do not rely on SSAO, depth of field, atmosphere or OIT.
- Tune the camera with `View::full()` so portrait and landscape both frame the hotel, as `Cam3::for_view` did.

## Assets

- Art sheets are built by `scripts/assets.py art` from `art/*.txt` into `assets/art/*.kga`, as on `main`. Add the sheet to `SHEETS` in `crates/app/src/art.rs`; pages are embedded, decoded on a background thread at start, and uploaded with a mip chain on first use.
- A sprite name that is missing draws magenta. Add a test in the core game file listing the sprites the renderer uses (see `flappy_sprites_all_exist`).
- Sound is `assets/sound.kgs` from `scripts/assets.py sound`. Effects are decoded at start, music and voice lines one per frame after that.
- Text is Fredoka through `kg_core::outline`, the same outlined look as `main`. Sizes snap to `GLYPH_SIZES` so animated text never rasterises new glyphs.

## Audio and juice

The game never talks to the audio device. It queues and the mixer picks up the queue each frame:

- `sfx::play(Sfx::Pop)`, `sfx::play_step(Sfx::Score, n)` for a rising ladder, `sfx::say(Say::Great)` for a voice line.
- Music follows the menu's `Kind` mood. Large sounds and voice lines duck the music by `DUCK_DB`; the same effect twice inside `MIN_GAP` plays once; voice takes never repeat the last one.
- Juice is the same as `main`: `feel::impact(Impact::Large)` for hit-stop and a flash (the menu applies both), `fx.sparkle/puff/confetti`, `shake.add`, `popups.add`, `bump.kick`. Read the `game-feel` and `audio-design` skills and `docs/GAME-DESIGN.md` before changing any of it.

## Tests

- All logic tests stay in the core file and run with `cargo test -p kg_core`. Keep the no-repeat, rising-difficulty and scripted weak/strong player tests the design rules ask for.
- In the renderer file, test that each screen draws (look at `pen.used()` for the textures you expect) and that full-bleed layers cover `view.full()` with a cutout inset. See `crates/app/src/games/flappy.rs`.
- `games::tests::every_ported_game_survives_a_minute_of_random_input` picks the game up on its own once it is registered.

## Screenshot scenes

Add numbered scenes to `SCENES` and `setup` in `crates/app/src/shots.rs` (Flappy is 10-16; give each game its own block of ten). Each scene builds the state directly and returns a `Stage`. The scene test checks that each scene draws in both orientations.

```sh
scripts/shots.sh 1024 768                    # all scenes, landscape, /tmp/kgshots/sheet.png
OUT=/tmp/p scripts/shots.sh 768 1024         # portrait
ONLY="10 15" INSET=60 PORT=8800 scripts/shots.sh   # a few scenes with a fake cutout, own port
```

A scene number plus 1000 (`IDLE`) keeps the game running with no input until game over. Use your own `PORT` and `OUT` when several sessions run at once. Look at the sheet in both orientations every time.

## Running it

| Where | Command | Notes |
| --- | --- | --- |
| Desktop | `cargo run --release -p kids_games` | `--shot N`, `--jev flappy`, `--demo 1`; `KG_SAVE_DIR`, `KG_INSET` |
| Web | `scripts/web.sh`, then `python3 -m http.server -d dist` | `?shot=N`, `?inset=60`, `?jev=flappy&key=..` |
| Web deploy | `scripts/deploy-web.sh` | Cloudflare Pages branch `bevy`, https://bevy.kids-free-games.pages.dev |
| Android | `scripts/android.sh` | arm64 APK in `target/android-artifacts/`; `ABI=x86_64` for the emulator |
| iOS | on `ssh mac`: `scripts/ios.sh` | simulator by default; `TARGET=aarch64-apple-ios SIGN_IDENTITY=..` for a device |

Run windows only inside a private headless sway (see the user's desktop rules); `--demo 1` drives a scripted first launch and a Flappy run.

## Screenshot scenes for a game

Put a game's scenes in their own file, `crates/app/src/shots/<key>_scene.rs`. It holds `pub fn setup(n: u32, view: &View) -> Stage` returning `Stage::Game(Box::new(play))`, and a `#[cfg(test)] pub fn check()` that uses `super::game::<YourPlay>(setup(n, &v))` to reach the concrete game. The shared `shots.rs` then gains only these lines: `mod <key>_scene;`, your entries in `SCENES` (a slice, so no length to edit), one `NN..=MM => <key>_scene::setup(n, view),` arm, and one test calling `<key>_scene::check()`. Idle scenes (scene plus `IDLE`) step `Stage::Game` until `Play::over()` is true, so implement `over` for your game. Don't add `Stage` variants.

## Checklist for one game

1. Move the logic file to `crates/core/src/games/`, strip drawing, keep every test, add it to `games/mod.rs`. `cargo test -p kg_core` passes.
2. Write the renderer in `crates/app/src/games/`, port each `draw_*`, register it in `maker`.
3. Add its sheet to `SHEETS`, and a sprites-exist test.
4. Add renderer tests: each screen draws, backgrounds are full bleed.
5. Add screenshot scenes for every screen and state; check both orientations and an inset.
6. Play it on desktop or web from the menu: input, sound, hit-stop, results board.
7. `cargo test --workspace` and `cargo clippy --workspace --all-targets` are clean.
8. Commit on your branch with a short subject.
