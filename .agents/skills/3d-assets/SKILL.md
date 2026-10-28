---
name: 3d-assets
description: >
  Make animated 3D game assets (character, creature, prop) for the kids games: generate a mesh
  from a FLUX concept image, texture it, rig it, animate it, then either bake it to sprite sheets
  for the 2D art pipeline or export a checked .glb. Use when a game needs a 3D model, an animated
  character, or turntable/animation frames rendered from a model.
---

# 3D assets

Pipeline, in order. Each stage has a gate; do not continue past a failed gate.

```
concept (FLUX, art/gen) -> image-to-3D mesh -> cleanup + retopo (Blender) -> texture/PBR
  -> auto-rig -> animate -> audit -> bake to sprites  OR  export .glb
```

Art style is the painted 3D-cartoon look in docs/STYLE.md. The concept image is the style anchor:
every later stage must still look like it.

## Tools (all on spark unless noted)

| Stage | Tool | Notes |
|---|---|---|
| Concept | `art/gen/make.sh` (FLUX.1-dev, ComfyUI) | One subject, 3/4 view, neutral A or T pose, plain background, `cut: true` so rembg removes it. Shared style line ends every prompt. |
| Image to mesh | Hunyuan3D-2 / 2.1 (github.com/Tencent-Hunyuan/Hunyuan3D-2). Shape 6 GB VRAM, shape+texture 16 GB. `api_server.py --port 8080`, `Hunyuan3DPaintPipeline` for textures, 2.1 adds PBR. | Preferred. Check its licence terms before shipping anything; it is not plain MIT. |
| Image to mesh (alt) | TRELLIS (github.com/microsoft/TRELLIS, MIT, 16 GB). `pipeline.run(image)` then `postprocessing_utils.to_glb(..., simplify=0.95, texture_size=1024)`. | Use when Hunyuan gives a bad silhouette; try both, keep the better. |
| ComfyUI route | kijai/ComfyUI-Hunyuan3DWrapper, MrForExample/ComfyUI-3D-Pack | Same models inside the ComfyUI already on spark. |
| Rig | UniRig (github.com/VAST-AI-Research/UniRig, MIT, 8 GB). In: obj/fbx/glb. Out: rigged glb/fbx with skeleton + skin weights. | Humanoids, quadrupeds, props with limbs. For simple wobblers skip it and animate procedurally. |
| Animate / cleanup / render / export | Blender headless (`blender -b -P script.py`), `bpy` | See below. |
| Agent control | blender-mcp (github.com/ahujasid/blender-mcp): socket on 9876, tools for objects, materials, viewport/wireframe/animation-strip screenshots, runs Python. Also wraps Hunyuan3D, Meshy, Tripo, Rodin, Poly Haven, Sketchfab. | Use only for interactive iteration, on localhost, with `BLENDER_MCP_SAFE_MODE=1`. The socket has no auth. Final assets must come from a re-runnable `bpy` script, not MCP clicks. |
| Reference skill | lovecatisgood-sudo/3d-asset-generation-blender-unity-game-development-skills (blender-3d-asset-generation) | Not installed. Read its `references/automation-and-qa.md` and `scripts/audit_blender_asset.py` for ideas. Install only with the user's OK. |

Blender (`bpy`) is not installed on spark or locally yet. Get it with
`pip install bpy` in a venv on spark (needs Python 3.11) or a Blender tarball, and record the
path in a `remember` note.

## Layout

```
art/3d/<asset>/
  concept.json      FLUX job (same schema as art/gen/prompts)
  build.py          bpy script: import mesh, clean, retopo, material, rig bind, actions, render/export
  actions.json      clip name -> frame range, loop flag
  README.md         seeds, model versions, what was hand-fixed
```

Generated meshes, textures and renders go to `art/3d/out/` (gitignored). Commit only `build.py`,
`concept.json`, `actions.json` and the final baked sheet or glb (`git add -f`).

