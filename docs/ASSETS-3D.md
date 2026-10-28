# Lane Runner asset pipeline

Every visual in Lane Runner is generated in-house. Nothing is Kenney or stock.

| Asset | Source | Prompts / script |
|---|---|---|
| Sky, road, pavement, wall tile | FLUX.1-dev, mirror-tiled | `art/gen/prompts/runner.json` |
| Props (barrier, hurdle, gate, train, coin, tree, finish arch) | FLUX.1-dev, rembg cut | `art/gen/prompts/runner_props.json` |
| Hero (run, idle, jump, slide, cheer, fall frames) | FLUX concept, Hunyuan3D-2 mesh and texture, Blender rig and clips, rendered to sprites | `art/gen/prompts/player3d.json`, `art/3d/player/build.py` |

Hero steps, all on spark:

1. `art/gen/make.sh art/gen/prompts/player3d.json` renders four concepts. Pick the cleanest A-pose.
2. Hunyuan3D-2 (`/tmp/Hunyuan3D-2`, venv `/tmp/3d`) makes the shape, then the texture, from the cut image.
3. `blender -b -P art/3d/player/build.py -- tex.glb out.glb sheets/` cleans and decimates to 5000 tris, builds a 14 bone rig, keys the clips from code, exports a glb and renders every clip from front, back and side.
4. `python3 scripts/glb_audit.py out.glb 5000` checks the budget.
5. Chosen frames are listed in `art/runner.txt` and packed by `python3 scripts/assets.py art`. `hero_frame` in `crates/app/src/games/runner/world.rs` picks the frame from game state.

Tests: `every_hero_frame_exists_in_the_runner_sheet`, `every_runner_prop_exists_in_the_runner_sheet`.

## Seeing a model

`cargo run -p kids_games --example model_sheet -- models/hero.glb --clips run,jump --views game,side --out /tmp/sheet.png`

Renders the glb through Bevy's own glTF and animation path, headless (no window, so it never touches the desktop), and writes one PNG: a row per clip and view, a column per frame. Without `--clips` a model with no animations gets a turntable per view. Views: `front side back three game`. Flags: `--size`, `--frames`, `--height`, `--assets`. Read the PNG with an image viewer tool to check poses, the real in-game result.

## Hero motion (Kimodo)

Clips are motion capture quality motion from NVIDIA Kimodo (text to motion, SOMA skeleton, BVH), retargeted onto the 14 bone hero rig.

1. Kimodo lives on spark: repo `/tmp/kimodo`, venv `/tmp/kmv` (needs HF access to `meta-llama/Meta-Llama-3-8B-Instruct`). Its `MotionCorrection` package builds on ARM with x86 intrinsics mapped by SIMDe (`#include <simde/x86/avx2.h>` in `src/cpp/Math/SIMD.h`, `-msse4.1 -mavx` removed from CMakeLists).
2. `TEXT_ENCODER_MODE=local PYTHONPATH=/tmp/kimodo /tmp/kmv/bin/python art/3d/motion/kimodo_batch.py art/3d/motion/hero.json out/` renders several seeds per prompt (encoder load is about 8 minutes, then seconds per clip).
3. `art/3d/motion/hero_clips.json` picks one take per game clip and how to cut it: `loop` finds the closest matching pose pair for a seamless cycle, `trim: air|low` keeps the airborne or crouched span, `amp` scales a bone group's rotation (cartoon exaggeration), `lift: false` pins the hips.
4. `blender -b -P art/3d/player/build.py -- tex.glb assets/models/hero.glb sheets/ art/3d/motion/hero_clips.json` retargets by bone directions (limbs) and trunk frames (hips, chest, head) measured against the BVH rest pose, then exports at 30 fps.
5. Check with `model_sheet`. The runner scales jump and slide playback to the game's air time and slide time, and the run to ground speed.
