import bpy, sys, math, json
from mathutils import Vector, Quaternion, Matrix, Euler

argv = sys.argv[sys.argv.index("--") + 1:]
SRC, OUT, SHEETS = argv[0], argv[1], argv[2]
MOTION = argv[3] if len(argv) > 3 else None
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
    JOINTS[f"arm.{side}"] = ((sx * 0.15, 0.04, 0.44), (sx * 0.205, 0.045, 0.402), "spine")
    JOINTS[f"forearm.{side}"] = ((sx * 0.205, 0.045, 0.402), (sx * 0.32, 0.062, 0.282), f"arm.{side}")
    JOINTS[f"leg.{side}"] = ((sx * 0.065, 0, 0.25), (sx * 0.065, 0.005, 0.14), "hips")
    JOINTS[f"shin.{side}"] = ((sx * 0.065, 0.005, 0.14), (sx * 0.07, 0.03, 0.05), f"leg.{side}")
    JOINTS[f"foot.{side}"] = ((sx * 0.07, 0.03, 0.05), (sx * 0.145, -0.07, 0.02), f"shin.{side}")
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


def rot_zyx(rx, ry, rz):
    return Matrix.Rotation(rz, 3, "Z") @ Matrix.Rotation(ry, 3, "Y") @ Matrix.Rotation(rx, 3, "X")


def rot_x(a):
    return Matrix.Rotation(a, 3, "X")


def arm_rot(d, side):
    swing, down, extra = d.get(f"arm.{side}", (0, 0, 0))
    return rot_x(swing) @ Matrix.Rotation(down, 3, "Y") @ Matrix.Rotation(extra, 3, "Z")


def euler_q(name, rx, ry, rz, d=None):
    M = REST[name]
    if name.startswith("arm."):
        R = arm_rot(d, name[-1])
    elif name.startswith("forearm."):
        side = name[-1]
        body = rot_zyx(*d.get("hips", (0, 0, 0))) @ rot_zyx(*d.get("spine", (0, 0, 0)))
        wa = body @ arm_rot(d, side)
        R = wa.inverted() @ rot_x(rx) @ wa
    else:
        R = rot_zyx(rx, ry, rz)
    return (M.inverted() @ R @ M).to_quaternion()


def sin(p, k=1, off=0):
    return math.sin(k * p + off)


def cos(p):
    return math.cos(p)


def smooth(x):
    x = max(0.0, min(1.0, x))
    return x * x * (3 - 2 * x)


SIDES = (("L", 1, 0.0), ("R", -1, math.pi))


def idle(t):
    p = 2 * math.pi * t
    d = {
        "hips": (0, 0.03 * sin(p), 0),
        "spine": (0.05 + 0.03 * sin(p), -0.02 * sin(p), 0.04 * sin(p)),
        "head": (-0.03 * sin(p, 1, 0.8), 0, 0.12 * sin(p, 1, 0.6)),
    }
    for side, sg, ph in SIDES:
        d[f"arm.{side}"] = (-0.06 * sin(p, 1, 0.9 + ph), sg * (0.9 + 0.05 * sin(p, 1, 0.4)), 0)
        d[f"forearm.{side}"] = (-0.25 - 0.05 * sin(p), 0, 0)
    return d, 0.012 * sin(p)


def run(t):
    p = 2 * math.pi * t
    d = {
        "hips": (0.0, 0.07 * sin(p), 0.22 * sin(p)),
        "spine": (0.32, -0.05 * sin(p), -0.3 * sin(p)),
        "head": (-0.24, 0.04 * sin(p), 0.1 * sin(p)),
    }
    for side, sg, ph in SIDES:
        q = p + ph
        d[f"leg.{side}"] = (0.95 * sin(q), sg * 0.05, 0)
        d[f"shin.{side}"] = (0.15 + 1.45 * max(0.0, -cos(q)), 0, 0)
        d[f"foot.{side}"] = (0.35 * sin(q), 0, 0)
        d[f"arm.{side}"] = (-0.95 * sin(q), sg * 1.0, 0)
        d[f"forearm.{side}"] = (-(1.25 - 0.35 * sin(q)), 0, 0)
    return d, 0.055 * sin(p) ** 2


