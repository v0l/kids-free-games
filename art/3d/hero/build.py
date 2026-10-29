import json
import math
import sys

import bpy
from mathutils import Matrix, Quaternion, Vector

argv = sys.argv[sys.argv.index("--") + 1:]
SRC, OUT, MOTION = argv[0], argv[1], argv[2]
TRIS = 5000
FPS = 30
TEX = 512

SOURCE_OF = {
    "hips": "Hips",
    "spine": "Chest",
    "head": "Head",
    "arm.L": "LeftArm",
    "forearm.L": "LeftForeArm",
    "hand.L": "LeftHand",
    "arm.R": "RightArm",
    "forearm.R": "RightForeArm",
    "hand.R": "RightHand",
    "leg.L": "LeftLeg",
    "shin.L": "LeftShin",
    "foot.L": "LeftFoot",
    "leg.R": "RightLeg",
    "shin.R": "RightShin",
    "foot.R": "RightFoot",
}
TIP_OF = {
    "Hips": "Spine1",
    "Chest": "Neck1",
    "Head": "HeadEnd",
    "LeftArm": "LeftForeArm",
    "LeftForeArm": "LeftHand",
    "LeftHand": "LeftHandMiddle4",
    "RightArm": "RightForeArm",
    "RightForeArm": "RightHand",
    "RightHand": "RightHandMiddle4",
    "LeftLeg": "LeftShin",
    "LeftShin": "LeftFoot",
    "LeftFoot": "LeftToeBase",
    "RightLeg": "RightShin",
    "RightShin": "RightFoot",
    "RightFoot": "RightToeBase",
}

JOINTS = {
    "root": ((0, 0, 0.0), (0, 0, 0.1), None),
    "hips": ((0, 0, 0.26), (0, 0, 0.36), "root"),
    "spine": ((0, 0, 0.36), (0, 0, 0.56), "hips"),
    "head": ((0, 0, 0.56), (0, 0, 0.98), "spine"),
}
for side, sx in (("L", 1), ("R", -1)):
    JOINTS[f"arm.{side}"] = ((sx * 0.15, 0.04, 0.44), (sx * 0.205, 0.045, 0.402), "spine")
    JOINTS[f"forearm.{side}"] = ((sx * 0.205, 0.045, 0.402), (sx * 0.253, 0.049, 0.366), f"arm.{side}")
    JOINTS[f"hand.{side}"] = ((sx * 0.253, 0.049, 0.366), (sx * 0.321, 0.062, 0.282), f"forearm.{side}")
    JOINTS[f"leg.{side}"] = ((sx * 0.065, 0, 0.25), (sx * 0.065, 0.005, 0.14), "hips")
    JOINTS[f"shin.{side}"] = ((sx * 0.065, 0.005, 0.14), (sx * 0.07, 0.03, 0.05), f"leg.{side}")
    JOINTS[f"foot.{side}"] = ((sx * 0.07, 0.03, 0.05), (sx * 0.145, -0.07, 0.02), f"shin.{side}")


def load_mesh():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=SRC)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    body = bpy.context.view_layer.objects.active
    body.name = "player"
    if body.parent:
        bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    for o in list(bpy.context.scene.objects):
        if o.type == "EMPTY":
            bpy.data.objects.remove(o)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=1e-5)
    bpy.ops.mesh.normals_make_consistent(inside=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.modifier_add(type="TRIANGULATE")
    bpy.ops.object.modifier_apply(modifier="Triangulate")
    dec = body.modifiers.new("dec", "DECIMATE")
    dec.ratio = min(1.0, TRIS / max(1, len(body.data.polygons)))
    bpy.ops.object.modifier_apply(modifier="dec")
    zs = [v.co.z for v in body.data.vertices]
    k = 1.0 / (max(zs) - min(zs))
    body.scale = (k, k, k)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    vs = body.data.vertices
    low = min(v.co.z for v in vs)
    cx = sum(v.co.x for v in vs) / len(vs)
    cy = sum(v.co.y for v in vs) / len(vs)
    body.location = (-cx, -cy, -low)
    bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
    for p in body.data.polygons:
        p.use_smooth = True
    for img in bpy.data.images:
        if img.size[0] > TEX:
            img.scale(TEX, TEX)
    return body


def build_rig(body):
    data = bpy.data.armatures.new("rig")
    rig = bpy.data.objects.new("rig", data)
    bpy.context.scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    for name, (head, tail, parent) in JOINTS.items():
        b = data.edit_bones.new(name)
        b.head, b.tail = Vector(head), Vector(tail)
        if parent:
            b.parent = data.edit_bones[parent]
            b.use_connect = Vector(head) == data.edit_bones[parent].tail
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.parent_set(type="ARMATURE_AUTO")
    groups = {g.name for g in body.vertex_groups}
    bare = sum(1 for v in body.data.vertices if not any(g.weight > 0 for g in v.groups))
    if bare or not all(n in groups for n in JOINTS if n != "root"):
        sys.exit(f"automatic weights failed: {bare} bare, groups {sorted(groups)}")
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="POSE")
    for pb in rig.pose.bones:
        pb.rotation_mode = "QUATERNION"
    return rig


