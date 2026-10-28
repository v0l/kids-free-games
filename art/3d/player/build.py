import bpy, sys, math, json
from mathutils import Vector, Quaternion, Matrix

argv = sys.argv[sys.argv.index("--") + 1:]
SRC, OUT, SHEETS = argv[0], argv[1], argv[2]
TRIS = 5000
FPS = 24

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=SRC)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
for o in bpy.context.scene.objects:
    o.select_set(o.type == "MESH")
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
ratio = min(1.0, TRIS / max(1, len(body.data.polygons)))
dec = body.modifiers.new("dec", "DECIMATE")
dec.ratio = ratio
bpy.ops.object.modifier_apply(modifier="dec")

ys = [v.co.z for v in body.data.vertices]
height = max(ys) - min(ys)
s = 1.0 / height
body.scale = (s, s, s)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
low = min(v.co.z for v in body.data.vertices)
cx = sum(v.co.x for v in body.data.vertices) / len(body.data.vertices)
cy = sum(v.co.y for v in body.data.vertices) / len(body.data.vertices)
body.location = (-cx, -cy, -low)
bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
for p in body.data.polygons:
    p.use_smooth = True

arm_data = bpy.data.armatures.new("rig")
rig = bpy.data.objects.new("rig", arm_data)
bpy.context.scene.collection.objects.link(rig)
bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="EDIT")

JOINTS = {
    "root": ((0, 0, 0.0), (0, 0, 0.1), None),
    "hips": ((0, 0, 0.26), (0, 0, 0.36), "root"),
    "spine": ((0, 0, 0.36), (0, 0, 0.56), "hips"),
    "head": ((0, 0, 0.56), (0, 0, 0.98), "spine"),
}
for side, sx in (("L", 1), ("R", -1)):
    JOINTS[f"arm.{side}"] = ((sx * 0.13, 0, 0.43), (sx * 0.26, 0, 0.38), "spine")
    JOINTS[f"forearm.{side}"] = ((sx * 0.26, 0, 0.38), (sx * 0.36, 0, 0.33), f"arm.{side}")
    JOINTS[f"leg.{side}"] = ((sx * 0.08, 0, 0.26), (sx * 0.08, 0, 0.14), "hips")
    JOINTS[f"shin.{side}"] = ((sx * 0.08, 0, 0.14), (sx * 0.08, 0, 0.04), f"leg.{side}")
    JOINTS[f"foot.{side}"] = ((sx * 0.08, 0, 0.04), (sx * 0.08, -0.08, 0.02), f"shin.{side}")
for name, (head, tail, parent) in JOINTS.items():
    b = arm_data.edit_bones.new(name)
    b.head, b.tail = Vector(head), Vector(tail)
    if parent:
        b.parent = arm_data.edit_bones[parent]
        b.use_connect = Vector(head) == arm_data.edit_bones[parent].tail
bpy.ops.object.mode_set(mode="OBJECT")

body.select_set(True)
rig.select_set(True)
bpy.context.view_layer.objects.active = rig
body.select_set(True)
bpy.ops.object.parent_set(type="ARMATURE_AUTO")
groups = {g.name for g in body.vertex_groups}
unweighted = sum(1 for v in body.data.vertices if not any(g.weight > 0 for g in v.groups))
print("groups", len(groups), "unweighted", unweighted)
if unweighted or len(groups) < len(JOINTS) - 1:
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    body.vertex_groups.clear()
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.parent_set(type="ARMATURE_ENVELOPE")

bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")
for pb in rig.pose.bones:
    pb.rotation_mode = "QUATERNION"

REST = {b.name: b.bone.matrix_local.to_3x3() for b in rig.pose.bones}


def euler_q(name, rx, ry, rz):
    R = Matrix.Rotation(rz, 3, "Z") @ Matrix.Rotation(ry, 3, "Y") @ Matrix.Rotation(rx, 3, "X")
    M = REST[name]
    return (M.inverted() @ R @ M).to_quaternion()


