# Lane Runner asset pipeline

Every visual in Lane Runner is made in-house. Nothing is Kenney or stock.

| Asset | Source | Files |
|---|---|---|
| Sky backdrop | FLUX.1-dev sprite | `art/gen/prompts/runner.json`, `art/runner.txt` |
| Hero, motion clips, tree | gcad mesh text, built at startup by gcad-mesh | `assets/models/hero.gcad`, `assets/models/motion.gcad`, `assets/models/prop_tree.gcad` |
| Barrier, hurdle, gate, train, coin, finish arch, road, buildings | gcad solid parts, exported to GLB | `art/3d/props/*.gcad`, `art/3d/props/build.sh`, `assets/models/prop_*.glb` |

## Models built at startup

The hero and the tree ship as the `.gcad` text itself (`assets/models/*.gcad`, copied by `scripts/web.sh`, `scripts/android.sh` and `scripts/ios.sh` like the GLBs). `crates/app/src/gcad.rs` is a Bevy asset loader for `.gcad`: it splices `include` lines through the asset reader (so it works from the Android APK and over HTTP on the web), builds the model with `gcad_mesh::build_with` (gcad pinned by git rev in `crates/app/Cargo.toml`), and turns the result into a scene: one mesh and material per part with vertex colours, a joint entity per bone, a `SkinnedMesh`, and an `AnimationClip` per clip as `models/hero.gcad#Animation/<name>`. The model is scaled to 1 unit high with its feet at 0 (the runner then scales it to `HERO_H`); unrigged models are centred on x and z.

`GcadPlugin` warms both models on a background task at app start, and the loader runs on Bevy's asset threads, so nothing builds on the main thread. A process-wide cache keyed by the text and detail level builds each model once even when the scene and its six clips are requested separately. The runner stays on "Getting ready..." and ignores taps until the hero has loaded (`RunnerPlay::loaded`).

Build time per model (release, best of three; the hero includes its clips):

| | hero | tree | total |
|---|---|---|---|
| Desktop, 48 threads (`cargo run --release -p kids_games --example gcad_timing`) | 384 ms | 97 ms | 480 ms |
| Desktop, one core (`taskset -c 0`) | 773 ms | 279 ms | 1052 ms |
| Android emulator `kidsgames-test` (x86_64, 4 cores, swiftshader), three cold starts | 463-1095 ms | 138-475 ms | 0.6-1.6 s |
| Web, headless Chrome (wasm32, single thread) | 829 ms | 272 ms | 1.1 s |

All of it is under the 2 s budget and off the main thread, so there is no on-disk cache. `GcadSettings::detail` (2 halves the grid) is there for slow devices: one core drops from 1052 ms to 488 ms at detail 2.

Sizes: the three `.gcad` files are 132315 B (26582 B gzipped); the GLBs they replace were 722524 B (`hero.glb` 645104 B, `prop_tree.glb` 77420 B).

Tests in `crates/app/src/games/runner/world.rs` build every `.gcad` and check the rig (15 bones), the six clip names, triangle budgets, height and feet, and that a clip moves the rig. `a_tap_does_not_start_the_run_before_the_hero_is_built` covers the loading gate. `docs/hero/glb_vs_gcad.png` compares the old GLB (top) with the gcad build (bottom) for every clip from the game camera: 99.8% of pixels within 16 levels, mean difference 0.1.

The eight solid-part props still ship as GLBs: gcad-mesh runs only the mesh subset (gcad issue #271 asks for flat solid profiles). `art/3d/props/build.sh` exports them with `gcad export` and `art/3d/scale_glb.py` scales them.

## Hero motion

1. Kimodo (NVIDIA, model under the NVIDIA Open Model License, outputs unrestricted) renders takes from text prompts on spark: repo `/tmp/kimodo`, venv `/tmp/kmv`; `art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/`. The chosen takes are kept in `art/3d/motion/bvh`.
2. `python3 art/3d/hero/retarget.py` retargets them onto the rig in `assets/models/hero.gcad` and writes `assets/models/motion.gcad`. `art/3d/hero/clips.json` picks one take per clip: `trim` frame range, `loop`, `amp` per bone, `arm_clear` degrees, `lift` plant or ground, `upright: false` to keep the take's own spine lean (otherwise spine and head lean is scaled down so the chibi does not hunch).
3. Keys are reduced per bone: a key is dropped when slerping its neighbours stays within `ANGLE_TOLERANCE` (0.4 degrees) of every frame between them, and the hip lift within `LIFT_TOLERANCE` (0.3 cm); numbers are rounded to 0.1 degree and 1 ms. That took motion.gcad from 271780 B (4224 keys) to 126538 B (2798 keys). `cargo run --release -p kids_games --example motion_error -- full.gcad assets/models/motion.gcad` skins both at 120 Hz: worst vertex error 0.67 cm on the 134 cm hero.

## Seeing a model

`cargo run --release -p kids_games --example model_sheet -- models/hero.gcad --clips run,jump --views game,side --out /tmp/sheet.png`

Renders a `.gcad` or `.glb` through the game's own loader and animation path, headless, into one PNG: a row per clip and view, a column per frame. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. `gcad render` and `gcad inspect` show the source with grids and part names; `gcad view` opens the live viewer.

## History

Before November 2026 the hero and props were Hunyuan3D-2 meshes (`art/3d/hero/build.py`, `art/3d/props.py`, `art/3d/prop_build.py`, kept for reference). See `docs/LICENSES.md` for why none of that output ships any more.
