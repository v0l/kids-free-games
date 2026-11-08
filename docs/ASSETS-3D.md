# Lane Runner asset pipeline

Every visual in Lane Runner is made in-house. Nothing is Kenney or stock.

| Asset | Source | Files |
|---|---|---|
| Sky backdrop | FLUX.1-dev sprite | `art/gen/prompts/runner.json`, `art/runner.txt` |
| Hero, motion clips, tree, barrier, hurdle, gate, train, coin, finish arch, road, buildings | gcad text, built at startup by gcad-bevy | `assets/models/*.gcad` |

## Models built at startup

Every model ships as the `.gcad` text itself (`assets/models/*.gcad`, copied by `scripts/web.sh`, `scripts/android.sh` and `scripts/ios.sh`). There is no export step and no `.glb` in the game: edit a `.gcad` and run the game. The loader is the `gcad-bevy` crate from [v0l/gcad-engines](https://github.com/v0l/gcad-engines) (`bevy/`, docs in its `docs/bevy.md`), pinned by git rev in `crates/app/Cargo.toml`; it is public, so CI and release builds fetch it with no credentials. It uses gcad 5a5fde3, the only gcad in `Cargo.lock`. `GcadPlugin` registers the `.gcad` asset loader: it splices `include` lines through the asset reader (so it works from the Android APK and over HTTP on the web), builds the model with gcad's `gcad2glb` library (mesh-only, `parallel` on everything but wasm32) on Bevy's async compute pool, and turns it into a scene: one mesh and material per part with vertex colours, a joint entity per bone, a `SkinnedMesh`, and an `AnimationClip` per clip as `models/hero.gcad#Animation/<name>`. The game depends on it with `default-features = false`, which leaves Bevy's `multi_threaded` off as before.

`crates/app/src/gcad.rs` holds only what is the game's own: `settings(scale)`, which is `GcadSettings::fit_height(1.0)` (1 unit high, feet at 0, unrigged models centred on x and z; the runner then scales each model to the height in `prop(c, models, name, h, at)`) or a fixed `scale` instead (0.01 for the road and buildings, which the runner places in metres from their millimetre sources), and `ModelsPlugin`, the startup warm-up. The hero clips load with `load_gcad_clip` and the same settings as the scene, so they share one build. The prop heights and the hero clip names stay in `crates/app/src/games/runner/world.rs`.

The five buildings are `prop_building1.gcad` to `prop_building5.gcad`, two lines each: `let style= floors=` and `include building_base.gcad`.

`ModelsPlugin` starts loading every model the runner uses (`Models::load`: hero, six clips, 13 props) at app start and logs `model <name> ready N ms after start` as each one lands; the runner reuses those handles. The game runs Bevy without `multi_threaded`, so Bevy's task pools, and the builds on them, run on the main thread during the first frames, the same as with the old loader. gcad-bevy keeps each build in memory, and on desktop also on disk (`~/.cache/gcad/bevy`, keyed on gcad version, text, includes and settings), so the second start needs no build at all. The runner stays on "Getting ready..." and ignores taps until the hero has loaded (`RunnerPlay::loaded`).

## The shipped bake

`scripts/bake.sh` runs `cargo run --release -p kids_games --example bake` (gcad-bevy's `bake::bake` with `kids_games::gcad::bake_settings`, the same `settings(scale)` the runner loads with) and then `--check`. It writes `assets/gcad-baked/`: one `.bin` per model and an index, 14 files and 1.33 MB (389 KB gzipped), in 0.6 s. `scripts/android.sh`, `scripts/ios.sh`, `scripts/web.sh`, `scripts/macos-app.sh` and the desktop job of the release workflow run it and the three app scripts copy `assets/gcad-baked/` next to `assets/models/`. The directory is generated, not committed (`.gitignore`): it changes on every gcad bump and rebuilding it costs less than a second.

Tests in `world.rs` keep the bake honest: `the_bake_key_is_the_key_models_load_asks_for` checks that `bake::key_of` equals the cache key of every `Models::load` asset, and `a_fresh_bake_of_the_models_passes_the_check` bakes a copy of the models, checks it passes and that editing a model makes `bake::check` report it. `bake_settings` is built from `PROPS`, so a new prop is baked and checked without another edit.

Time until every model is ready (release, `ModelsPlugin`):

| | no bake, empty cache | no bake, warm disk cache | shipped bake, first start |
|---|---|---|---|
| Desktop, private headless sway | 672-730 ms, 14 builds | 103-113 ms | 108-112 ms, 0 builds |
| Android emulator `kidsgames-test` x86_64 | 1337 ms, 14 builds | 558-675 ms | 496-798 ms, 0 builds |
| iOS simulator, iPad Pro 11-inch (M4) | 942 ms, 14 builds | 110-112 ms | 106-115 ms, 0 builds |
| Web, headless Chrome | 2765-3044 ms, 14 builds | no disk cache | 2081-2677 ms, 0 builds |

On the web the models wait for the 13 MB of sound files that load first; loading them after the models would cut that number further.

Sizes: the sixteen `.gcad` files are 140549 B (29069 B gzipped), of which the props are 9313 B. The twelve prop GLBs they replace were 1100352 B; before that the hero and tree GLBs were 722524 B.

| | before (GLB props) | after |
|---|---|---|
| Web bundle (`scripts/web.sh`, small) | 51296727 B | 49314507 B |
| Web wasm | 26603801 B | 25713699 B (no `bevy_gltf`) |
| Android arm64 APK | 36077613 B | 35640051 B |
| iOS simulator app | 42268 KiB | 40752 KiB |

With gcad-bevy (before is the `gcad2glb` loader at c43c004, same machine and profile): web bundle 49540006 B (was 49317630), wasm 25939198 B (was 25716822), arm64 APK 35774367 B (was 35643863), iOS simulator app 40864 KiB (was 40704). The difference is gcad-bevy's own surface: the build cache with its bitcode format, morph targets, LODs and normal maps.

## Tests

Tests in `crates/app/src/games/runner/world.rs` build every `.gcad` with `gcad_bevy::build` and the runner's settings:

- `the_hero_builds_from_gcad_with_every_clip_on_its_rig`: 15 bones, the six clip names, 14000 triangles at most, height and feet, a clip moves the rig.
- `every_prop_builds_from_gcad_at_the_size_the_runner_spawns_it`: every prop builds, has 1 to 4000 triangles (the old `scripts/glb_audit.py` budget), no empty body, no empty or unnamed part, and its extent at the runner's scale is within 2% of the kernel GLB it replaced.
- `the_tree_builds_from_gcad_inside_its_budget`, `every_model_the_runner_loads_is_on_disk`.
- `every_model_the_runner_loads_is_warmed_at_startup` runs `ModelsPlugin` headless (no window, no GPU), waits for every scene and clip to load through the asset server, and checks the hero scene is fitted to 1 unit with all six clips. It caught gcad-bevy rebuilding the hero unfitted when its clips loaded single-threaded (fixed in gcad-engines 7d1fc40, `every_clip_and_the_scene_keep_the_fit_in_any_order`).
- `a_tap_does_not_start_the_run_before_the_hero_is_built` covers the loading gate.

`docs/hero/glb_vs_gcad.png` compares the old hero GLB (top) with the gcad build (bottom) for every clip from the game camera: 99.8% of pixels within 16 levels, mean difference 0.1.

## Props: gcad-mesh against the kernel

The props are flat solid profiles (gcad #271): `plane`, `rect`, `circle`, `poly`, `array`, `extrude`, `repeat`, rim `fillet`, one body per part, no booleans between bodies. All eight sources run on gcad-mesh unchanged, so nothing waits on the brep kernel. Differences from the old `gcad export` GLBs:

| prop | triangles, gcad-mesh / kernel | notes |
|---|---|---|
| barrier | 2608 / 1884 | fillets are four bevel rings |
| hurdle | 1484 / 932 | |
| gate | 2104 / 1072 | |
| train | 3144 / 848 | fillet rings on body, roof and nose; under the 4000 budget, no visible change, so no `decimate` |
| coin | 284 / 1444 | 32-segment circle against the kernel's finer one |
| arch | 2328 / 1712 | |
| road | 636 / 636 | |
| buildings 1-5 | 396, 660, 924, 1188, 660 / same | |

Normals are flat per face. Silhouettes are within 0.6% of the kernel (gcad `tests/solid_silhouette.rs`). `docs/props/glb_vs_gcad.png` shows train, coin and barrier from the front, side and three-quarter view, kernel GLB then gcad-mesh, through `model_sheet`. In the game, runner shots 201 and 204-208 before and after are 98.4-99.7% of pixels within 16 levels, mean difference 0.2-1.3 (the moving sparkles and shadows account for most of it).

## From the game's loader to gcad-bevy

Until November 2026 the game had its own 469-line loader in `crates/app/src/gcad.rs` on the `gcad2glb` library (mesh, material, clip and skeleton entities, `include` splicing, cache, warm-up). That code moved into gcad-bevy, which grew out of it, and the game now depends on the crate. Two changes went into gcad-engines first: Bevy's `multi_threaded` became a default feature the game turns off (44bc147), and `load_gcad_clip` keeps the scene's fit when Bevy runs single-threaded (7d1fc40). Runner shots 201 and 204 before and after are 98.9% and 98.4% of pixels within 16 levels, mean difference 0.7 and 0.8 (the moving sparkles).

Earlier the switch from gcad-mesh to the `gcad2glb` library changed nothing visible: tests, triangle counts and every prop extent stayed the same.

## Solid props through the kernel (`brep` feature): not enabled

Measured with the `gcad2glb` command (kernel on) against the gcad-mesh build:

| prop | triangles, kernel / gcad-mesh | build time, kernel | gcad-mesh |
|---|---|---|---|
| barrier | 1884 / 2608 | 246 ms | 0.1 ms |
| train | 848 / 3144 | 479 ms | 0.2 ms |
| coin | 1444 / 284 | 25 ms | 0.0 ms |

(desktop, includes process start.) The kernel adds about 13 MB stripped (gcad's own measure; the command binary is 18.9 MB) and about 42 s clean build time, and needs the monstertruck `[patch.crates-io]` lines. That is far over the 1 MB and 50 ms limits, and the props already look the same, so the game stays mesh-only. Only the train would lose triangles (3144 to 848), and it is inside the 4000 budget.


## Hero motion

1. Kimodo (NVIDIA, model under the NVIDIA Open Model License, outputs unrestricted) renders takes from text prompts on spark: repo `/tmp/kimodo`, venv `/tmp/kmv`; `art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/`. The chosen takes are kept in `art/3d/motion/bvh`.
2. `python3 art/3d/hero/retarget.py` retargets them onto the rig in `assets/models/hero.gcad` and writes `assets/models/motion.gcad`. `art/3d/hero/clips.json` picks one take per clip: `trim` frame range, `loop`, `amp` per bone, `arm_clear` degrees, `lift` plant or ground, `upright: false` to keep the take's own spine lean (otherwise spine and head lean is scaled down so the chibi does not hunch).
3. Keys are reduced per bone: a key is dropped when slerping its neighbours stays within `ANGLE_TOLERANCE` (0.4 degrees) of every frame between them, and the hip lift within `LIFT_TOLERANCE` (0.3 cm); numbers are rounded to 0.1 degree and 1 ms. That took motion.gcad from 271780 B (4224 keys) to 126538 B (2798 keys). `cargo run --release -p kids_games --example motion_error -- full.gcad assets/models/motion.gcad` skins both at 120 Hz: worst vertex error 0.67 cm on the 134 cm hero.

## Seeing a model

`cargo run --release -p kids_games --example model_sheet -- models/hero.gcad --clips run,jump --views game,side --out /tmp/sheet.png`

Renders a `.gcad` through gcad-bevy and the game's animation path, headless, into one PNG: a row per clip and view, a column per frame. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. `gcad render` and `gcad inspect` show the source with grids and part names; `gcad view` opens the live viewer.

## History

Before November 2026 the hero and props were Hunyuan3D-2 meshes (`art/3d/hero/build.py`, `art/3d/props.py`, `art/3d/prop_build.py`, kept for reference). See `docs/LICENSES.md` for why none of that output ships any more.