def sin(p, k=1, off=0):
    return math.sin(k * p + off)


def idle(t):
    p = 2 * math.pi * t
    d = {"spine": (0.03 * sin(p), 0, 0), "head": (-0.02 * sin(p, 1, 0.8), 0.03 * sin(p), 0)}
    for side, sg in (("L", 1), ("R", -1)):
        d[f"arm.{side}"] = (0.04 * sin(p, 1, 0.5), sg * (0.75 + 0.03 * sin(p)), 0)
        d[f"forearm.{side}"] = (-0.15, sg * 0.1, 0)
    return d, 0.006 * sin(p, 2)


def run(t):
    p = 2 * math.pi * t
    d = {"spine": (0.18, 0.1 * sin(p), 0), "head": (-0.1, -0.06 * sin(p), 0), "hips": (0, 0.1 * sin(p), 0)}
    for side, sg, ph in (("L", 1, 0.0), ("R", -1, math.pi)):
        q = p + ph
        d[f"leg.{side}"] = (0.75 * sin(q), 0, 0)
        d[f"shin.{side}"] = (0.2 + 0.95 * max(0.0, sin(q, 1, 1.9)), 0, 0)
        d[f"foot.{side}"] = (-0.2 * sin(q), 0, 0)
        d[f"arm.{side}"] = (0, sg * 0.85, sg * 0.9 * sin(q))
        d[f"forearm.{side}"] = (0, sg * 0.1, sg * (0.7 + 0.5 * sin(q)))
    return d, 0.035 * math.cos(2 * p) - 0.01


def jump(t):
    crouch = max(0.0, 1 - abs(t - 0.12) / 0.12) if t < 0.24 else 0.0
    land = max(0.0, 1 - abs(t - 0.9) / 0.1) if t > 0.8 else 0.0
    air = 1.0 if 0.24 <= t <= 0.8 else 0.0
    k = crouch + land * 0.8
    d = {"spine": (0.25 * k - 0.05 * air, 0, 0), "head": (-0.1 * k, 0, 0)}
    for side, sg in (("L", 1), ("R", -1)):
        d[f"leg.{side}"] = (-0.6 * k - 0.7 * air * (0.5 if side == "L" else 1.0), 0, 0)
        d[f"shin.{side}"] = (1.0 * k + 1.1 * air * (0.5 if side == "L" else 1.0), 0, 0)
        d[f"arm.{side}"] = (0.5 * k - 0.4 * air, sg * (0.7 - 1.1 * air), 0)
        d[f"forearm.{side}"] = (-0.3, 0, 0)
    return d, -0.07 * k


def happy(t):
    p = 2 * math.pi * t
    h = abs(math.sin(p))
    d = {"spine": (-0.05, 0.06 * sin(p), 0), "head": (-0.12, 0.1 * sin(p), 0)}
    for side, sg in (("L", 1), ("R", -1)):
        d[f"arm.{side}"] = (-0.1 * sin(p), sg * (-0.9 + 0.2 * sin(p, 2)), 0)
        d[f"forearm.{side}"] = (0, sg * -0.25, 0)
        d[f"leg.{side}"] = (-0.25 * h, 0, 0)
        d[f"shin.{side}"] = (0.5 * h, 0, 0)
    return d, 0.2 * h


def fail(t):
    k = min(1.0, t * 3)
    wob = 0.04 * math.sin(t * 20) * (1 - t)
    d = {"spine": (-0.35 * k, wob, 0), "head": (0.3 * k, wob * 2, 0), "hips": (-0.1 * k, 0, 0)}
    for side, sg in (("L", 1), ("R", -1)):
        d[f"arm.{side}"] = (0.2 * k, sg * (-0.5 * k + 0.75 * (1 - k)), 0)
        d[f"forearm.{side}"] = (-0.3, 0, 0)
        d[f"leg.{side}"] = (-0.3 * k, 0, 0)
        d[f"shin.{side}"] = (0.4 * k, 0, 0)
    return d, -0.05 * k