def jump(t):
    u = t
    s = math.sin(math.pi * u)
    out = smooth((u - 0.6) / 0.4)
    reach = smooth(min(1.0, u * 2.5))
    d = {
        "hips": (0, 0, 0),
        "spine": (0.3 * (1 - u) - 0.05 * s, 0, 0),
        "head": (-0.2 * (1 - u), 0, 0),
        "leg.L": (0.6 - 1.7 * reach + 0.7 * out, 0, 0),
        "shin.L": (0.15 + 1.25 * s, 0, 0),
        "foot.L": (0.3 * (1 - u), 0, 0),
        "leg.R": (0.35 - 1.0 * smooth(min(1.0, u * 2.2)) + 0.5 * out, 0, 0),
        "shin.R": (0.3 + 1.0 * s, 0, 0),
        "foot.R": (0.3 * (1 - u), 0, 0),
    }
    a = smooth(min(1.0, u * 3)) * (1 - 0.5 * out)
    for side, sg, ph in SIDES:
        d[f"arm.{side}"] = (-0.3 * a, sg * (0.9 - 2.3 * a), 0)
        d[f"forearm.{side}"] = (-(0.3 + 0.3 * s), 0, 0)
    return d, 0.0


def slide(t):
    k = smooth(t * 4)
    d = {"hips": (-0.5 * k, 0, 0), "spine": (0.7 * k, 0, 0), "head": (-0.5 * k, 0, 0)}
    for side, sg, ph in SIDES:
        d[f"arm.{side}"] = (-0.7 * k, sg * 0.6, 0)
        d[f"forearm.{side}"] = (-0.7 * k, 0, 0)
        d[f"leg.{side}"] = (-1.2 * k, 0, 0)
        d[f"shin.{side}"] = (0.5 * k, 0, 0)
    return d, -0.35 * k


def happy(t):
    p = 2 * math.pi * t
    beat = (1 - cos(2 * p)) / 2
    d = {
        "hips": (0, 0.05 * sin(p), 0.1 * sin(p)),
        "spine": (-0.1, 0, -0.12 * sin(p)),
        "head": (-0.12, 0.08 * sin(p), 0.15 * sin(p)),
    }
    for side, sg, ph in SIDES:
        d[f"arm.{side}"] = (0.35 * sin(2 * p, 1, ph), sg * -1.3, 0)
        d[f"forearm.{side}"] = (-0.4 + 0.5 * sin(2 * p, 1, ph + 1.0), 0, 0)
        d[f"leg.{side}"] = (-0.3 * beat, 0, 0)
        d[f"shin.{side}"] = (0.2 + 0.7 * beat, 0, 0)
    return d, 0.0


def fail(t):
    k = smooth(t * 3)
    wob = 0.06 * math.sin(t * 22) * (1 - t)
    d = {
        "spine": (-0.4 * k, wob, wob),
        "head": (0.35 * k, wob * 2, 0),
        "hips": (-0.12 * k, 0, 0),
    }
    for side, sg, ph in SIDES:
        d[f"arm.{side}"] = (-0.7 * k, sg * (0.9 - 1.9 * k), 0)
        d[f"forearm.{side}"] = (-0.3, 0, 0)
        d[f"leg.{side}"] = (-0.35 * k, 0, 0)
        d[f"shin.{side}"] = (0.5 * k, 0, 0)
    return d, -0.06 * k


CLIPS = {
    "slide": (slide, 12, False),
    "idle": (idle, 48, True),
    "run": (run, 24, True),
    "jump": (jump, 24, False),
    "happy": (happy, 36, False),
    "fail": (fail, 36, False),
}