def import_bvh(path):
    objs = set(bpy.data.objects)
    acts = set(bpy.data.actions)
    bpy.ops.import_anim.bvh(
        filepath=path, axis_forward="-Z", axis_up="Y", global_scale=0.01,
        use_fps_scale=False, update_scene_fps=False, rotate_mode="NATIVE",
    )
    src = [o for o in bpy.data.objects if o not in objs][0]
    return src, [a for a in bpy.data.actions if a not in acts]


def rest_alignment(rig, src, frame):
    out = {}
    for ours, theirs in SOURCE_OF.items():
        b = rig.data.bones[ours]
        ours_dir = (rig.matrix_world.to_3x3() @ (b.tail_local - b.head_local)).normalized()
        sb = src.data.bones
        theirs_dir = (frame @ (sb[TIP_OF[theirs]].head_local - sb[theirs].head_local)).normalized()
        out[ours] = ours_dir.rotation_difference(theirs_dir).to_matrix()
    return out


def clear_body(rot, rest_dir, sx, chest, least):
    d = rot @ rest_dir
    local = chest.inverted() @ d
    want = math.sin(least)
    if local.x * sx >= want:
        return rot
    flat = Vector((0.0, local.y, local.z))
    if flat.length < 1e-6:
        flat = Vector((0.0, 0.0, -1.0))
    fixed = flat.normalized() * math.sqrt(1 - want * want)
    fixed.x = sx * want
    return d.rotation_difference(chest @ fixed).to_matrix() @ rot


def scaled(rot, k):
    if k == 1.0:
        return rot
    axis, ang = rot.to_quaternion().to_axis_angle()
    return Quaternion(axis, ang * k).to_matrix()


def best_loop(deltas):
    keys = ["leg.L", "leg.R", "shin.L", "shin.R", "arm.L", "arm.R"]
    n = len(deltas)
    best = None
    for i in range(int(n * 0.1), int(n * 0.6)):
        for j in range(i + 12, min(n, i + 35)):
            d = sum(
                deltas[i][b].to_quaternion().rotation_difference(deltas[j][b].to_quaternion()).angle ** 2
                for b in keys
            )
            if best is None or d < best[0]:
                best = (d, i, j)
    return best[1], best[2]


def sample(src, frame, rest):
    mw = (frame @ src.matrix_world).to_3x3()
    deltas = {}
    for ours, theirs in SOURCE_OF.items():
        cur = (mw @ src.pose.bones[theirs].matrix.to_3x3()).normalized()
        deltas[ours] = cur @ rest[theirs].inverted()
    heads = {pb.name: frame @ src.matrix_world @ pb.head for pb in src.pose.bones}
    return deltas, heads


def lowest(heads):
    return min(heads[n].z for n in ("LeftToeEnd", "RightToeEnd", "LeftFoot", "RightFoot"))