def slide(t):
    k = min(1.0, t * 4)
    d = {"hips": (-0.5 * k, 0, 0), "spine": (0.7 * k, 0, 0), "head": (-0.5 * k, 0, 0)}
    for side, sg in (("L", 1), ("R", -1)):
        d[f"arm.{side}"] = (0.0, sg * 0.5, sg * 0.5 * k)
        d[f"forearm.{side}"] = (0, 0, sg * 0.5)
        d[f"leg.{side}"] = (-1.2 * k, 0, 0)
        d[f"shin.{side}"] = (0.5 * k, 0, 0)
    return d, -0.35 * k


CLIPS = {
    "slide": (slide, 12, False),
    "idle": (idle, 48, True),
    "run": (run, 24, True),
    "jump": (jump, 30, False),
    "happy": (happy, 36, False),
    "fail": (fail, 36, False),
}

for name, (fn, n, loop) in CLIPS.items():
    act = bpy.data.actions.new(name)
    rig.animation_data_create()
    rig.animation_data.action = act
    count = n + 1
    for i in range(count):
        d, z = fn(i / n)
        for pb in rig.pose.bones:
            pb.rotation_quaternion = Quaternion((1, 0, 0, 0))
            pb.location = (0, 0, 0)
        for bone, (rx, ry, rz) in d.items():
            rig.pose.bones[bone].rotation_quaternion = euler_q(bone, rx, ry, rz)
        rig.pose.bones["root"].location = Vector((0, z, 0))
        for pb in rig.pose.bones:
            pb.keyframe_insert("rotation_quaternion", frame=i + 1)
            if pb.name == "root":
                pb.keyframe_insert("location", frame=i + 1)
    if hasattr(act, "use_cyclic"):
        pass
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, 1, act)
    strip.name = name
rig.animation_data.action = None
bpy.ops.object.mode_set(mode="OBJECT")

for img in bpy.data.images:
    if img.size[0] > 512:
        img.scale(512, 512)
mat_ok = [m.name for m in body.data.materials]
print("materials", mat_ok, "tris", len(body.data.polygons), "bones", len(rig.pose.bones))

bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
rig.select_set(True)
bpy.ops.export_scene.gltf(
    filepath=OUT, export_format="GLB", use_selection=True, export_animations=True,
    export_nla_strips=True, export_force_sampling=True, export_apply=False, export_yup=True,
    export_image_format="JPEG", export_jpeg_quality=88,
)

sc = bpy.context.scene
sc.render.fps = FPS
sc.render.engine = "BLENDER_WORKBENCH"
sc.display.shading.light = "FLAT"
sc.display.shading.color_type = "TEXTURE"
sc.view_settings.view_transform = "Standard"
sc.render.film_transparent = True
sc.render.resolution_x = sc.render.resolution_y = 384
cam = bpy.data.objects.new("c", bpy.data.cameras.new("c"))
sc.collection.objects.link(cam)
sc.camera = cam
cam.data.type = "ORTHO"
cam.data.ortho_scale = 1.9
sc.world = bpy.data.worlds.new("w")

for track in rig.animation_data.nla_tracks:
    track.mute = True


def render_clip(name, views):
    act = bpy.data.actions[name]
    rig.animation_data.action = act
    n = CLIPS[name][1]
    frames = [1 + int(i * n / 8) for i in range(8)]
    for vname, ang in views:
        a = math.radians(ang)
        cam.location = Vector((math.sin(a) * 4, -math.cos(a) * 4, 0.6))
        cam.rotation_euler = Vector(Vector((0, 0, 0.6)) - cam.location).to_track_quat("-Z", "Y").to_euler()
        for k, f in enumerate(frames):
            sc.frame_set(f)
            sc.render.filepath = f"{SHEETS}/{name}_{vname}_{k}.png"
            bpy.ops.render.render(write_still=True)


for clip in ("run", "idle", "jump", "happy", "fail", "slide"):
    render_clip(clip, (("front", 0), ("back", 180), ("side", 90)))
print("done")
