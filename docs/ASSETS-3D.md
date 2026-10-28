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