def retarget(rig, name, spec, rest_ours):
    src, made = import_bvh(spec["file"])
    f0, f1 = map(int, made[0].frame_range)
    sc = bpy.context.scene
    sc.frame_set(f0)
    rest_heads = {b.name: src.matrix_world @ b.head_local for b in src.data.bones}
    across = rest_heads["LeftLeg"] - rest_heads["RightLeg"]
    frame = Matrix.Rotation(-math.atan2(across.y, across.x), 4, "Z")
    mw = (frame @ src.matrix_world).to_3x3()
    rest = {b.name: (mw @ b.matrix_local.to_3x3()).normalized() for b in src.data.bones}
    align = rest_alignment(rig, src, frame)
    hips_rest = (frame @ rest_heads["Hips"]).z
    floor = min((frame @ rest_heads[n]).z for n in ("LeftToeEnd", "RightToeEnd"))
    stand = hips_rest - floor
    k = JOINTS["hips"][0][2] / stand

    clips = []
    for f in range(f0, f1 + 1):
        sc.frame_set(f)
        deltas, heads = sample(src, frame, rest)
        clips.append((deltas, (heads["Hips"].z - hips_rest) * k, lowest(heads) - floor, heads["Hips"].z - floor))
    bpy.data.objects.remove(src)
    for a in made:
        bpy.data.actions.remove(a)
    bpy.context.view_layer.objects.active = rig

    trim = spec.get("trim")
    if trim:
        if trim == "air":
            on = [i for i, c in enumerate(clips) if c[2] > 0.06]
        else:
            low = min(c[3] for c in clips)
            on = [i for i, c in enumerate(clips) if c[3] < low + 0.35 * (stand - low)]
        if on:
            pad = spec.get("pad", 4)
            a, b = max(0, on[0] - pad), min(len(clips), on[-1] + pad + 1)
            clips = clips[a:b]
            print(name, "trim", trim, a, b)
    if spec.get("loop"):
        i, j = best_loop([c[0] for c in clips])
        clips = clips[i:j] + [clips[i]]
        print(name, "loop", i, j)

    amp = spec.get("amp", {})
    clear = math.radians(spec.get("arm_clear", 30))
    act = bpy.data.actions.new(name)
    rig.animation_data_create()
    rig.animation_data.action = act
    bones = [b.name for b in rig.pose.bones if b.name != "root"]
    for t, (deltas, lift, _, _) in enumerate(clips):
        world = {}
        for ours in bones:
            delta = scaled(deltas[ours], amp.get(ours.split(".")[0], 1.0))
            rot = delta @ align[ours]
            if ours.startswith("arm."):
                b = rig.data.bones[ours]
                rest_dir = (b.tail_local - b.head_local).normalized()
                chest = world["spine"] @ rest_ours["spine"].inverted()
                rot = clear_body(rot, rest_dir, 1 if ours.endswith(".L") else -1, chest, clear)
            world[ours] = rot @ rest_ours[ours]
        for ours in bones:
            pb = rig.pose.bones[ours]
            parent = pb.parent.name
            pw = world.get(parent, rest_ours[parent])
            local_rest = rest_ours[parent].inverted() @ rest_ours[ours]
            pb.rotation_quaternion = (local_rest.inverted() @ pw.inverted() @ world[ours]).to_quaternion()
        root = rig.pose.bones["root"]
        root.rotation_quaternion = Quaternion()
        root.location = Vector((0, lift if spec.get("lift", True) else 0.0, 0))
        for pb in rig.pose.bones:
            pb.keyframe_insert("rotation_quaternion", frame=t + 1)
        root.keyframe_insert("location", frame=t + 1)
    act.name = name
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    track.strips.new(name, 1, act).name = name
    return len(clips)


def main():
    body = load_mesh()
    rig = build_rig(body)
    rest_ours = {b.name: b.bone.matrix_local.to_3x3().normalized() for b in rig.pose.bones}
    for name, spec in json.load(open(MOTION)).items():
        print(name, "frames", retarget(rig, name, spec, rest_ours))
    rig.animation_data.action = None
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.context.scene.render.fps = FPS
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    rig.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=OUT, export_format="GLB", use_selection=True, export_animations=True,
        export_nla_strips=True, export_force_sampling=True, export_apply=False, export_yup=True,
        export_image_format="JPEG", export_jpeg_quality=88,
    )
    print("tris", len(body.data.polygons), "bones", len(rig.pose.bones))


main()
