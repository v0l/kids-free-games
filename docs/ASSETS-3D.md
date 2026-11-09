# 3D asset pipeline

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

The game logs `every model ready N ms after start, B builds, K baked` when the last model lands. Final branch, shipped bake: desktop headless sway 121-124 ms, Android emulator `kidsgames-test` x86_64 948-1078 ms (software rendering, busy host), iOS simulator iPad Pro 11-inch (M4) 174-179 ms, web headless Chrome 2514 ms, all with 0 builds and 14 baked.

On the web the models wait for the 13 MB of sound files that load first; loading them after the models would cut that number further.

Source sizes: the sixteen `.gcad` files are 140549 B (29069 B gzipped), of which the props are 9313 B. The twelve prop GLBs they replace were 1100352 B; before that the hero and tree GLBs were 722524 B.

With the shipped bake (same machine and profile, measured by building with and without `assets/gcad-baked`):

| | without bake | with bake | delta |
|---|---|---|---|
| Web bundle (`scripts/web.sh`, small) | 49615608 B | 50951885 B | +1336277 B raw (392868 B gzipped) |
| Android x86_64 APK (`scripts/android.sh`, small) | 35720095 B | 36121911 B | +401816 B (the APK deflates it) |
| iOS simulator app | 40900 KiB | 42240 KiB | +1340 KiB |


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

## Primitive games

