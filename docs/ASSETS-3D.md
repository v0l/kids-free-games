# Lane Runner asset pipeline

Every visual in Lane Runner is generated in-house. Nothing is Kenney or stock.

| Asset | Source | Files |
|---|---|---|
| Sky, road, pavement, wall tile | FLUX.1-dev, mirror-tiled sprites | `art/gen/prompts/runner.json`, `art/runner.txt` |
| Props (barrier, hurdle, gate, train, coin, tree, finish arch) | FLUX.1-dev concept, Hunyuan3D-2 mesh and texture | `art/gen/prompts/runner_props.json`, `art/3d/props.py`, `art/3d/prop_build.py`, `assets/models/prop_*.glb` |
| Hero | FLUX concept, Hunyuan3D-2 mesh and texture, Kimodo motion | `art/3d/hero/`, `art/3d/motion/`, `assets/models/hero.glb` |

The runner loads the glbs with `bevy_gltf` (`Models` in `crates/app/src/games/runner/world.rs`) and plays the hero clips with an `AnimationGraph`. `every_model_the_runner_loads_is_a_glb_on_disk_with_its_clips` checks the files.

## Hero mesh

1. `art/gen/make.sh art/gen/prompts/player3d.json` renders four concepts (`art/3d/hero/concept.json`). Pick the cleanest A-pose.
2. Hunyuan3D-2 on spark (`/tmp/Hunyuan3D-2`, venv `/tmp/3d`) makes the shape, then the texture, from the cut image.

## Hero motion (Kimodo)

1. Kimodo lives on spark: repo `/tmp/kimodo`, venv `/tmp/kmv` (needs HF access to `meta-llama/Meta-Llama-3-8B-Instruct`). Its `MotionCorrection` package builds on ARM with x86 intrinsics mapped by SIMDe (`#include <simde/x86/avx2.h>` in `src/cpp/Math/SIMD.h`, `-msse4.1 -mavx` removed from CMakeLists).
2. `TEXT_ENCODER_MODE=local PYTHONPATH=/tmp/kimodo /tmp/kmv/bin/python art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/` renders several seeds per prompt (the encoder takes about 8 minutes to load, then seconds per clip). The chosen takes are kept in `art/3d/motion/bvh`.
3. `art/3d/motion/hero_clips.json` picks one take per game clip: `loop` cuts a seamless cycle at the closest matching pose pair, `trim: air|low` keeps the airborne or crouched span, `lift: false` pins the hips, `arm_clear` (degrees, default 30) keeps the arms off the wide jacket, `amp` scales a bone's rotation.

## Rig and retarget

`blender -b -P art/3d/hero/build.py -- tex.glb assets/models/hero.glb art/3d/motion/hero_clips.json`

- Cleans and decimates the mesh to 5000 tris, builds a 16 bone rig (hips, chest, head, upper arm, forearm, hand, thigh, shin, foot) with joints placed on the mesh, and skins it with automatic weights.
- Copies rotations, not directions: each frame, every source bone's world rotation relative to its BVH rest pose is applied to the matching bone, after a fixed rest offset that lines our bone up with the source bone. Wrist turns, elbow direction and shoulder twist come through as captured.
- The only chibi correction is `arm_clear`: if an upper arm would swing into the torso it is turned outward just enough, carrying the forearm and hand with it.
- Exports at 30 fps. The runner scales jump and slide playback to the game's air time and slide time, and the run to ground speed. The game adds nothing but the landing squash on top.

## Seeing a model

`cargo run -p kids_games --example model_sheet -- models/hero.glb --clips run,jump --views game,side --out /tmp/sheet.png`

Renders the glb through Bevy's own glTF and animation path, headless (no window, so it never touches the desktop), and writes one PNG: a row per clip and view, a column per frame. Without `--clips` a model with no animations gets a turntable per view. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. Read the PNG with an image viewer tool to check poses, the real in-game result. `python3 scripts/glb_audit.py file.glb 5000` checks budgets.