## Stages and gates

1. **Concept.** Render 4 seeds, pick one. Gate: clean silhouette, limbs separated from the body
   (generators fuse touching limbs), no text, no held objects unless the game needs them.
2. **Mesh.** Run Hunyuan3D (or TRELLIS) on the cut PNG. Save the raw glb with seed and model
   version in README. Gate: one connected body, no floating shards, closed surface.
3. **Cleanup.** In `build.py`: delete loose parts, merge by distance, recalc normals outward,
   decimate (Quadric) to the tri budget, set origin at feet, apply scale, Y-up/-Z-forward on export.
   Budgets: character 3k-6k tris, prop under 1.5k, one 1024 texture (512 for web builds).
4. **Texture.** Keep the generator's baked texture when it matches the concept. For PBR use
   Hunyuan3D-2.1. Painted look means low metallic, rough 0.6-0.9, no specular noise. Gate:
   turntable render beside the concept image looks like the same character.
5. **Rig.** UniRig on the cleaned mesh, then in Blender check the bone count (target 20-40 for a
   biped, cap 64), rename to a stable scheme (`root, hips, spine, head, arm.L ...`), and test by
   posing extremes. Gate: elbows, knees and the mouth/ears do not tear when posed. Bad skin
   weights: re-run with a different seed or fix with `bpy.ops.object.parent_set(type='ARMATURE_AUTO')`.
6. **Animate.** Clips the games need: `idle` (loop, 1-2 s), `walk`/`run` (loop), `happy` (one-shot),
   `fail` (one-shot), plus game-specific ones. Write keyframes in `build.py` from `actions.json`
   (procedural pose keys, squash and stretch on the root, anticipation before jumps, see the
   game-feel skill). Retargeting mocap (Mixamo style) is allowed if the licence allows redistribution.
   Gate: loops close (first frame equals last), root does not drift, feet do not slide.
7. **Audit.** Run `python3 scripts/glb_audit.py <file.glb>`; it fails on missing normals/UVs, tri
   or bone budget overrun, unnamed animations, bounds not near the origin, and textures over budget.
   Also render a 4-angle contact sheet and look at it with `read_file`. Do not skip the look.
8. **Deliver**, one of:
   - **Sprites (default).** Render each clip from a fixed camera in Blender (Workbench/EEVEE,
     transparent film, 256 px frames, 8-12 fps, 8 frames per angle for turntables) to
     `art/3d/out/<asset>/<clip>_NN.png`, then add the frames to an `art/<sheet>.txt` manifest
     (`name | gen:<file>.png | ...`) and run `python3 scripts/assets.py art`. This costs no
     engine support and keeps the web wasm small.
   - **Live 3D (Bevy).** Export glb. Enabling `bevy_gltf` adds 1.4 MB to the web wasm
     (docs/BEVY.md), so for simple things prefer `hotel::build::Model` primitives. If glb is
     justified, put the budget numbers in the commit body.

## Rules

- Everything is reproducible: seeds, model versions and `build.py` are committed; never ship an
  asset that only exists as a hand-edited .blend.
- License check before use: Kenney/CC0 and own generations are fine; Hunyuan3D, Meshy, Tripo and
  Rodin outputs each carry terms, note which model made the asset in its README.
- Kid-friendly: no scary faces, no sharp teeth, big eyes, rounded forms (docs/STYLE.md).
- A mesh generator is a starting point. Expect to repair topology and weights; budget two
  iterations before judging a model unfit.
- Stay off the user's screen: Blender runs `-b` only, previews are PNG renders.

## Sources

- github.com/microsoft/TRELLIS, github.com/Tencent-Hunyuan/Hunyuan3D-2, github.com/VAST-AI-Research/UniRig
- github.com/ahujasid/blender-mcp
- skills.sh: lovecatisgood-sudo/.../blender-3d-asset-generation, silvio-l/skills@game-asset-director