The other 3D games used to build every object in Rust from `game3d::Shape` primitives. They move to `.gcad` one game at a time (#336). The Happy Hotel is done; the rest follow the same rules.

### How a primitive game loads its models

- Each fixed object is one `.gcad` file in `assets/models/`, named `<game>_<object>.gcad` (a flat folder, because the web, Android, iOS and macOS scripts copy `assets/models/*`). Files that are only included end in `_base` (`hotel_bed_base.gcad`); a variant is two lines, `let fancy=1` and `include hotel_bed_base.gcad`.
- Models are built mesh-only by gcad-bevy at their own millimetre-free scale (`GcadSettings::default()`, no fit: one gcad unit is one game metre, Z up becomes Y up). The game lists its models in one `static` (`hotel::world::MODELS`).
- `gcad::ModelsPlugin` starts loading every listed model at app start next to the runner's. `game3d::stock(MODELS)` waits until they are loaded, takes each part's mesh out of the built scene, drops its vertex colours and keeps its colour and gloss, and registers it as `Shape::Model(name, part)` in the `Palette`. The game's `sync` runs only once the shelf is stocked (`stocked(MODELS)`). Without an asset server (headless tests) the shelf builds the same files synchronously with `gcad_bevy::build`.
- Instances keep sharing one mesh handle per part, so a hundred guests draw from the same meshes as before; materials stay the Kit's cached `Paint`s, keyed by colour and finish.

### Recolouring and animation hooks (decision)

- Colour per instance goes through part names, not `set` overrides: `Model::model(shelf, name, at, |part| Tint)` places every part and lets the game return a colour, a hide, an extra transform or an `Anim` per part name. A `set name=value` override would need one build per colour (guests have ten shirts, five hair colours, four skins), so it is kept for shape variants only (`fancy`, `full`, `h`, `up`).
- A part's colour in the file is its default; the hotel overrides `torso`, `arm_*`, `hand_*`, `head`, `hair`, `hat`, `brim`, legs, sheets, wood, rugs, stools and cushions from `Look` and the theme.
- Animation stays per part: the hotel tags `leg_l`, `shoe_l`, `arm_l` (`Anim::Leg`, `Anim::Arm`), the swimmer's `arm_*`, `leg_*`, `head` (`Stroke`, `Kick`, `Turn`) and the coin top (`Bob`), and `animate` moves those entities as before. Characters that need bones later use the gcad rig and clips like the runner hero.
- Gloss comes from the file: `color ... gloss=0.6` on a mesh shell gives `Finish::Gloss`, a flat body is `Finish::Matte`.

### gcad-mesh additions

The hotel needed shapes gcad-mesh did not have, added to gcad master in 434c02e (tests in `crates/gcad-mesh/tests/shapes.rs`) and pinned in gcad-engines e367a57:

- `ngon d n` (an n-sided cylinder when extruded), `ellipse dx dy` (the rug).
- `extrude d scale=k` (a frustum: pots, bins, toilets, lamp shades) and `scale=0` (a cone: the umbrella).
- `blob ... mode=shell segments=N`: a UV ellipsoid of `N*N` triangles. A plain shell meshes to about 2000 triangles, which put a guest at 27000.

Rounded boxes (`block round=`) and spheres already existed; torus and sector are left for Helix and Ball Sort.

### Hotel inventory

| model | made of | instances | per-instance |
|---|---|---|---|
| `hotel_person`, `_up`, `_sit`, `_sit_up` | legs, shoes, boots, sleeves (extrusions), hat and brim (10-gons), torso, hands, head, hair, eyes, glints, mouth (shells) | every guest, staff and the player | `Look` colours, `wide`, hidden hat/boots, leg and arm swing |
| `hotel_person_lie` | head, hair | sleeping guests | skin, hair |
| `hotel_swimmer` | torso, arms, legs, head, cap, goggles | swimming guests | `Look` colours, stroke, kick, head turn |
| `hotel_desk`, `hotel_counter`, `hotel_sink`, `hotel_shelf`, `hotel_partition` | boxes with a top cap | one each, shelves two | trim colour, shelf items |
| `hotel_bed`, `hotel_bed_fancy` | frame, sheet, headboard, pillows, crown | one per room | sheet and wood colour, room scale |
| `hotel_plant`, `_mid`, `_tall` | tapered pot, three leaf shells | 8 to 20 | |
| `hotel_sofa`, `hotel_wardrobe`, `hotel_tv`, `hotel_lamp`, `hotel_rug`, `hotel_bin`, `hotel_bin_full`, `hotel_laundry` | boxes, 8 and 10-gons, frustums, ellipses, shells | per room or lobby | accent and wood colours |
| `hotel_toilet`, `hotel_roll`, `hotel_burger`, `hotel_towel` | | per stall, carried items | |
| `hotel_table`, `hotel_stool`, `hotel_sunbed`, `hotel_umbrella` | | cafe and pool | accent and trim colours |
| `hotel_car`, `hotel_entrance` | | two cars, one door frame | car colour, trim |
| `hotel_coin`, `hotel_coin_top` | 10-gon column, scaled per coin | coin piles and tips | height, bob |

Still drawn from primitives in the hotel, on purpose: floors, zones, the pool water, ripples, wakes, dust, shadows, pads and their progress dots (flat layers sized from the layout each frame or animated by scale), the walls, locked doors and pool rim (boxes sized from the layout, `cuboid`/`cuboid2`), and the splash drops and rings (scaled every frame).

### Procedural geometry that stays in Rust

| game | what | why |
|---|---|---|
| Hotel | walls, locked doors, pool rim, floors, zones, pads | sized from `Layout` per hotel; a model per size would be a file per wall |
| Paper Land | land contours and trails (`paper/land.rs`) | marching squares over the live ownership grid |
| Helix | ring sectors (`Shape::Sector`) | sized by `RING_OUTER`, `RING_INNER`, `SEGMENTS` and generated per tower |

### Checks

`crates/app/src/gcad.rs` tests build every shipped `.gcad` (everything in `assets/models` but `*_base` and `motion`) with `gcad_bevy::build`: each builds mesh-only, has triangles within its budget (hero 14000, tree 12000, people 3000, other hotel props 1500, other props 4000), no empty or unnamed part and a real size; every hotel model is listed in `MODELS` and on disk; people have every part the hotel recolours and animates; the bed, desk, sofa, car, wardrobe and umbrella keep the size of the primitives they replaced within 3 cm; all hotel models build in under 1.5 s on the test machine. The hotel and trains tests run against the gcad models.

### Hotel numbers

- Triangles: a standing person is 1696 (the primitive version was 4952: ten 480-triangle spheres, six boxes and two 10-sided cylinders), the swimmer about 1500, every prop under 1500.
- Build on the Android emulator `kidsgames-test` (x86_64, cold start with app data cleared, `gcad_bevy` debug log, measured before the shipped bake landed), three runs: the 36 hotel models take 0 or 1 ms each, 8 to 10 ms together, against the 150 ms budget. The hero (1034-1088 ms) and tree (219-241 ms) are unchanged. `bake_settings` now lists every hotel model, so release builds ship them in `assets/gcad-baked` (50 entries) and do not build them at all.
- Emulator: the hotel opens and draws with every model from `.gcad` (`docs/shots/hotel-emulator.png`).
- Screenshots 170-183 before and after (`scripts/shots.sh`, headless Chrome): 99.5-99.95% of pixels within 16 levels, mean difference 0.03-0.27. The low-poly shells differ from the old 20-segment spheres at the edges only.
- Sizes against 83f7744 (same machine and profile): web bundle 49540006 B to 49664393 B, wasm 25939198 B to 26053064 B, Android arm64 APK 35774371 B to 35841018 B. The 42 hotel `.gcad` files are 10521 B (2543 B gzipped). iOS not rebuilt for this step.

## Hero motion

1. Kimodo (NVIDIA, model under the NVIDIA Open Model License, outputs unrestricted) renders takes from text prompts on spark: repo `/tmp/kimodo`, venv `/tmp/kmv`; `art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/`. The chosen takes are kept in `art/3d/motion/bvh`.
2. `python3 art/3d/hero/retarget.py` retargets them onto the rig in `assets/models/hero.gcad` and writes `assets/models/motion.gcad`. `art/3d/hero/clips.json` picks one take per clip: `trim` frame range, `loop`, `amp` per bone, `arm_clear` degrees, `lift` plant or ground, `upright: false` to keep the take's own spine lean (otherwise spine and head lean is scaled down so the chibi does not hunch).
3. Keys are reduced per bone: a key is dropped when slerping its neighbours stays within `ANGLE_TOLERANCE` (0.4 degrees) of every frame between them, and the hip lift within `LIFT_TOLERANCE` (0.3 cm); numbers are rounded to 0.1 degree and 1 ms. That took motion.gcad from 271780 B (4224 keys) to 126538 B (2798 keys). `cargo run --release -p kids_games --example motion_error -- full.gcad assets/models/motion.gcad` skins both at 120 Hz: worst vertex error 0.67 cm on the 134 cm hero.

## Seeing a model

`cargo run --release -p kids_games --example model_sheet -- models/hero.gcad --clips run,jump --views game,side --out /tmp/sheet.png`

Renders a `.gcad` through gcad-bevy and the game's animation path, headless, into one PNG: a row per clip and view, a column per frame. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. `gcad render` and `gcad inspect` show the source with grids and part names; `gcad view` opens the live viewer.

## History

Before November 2026 the hero and props were Hunyuan3D-2 meshes (`art/3d/hero/build.py`, `art/3d/props.py`, `art/3d/prop_build.py`, kept for reference). See `docs/LICENSES.md` for why none of that output ships any more.