LIMBS = {
    "arm.L": ("LeftArm", "LeftForeArm"),
    "forearm.L": ("LeftForeArm", "LeftHand"),
    "arm.R": ("RightArm", "RightForeArm"),
    "forearm.R": ("RightForeArm", "RightHand"),
    "leg.L": ("LeftLeg", "LeftShin"),
    "shin.L": ("LeftShin", "LeftFoot"),
    "foot.L": ("LeftFoot", "LeftToeEnd"),
    "leg.R": ("RightLeg", "RightShin"),
    "shin.R": ("RightShin", "RightFoot"),
    "foot.R": ("RightFoot", "RightToeEnd"),
}
TRUNK = {
    "hips": ("Hips", "Chest", "LeftLeg", "RightLeg"),
    "spine": ("Chest", "Neck1", "LeftArm", "RightArm"),
    "head": ("Head", "HeadEnd", "LeftEye", "RightEye"),
}
ORDER = ["hips", "spine", "head", "arm.L", "forearm.L", "arm.R", "forearm.R",
         "leg.L", "shin.L", "foot.L", "leg.R", "shin.R", "foot.R"]
HIP_H = JOINTS["hips"][0][2]


def rest_dir(b):
    h, t = JOINTS[b][0], JOINTS[b][1]
    return (Vector(t) - Vector(h)).normalized()


def basis(up, side):
    z = up.normalized()
    x = (side - z * side.dot(z)).normalized()
    y = z.cross(x)
    return Matrix((x, y, z)).transposed()


def import_bvh(path):
    before = set(bpy.data.objects)
    acts = set(bpy.data.actions)
    bpy.ops.import_anim.bvh(filepath=path, axis_forward="-Z", axis_up="Y", global_scale=0.01,
                            use_fps_scale=False, update_scene_fps=False, rotate_mode="NATIVE")
    src = [o for o in bpy.data.objects if o not in before][0]
    return src, [a for a in bpy.data.actions if a not in acts]


def sample(src, f, yaw):
    bpy.context.scene.frame_set(f)
    mw = src.matrix_world
    P = {pb.name: yaw @ (mw @ pb.head) for pb in src.pose.bones}
    return P


TOE_IN = math.radians(33)
PALM_IN = math.radians(80)
FWD = Vector((0, -1, 0))
UP = Vector((0, 0, 1))


def rx(a):
    return Matrix.Rotation(a, 3, "X")


def toward(b, v):
    return rest_dir(b).rotation_difference(v.normalized()).to_matrix()


def swing(v):
    return math.atan2(v.dot(FWD), -v.dot(UP))


