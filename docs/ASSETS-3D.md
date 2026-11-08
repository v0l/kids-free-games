# Lane Runner asset pipeline

Every visual in Lane Runner is made in-house. Nothing is Kenney or stock.

| Asset | Source | Files |
|---|---|---|
| Sky backdrop | FLUX.1-dev sprite | `art/gen/prompts/runner.json`, `art/runner.txt` |
| Hero, motion clips, tree, barrier, hurdle, gate, train, coin, finish arch, road, buildings | gcad text, built at startup by gcad-mesh | `assets/models/*.gcad` |

## Models built at startup

Every model ships as the `.gcad` text itself (`assets/models/*.gcad`, copied by `scripts/web.sh`, `scripts/android.sh` and `scripts/ios.sh`). There is no export step and no `.glb` in the game: edit a `.gcad` and run the game. `crates/app/src/gcad.rs` is a Bevy asset loader for `.gcad`: it splices `include` lines through the asset reader (so it works from the Android APK and over HTTP on the web), builds the model with `gcad_mesh::build_with` (gcad pinned by git rev in `crates/app/Cargo.toml`, `parallel` on everything but wasm32), and turns the result into a scene: one mesh and material per part with vertex colours, a joint entity per bone, a `SkinnedMesh`, and an `AnimationClip` per clip as `models/hero.gcad#Animation/<name>`. The model is scaled to 1 unit high with its feet at 0 (the runner then scales it to the height in `prop(c, models, name, h, at)`); `GcadSettings::scale` gives a fixed scale instead (0.01 for the road and buildings, which the runner places in metres from their millimetre sources). Unrigged models are centred on x and z.

The five buildings are `prop_building1.gcad` to `prop_building5.gcad`, two lines each: `let style= floors=` and `include building_base.gcad`.

`GcadPlugin` warms every model in `WARM` on a background task at app start, and the loader runs on Bevy's asset threads, so nothing builds on the main thread. A process-wide cache keyed by the text and detail level builds each model once even when the scene and its six clips are requested separately. The runner stays on "Getting ready..." and ignores taps until the hero has loaded (`RunnerPlay::loaded`).

Build time per model (release, best of three; the hero includes its clips; props are all twelve together):

| | hero | tree | 12 props | total |
|---|---|---|---|---|
| Desktop, 48 threads (`cargo run --release -p kids_games --example gcad_timing`) | 449 ms | 80 ms | 1.5 ms | 531 ms |
| Desktop, one core (`taskset -c 0`) | 870 ms | 201 ms | 1.6 ms | 1073 ms |
| Android emulator `kidsgames-test` (x86_64, 4 cores, swiftshader), three cold starts | 912-1318 ms | 194-285 ms | 9-10 ms | 1.1-1.6 s |
| Web, headless Chrome (wasm32, single thread, busy machine) | 1504 ms | 370 ms | 12 ms | 1.9 s |

All of it is under the 2 s budget and off the main thread, so there is no on-disk cache. `GcadSettings::detail` (2 halves the grid) is there for slow devices.

Sizes: the sixteen `.gcad` files are 140549 B (29069 B gzipped), of which the props are 9313 B. The twelve prop GLBs they replace were 1100352 B; before that the hero and tree GLBs were 722524 B.

| | before (GLB props) | after |
|---|---|---|
| Web bundle (`scripts/web.sh`, small) | 51296727 B | 49314507 B |
| Web wasm | 26603801 B | 25713699 B (no `bevy_gltf`) |
| Android arm64 APK | 36077613 B | 35640051 B |
| iOS simulator app | 42268 KiB | 40752 KiB |

## Tests

Tests in `crates/app/src/games/runner/world.rs` build every `.gcad` with gcad-mesh:

- `the_hero_builds_from_gcad_with_every_clip_on_its_rig`: 15 bones, the six clip names, 14000 triangles at most, height and feet, a clip moves the rig.
- `every_prop_builds_from_gcad_at_the_size_the_runner_spawns_it`: every prop builds, has 1 to 4000 triangles (the old `scripts/glb_audit.py` budget), no empty body, no empty or unnamed part, and its extent at the runner's scale is within 2% of the kernel GLB it replaced.
- `the_tree_builds_from_gcad_inside_its_budget`, `every_model_the_runner_loads_is_on_disk`, `every_model_the_runner_loads_is_warmed_at_startup`.
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

When gcad grows a runtime library for solid parts that gcad-mesh cannot build (real booleans, fillets on non-extrude edges), a prop that needs them switches its loader to that crate; none does today.

## Hero motion

1. Kimodo (NVIDIA, model under the NVIDIA Open Model License, outputs unrestricted) renders takes from text prompts on spark: repo `/tmp/kimodo`, venv `/tmp/kmv`; `art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/`. The chosen takes are kept in `art/3d/motion/bvh`.
2. `python3 art/3d/hero/retarget.py` retargets them onto the rig in `assets/models/hero.gcad` and writes `assets/models/motion.gcad`. `art/3d/hero/clips.json` picks one take per clip: `trim` frame range, `loop`, `amp` per bone, `arm_clear` degrees, `lift` plant or ground, `upright: false` to keep the take's own spine lean (otherwise spine and head lean is scaled down so the chibi does not hunch).
3. Keys are reduced per bone: a key is dropped when slerping its neighbours stays within `ANGLE_TOLERANCE` (0.4 degrees) of every frame between them, and the hip lift within `LIFT_TOLERANCE` (0.3 cm); numbers are rounded to 0.1 degree and 1 ms. That took motion.gcad from 271780 B (4224 keys) to 126538 B (2798 keys). `cargo run --release -p kids_games --example motion_error -- full.gcad assets/models/motion.gcad` skins both at 120 Hz: worst vertex error 0.67 cm on the 134 cm hero.

## Seeing a model

`cargo run --release -p kids_games --example model_sheet -- models/hero.gcad --clips run,jump --views game,side --out /tmp/sheet.png`

Renders a `.gcad` through the game's own loader and animation path, headless, into one PNG: a row per clip and view, a column per frame. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. `gcad render` and `gcad inspect` show the source with grids and part names; `gcad view` opens the live viewer.

## History

Before November 2026 the hero and props were Hunyuan3D-2 meshes (`art/3d/hero/build.py`, `art/3d/props.py`, `art/3d/prop_build.py`, kept for reference). See `docs/LICENSES.md` for why none of that output ships any more.