def pose_world(P, R0, spec, foot0):
    amp = spec.get("amp", {})
    k = lambda g: amp.get(g, 1.0)
    least = math.radians(spec.get("arm_abd", 22))
    W = {}
    for b, (a, c, l, r) in TRUNK.items():
        W[b] = basis(P[c] - P[a], P[l] - P[r]) @ R0[b].inverted()
    for side, sx in (("L", 1), ("R", -1)):
        S = "Left" if side == "L" else "Right"
        chest = W["spine"]
        u = (chest.inverted() @ (P[f"{S}ForeArm"] - P[f"{S}Arm"])).normalized()
        f = (chest.inverted() @ (P[f"{S}Hand"] - P[f"{S}ForeArm"])).normalized()
        pitch = swing(u) * k("arm")
        bend = u.angle(f) * k("forearm")
        abd = min(max(math.asin(max(-1.0, min(1.0, u.x * sx))), least), 1.45)
        hang = Vector((sx * math.sin(abd), 0, -math.cos(abd)))
        fabd = max(abd - 0.12, 0.2)
        fhang = Vector((sx * math.sin(fabd), 0, -math.cos(fabd)))
        palm = Matrix.Rotation(sx * PALM_IN, 3, rest_dir(f"forearm.{side}"))
        W[f"arm.{side}"] = chest @ rx(-pitch) @ toward(f"arm.{side}", hang)
        W[f"forearm.{side}"] = chest @ rx(-(pitch + bend)) @ toward(f"forearm.{side}", fhang) @ palm
        hips = W["hips"]
        t = (hips.inverted() @ (P[f"{S}Shin"] - P[f"{S}Leg"])).normalized()
        sh = (hips.inverted() @ (P[f"{S}Foot"] - P[f"{S}Shin"])).normalized()
        ft = (hips.inverted() @ (P[f"{S}ToeEnd"] - P[f"{S}Foot"])).normalized()
        thigh = swing(t) * k("leg")
        knee = t.angle(sh) * k("shin")
        toe = (math.atan2(ft.dot(UP), ft.dot(FWD)) - foot0[side]) * k("foot")
        twist = Matrix.Rotation(-sx * TOE_IN, 3, "Z")
        down = Vector((0, 0, -1))
        W[f"leg.{side}"] = hips @ rx(-thigh) @ toward(f"leg.{side}", down) @ twist
        W[f"shin.{side}"] = hips @ rx(-(thigh - knee)) @ toward(f"shin.{side}", down) @ twist
        W[f"foot.{side}"] = hips @ rx(-toe) @ twist
    return W


def features(W):
    out = []
    for b in LIMBS:
        out.extend(W[b] @ rest_dir(b))
    return out


def best_loop(feats, lo, hi):
    n = len(feats)
    best = None
    for i in range(int(n * 0.1), int(n * 0.6)):
        for p in range(lo, hi + 1):
            j = i + p
            if j >= n:
                break
            d = sum((x - y) ** 2 for x, y in zip(feats[i], feats[j]))
            if best is None or d < best[0]:
                best = (d, i, j)
    return best


def retarget(name, spec):
    src, made = import_bvh(spec["file"])
    f0, f1 = map(int, made[0].frame_range)
    P0 = sample(src, f0, Matrix.Identity(4))
    side = P0["LeftLeg"] - P0["RightLeg"]
    yaw_ang = math.atan2(side.y, side.x)
    yaw = Matrix.Rotation(-yaw_ang + math.radians(spec.get("yaw", 0)), 4, "Z")
    up = P0["Head"] - P0["Hips"]
    print(name, "src up", tuple(round(v, 2) for v in up.normalized()))
    stand_h = (P0["Hips"].z - min(P0["LeftToeEnd"].z, P0["RightToeEnd"].z))
    k = HIP_H / max(stand_h, 1e-3)
    ground = min(P0["LeftToeEnd"].z, P0["RightToeEnd"].z)
    mw = yaw @ src.matrix_world
    RP = {b.name: mw @ b.head_local for b in src.data.bones}
    R0 = {b: basis(RP[c] - RP[a], RP[l] - RP[r]) for b, (a, c, l, r) in TRUNK.items()}
    foot0 = {}
    for side, S in (("L", "Left"), ("R", "Right")):
        ft0 = RP[f"{S}ToeEnd"] - RP[f"{S}Foot"]
        foot0[side] = math.atan2(ft0.dot(UP), ft0.dot(FWD))
    frames = []
    feet = []
    hips = []
    for f in range(f0, f1 + 1):
        P = sample(src, f, yaw)
        frames.append((pose_world(P, R0, spec, foot0), (P["Hips"].z - ground - stand_h) * k))
        feet.append(min(P[j].z for j in ("LeftToeEnd", "RightToeEnd", "LeftFoot", "RightFoot")) - ground)
        hips.append(P["Hips"].z - ground)
    trim = spec.get("trim")
    if trim:
        if trim == "air":
            on = [i for i, h in enumerate(feet) if h > 0.06]
        else:
            low = min(hips)
            on = [i for i, h in enumerate(hips) if h < low + 0.35 * (stand_h - low)]
        if on:
            pad = spec.get("pad", 4)
            a, b = max(0, on[0] - pad), min(len(frames), on[-1] + pad + 1)
            print(name, "trim", trim, a, b, "of", len(frames))
            frames = frames[a:b]
    lo, hi = int(spec.get("from", 0) * len(frames)), int(spec.get("to", 1) * len(frames))
    if spec.get("loop"):
        d, i, j = best_loop([features(w) for w, _ in frames], 12, 34)
        print(name, "loop", i, j, round(d, 3))
        frames = frames[i:j] + [frames[i]]
    else:
        frames = frames[lo:hi]
    act = bpy.data.actions.new(name)
    rig.animation_data_create()
    rig.animation_data.action = act
    for t, (W, lift) in enumerate(frames):
        for pb in rig.pose.bones:
            pb.rotation_quaternion = Quaternion((1, 0, 0, 0))
            pb.location = (0, 0, 0)
        for b in ORDER:
            pb = rig.pose.bones[b]
            pname = pb.parent.name
            Mp, Mb = REST[pname], REST[b]
            Ap = W[pname] @ Mp if pname in W else Mp
            Ab = W[b] @ Mb
            q = ((Mp.inverted() @ Mb).inverted() @ Ap.inverted() @ Ab).to_quaternion()
            if b in TRUNK:
                lean = spec.get("amp", {}).get(b, 1.0)
                sway = spec.get("sway", 0.5)
                e = q.to_euler("XYZ")
                q = Euler((e.x * lean, e.y * sway, e.z * sway), "XYZ").to_quaternion()
            pb.rotation_quaternion = q
        rig.pose.bones["root"].location = Vector((0, lift if spec.get("lift", True) else 0.0, 0))
        for pb in rig.pose.bones:
            pb.keyframe_insert("rotation_quaternion", frame=t + 1)
            if pb.name == "root":
                pb.keyframe_insert("location", frame=t + 1)
    bpy.data.objects.remove(src)
    for a in made:
        bpy.data.actions.remove(a)
    act.name = name
    bpy.context.view_layer.objects.active = rig
    return len(frames)


if MOTION:
    specs = json.load(open(MOTION))
    CLIPS = {n: (None, s.get("frames", 8), s.get("loop", False)) for n, s in specs.items()}
    for name, spec in specs.items():
        n = retarget(name, spec)
        CLIPS[name] = (None, n - 1, spec.get("loop", False))
        track = rig.animation_data.nla_tracks.new()
        track.name = name
        strip = track.strips.new(name, 1, bpy.data.actions[name])
        strip.name = name
else:
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
                rig.pose.bones[bone].rotation_quaternion = euler_q(bone, rx, ry, rz, d)
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

bpy.context.scene.render.fps = 30 if MOTION else FPS
bpy.ops.object.select_all(action="DESELECT")
body.select_set(True)
rig.select_set(True)
bpy.ops.export_scene.gltf(
    filepath=OUT, export_format="GLB", use_selection=True, export_animations=True,
    export_nla_strips=True, export_force_sampling=True, export_apply=False, export_yup=True,
    export_image_format="JPEG", export_jpeg_quality=88,
)

sc = bpy.context.scene
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
        elev = 1.6 if vname == "game" else 0.6
        cam.location = Vector((math.sin(a) * 4, -math.cos(a) * 4, elev))
        cam.rotation_euler = Vector(Vector((0, 0, 0.6)) - cam.location).to_track_quat("-Z", "Y").to_euler()
        for k, f in enumerate(frames):
            sc.frame_set(f)
            sc.render.filepath = f"{SHEETS}/{name}_{vname}_{k}.png"
            bpy.ops.render.render(write_still=True)


for clip in [c for c in ("run", "idle", "jump", "happy", "fail", "slide") if c in bpy.data.actions]:
    render_clip(clip, (("game", 180), ("back", 180), ("side", 90)))
print("done")
