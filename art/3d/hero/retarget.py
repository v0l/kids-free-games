import json
import math
import re
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]

SOURCE_OF = {
    "hips_b": ("Hips", "Spine1"),
    "chest": ("Chest", "Neck1"),
    "head_b": ("Head", "HeadEnd"),
    "arm_l": ("LeftArm", "LeftForeArm"),
    "forearm_l": ("LeftForeArm", "LeftHand"),
    "hand_b_l": ("LeftHand", "LeftHandMiddle4"),
    "arm_r": ("RightArm", "RightForeArm"),
    "forearm_r": ("RightForeArm", "RightHand"),
    "hand_b_r": ("RightHand", "RightHandMiddle4"),
    "thigh_l": ("LeftLeg", "LeftShin"),
    "shin_l": ("LeftShin", "LeftFoot"),
    "foot_l": ("LeftFoot", "LeftToeBase"),
    "thigh_r": ("RightLeg", "RightShin"),
    "shin_r": ("RightShin", "RightFoot"),
    "foot_r": ("RightFoot", "RightToeBase"),
}

BVH_TO_GCAD = np.array([[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]])


def rot_axis(axis, deg):
    a = np.asarray(axis, float)
    a = a / np.linalg.norm(a)
    t = math.radians(deg)
    k = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + math.sin(t) * k + (1 - math.cos(t)) * k @ k


def between(u, v):
    u = u / np.linalg.norm(u)
    v = v / np.linalg.norm(v)
    c = float(np.dot(u, v))
    axis = np.cross(u, v)
    s = np.linalg.norm(axis)
    if s < 1e-9:
        if c > 0:
            return np.eye(3)
        perp = np.cross(u, [1, 0, 0])
        if np.linalg.norm(perp) < 1e-6:
            perp = np.cross(u, [0, 1, 0])
        return rot_axis(perp, 180)
    return rot_axis(axis, math.degrees(math.atan2(s, c)))


def axis_angle(r):
    c = max(-1.0, min(1.0, (np.trace(r) - 1) / 2))
    ang = math.acos(c)
    if ang < 1e-6:
        return 0.0, (1.0, 0.0, 0.0)
    if math.pi - ang < 1e-4:
        w, v = np.linalg.eigh((r + r.T) / 2)
        axis = v[:, np.argmax(w)]
    else:
        axis = np.array([r[2, 1] - r[1, 2], r[0, 2] - r[2, 0], r[1, 0] - r[0, 1]])
    axis = axis / np.linalg.norm(axis)
    return math.degrees(ang), tuple(axis)


def scaled(r, k):
    ang, axis = axis_angle(r)
    return rot_axis(axis, ang * k) if ang else np.eye(3)


class Bvh:
    def __init__(self, path):
        text = Path(path).read_text().split()
        self.names, self.parent, self.offset, self.channels = [], [], [], []
        stack, i = [], 0
        while text[i] != "MOTION":
            tok = text[i]
            if tok in ("ROOT", "JOINT"):
                self.names.append(text[i + 1])
                self.parent.append(stack[-1] if stack else None)
                i += 2
            elif tok == "End":
                self.names.append(f"_end{len(self.names)}")
                self.parent.append(stack[-1])
                self.channels.append([])
                i += 2
            elif tok == "{":
                stack.append(len(self.names) - 1)
                i += 1
            elif tok == "}":
                stack.pop()
                i += 1
            elif tok == "OFFSET":
                self.offset.append(np.array([float(x) for x in text[i + 1:i + 4]]))
                i += 4
            elif tok == "CHANNELS":
                n = int(text[i + 1])
                self.channels.append(text[i + 2:i + 2 + n])
                i += 2 + n
            else:
                i += 1
        frames = int(text[i + 2])
        self.dt = float(text[i + 5])
        values = np.array([float(x) for x in text[i + 6:]]).reshape(frames, -1)
        self.frames = values
        self.index = {n: k for k, n in enumerate(self.names)}
        self.rest = []
        for j in range(len(self.names)):
            p = self.parent[j]
            self.rest.append(self.offset[j] + (self.rest[p] if p is not None else 0))

    def pose(self, f):
        row = self.frames[f]
        col = 0
        rots, pos = [], []
        for j in range(len(self.names)):
            local = np.eye(3)
            off = self.offset[j].copy()
            for ch in self.channels[j]:
                v = row[col]
                col += 1
                if ch.endswith("position"):
                    off["XYZ".index(ch[0])] = v
                else:
                    local = local @ rot_axis(np.eye(3)["XYZ".index(ch[0])], v)
            p = self.parent[j]
            if p is None:
                rots.append(local)
                pos.append(off)
            else:
                rots.append(rots[p] @ local)
                pos.append(pos[p] + rots[p] @ off)
        g = BVH_TO_GCAD
        return [g @ r @ g.T for r in rots], [g @ p for p in pos]

    def rest_pos(self, name):
        return BVH_TO_GCAD @ self.rest[self.index[name]]

    def tip(self, a, b):
        return self.rest_pos(b) - self.rest_pos(a)


def read_rig(path):
    bones = {}
    order = []
    for line in Path(path).read_text().splitlines():
        m = re.match(r"bone (\w+)(.*)", line)
        if m:
            args = dict(re.findall(r"(\w+)=(\S+)", m.group(2)))
            name = m.group(1)
            parent = args.get("parent")
            head = (
                np.array([float(x) for x in args["head"].split(",")])
                if "head" in args
                else bones[parent]["tail"]
            )
            tail = np.array([float(x) for x in args["tail"].split(",")])
            bones[name] = {"parent": parent, "head": head, "tail": tail}
            order.append(name)
        m = re.match(r"mirror x of=(\w+)", line)
        if m and m.group(1) in bones:
            root = m.group(1)
            group = [root]
            for n in order:
                if bones[n]["parent"] in group:
                    group.append(n)
            for n in group:
                b = bones[n]
                flip = np.array([-1.0, 1.0, 1.0])
                mn = n.replace("_l", "_r")
                bones[mn] = {
                    "parent": b["parent"].replace("_l", "_r") if b["parent"] in group else b["parent"],
                    "head": b["head"] * flip,
                    "tail": b["tail"] * flip,
                }
            order.extend(n.replace("_l", "_r") for n in group)
    return bones, order


def fk(bones, order, local):
    world_r, world_t = {}, {}
    for n in order:
        b = bones[n]
        p = b["parent"]
        if p is None:
            world_r[n] = local[n]
            world_t[n] = b["head"] + local.get("_lift", np.zeros(3))
        else:
            world_r[n] = world_r[p] @ local[n]
            world_t[n] = world_t[p] + world_r[p] @ (b["head"] - bones[p]["head"])
    return world_r, world_t


def point(bones, wr, wt, bone, at):
    b = bones[bone]
    return wt[bone] + wr[bone] @ (at - b["head"])


CONTACTS = [
    ("foot_l", "tail", 1.0), ("foot_r", "tail", 1.0),
    ("shin_l", "tail", 2.4), ("shin_r", "tail", 2.4),
    ("hips_b", "head", 3.4), ("chest", "tail", 4.0),
    ("head_b", "mid", 6.6), ("hand_b_l", "tail", 1.2), ("hand_b_r", "tail", 1.2),
    ("forearm_l", "head", 1.6), ("forearm_r", "head", 1.6),
    ("shin_l", "head", 2.0), ("shin_r", "head", 2.0),
]


def lowest(bones, wr, wt):
    out = math.inf
    for bone, end, pad in CONTACTS:
        b = bones[bone]
        at = b["tail"] if end == "tail" else b["head"] if end == "head" else (b["head"] + b["tail"]) / 2
        out = min(out, point(bones, wr, wt, bone, at)[2] - pad)
    return out


def clear_arm(r_arm, rest_dir, chest_r, side, least):
    d = chest_r.T @ (r_arm @ rest_dir)
    want = math.sin(math.radians(least))
    if d[0] * side >= want:
        return r_arm
    flat = np.array([0.0, d[1], d[2]])
    if np.linalg.norm(flat) < 1e-6:
        flat = np.array([0.0, 0.0, -1.0])
    fixed = flat / np.linalg.norm(flat) * math.sqrt(1 - want * want)
    fixed[0] = side * want
    return chest_r @ between(d, fixed) @ chest_r.T @ r_arm


def best_loop(poses, lo, hi, span):
    keys = ["thigh_l", "thigh_r", "shin_l", "shin_r", "arm_l", "arm_r", "hips_b"]
    best = None
    n = len(poses)
    for i in range(int(n * lo), int(n * hi)):
        for j in range(i + span[0], min(n, i + span[1])):
            d = sum(axis_angle(poses[i][b].T @ poses[j][b])[0] ** 2 for b in keys)
            if best is None or d < best[0]:
                best = (d, i, j)
    return best[1], best[2]


def retarget(name, spec, bones, order):
    src = Bvh(ROOT / spec["file"])
    rest_r = {b: np.eye(3) for b in order}
    align = {}
    for b in order:
        a, tip = SOURCE_OF[b]
        ours = bones[b]["tail"] - bones[b]["head"]
        align[b] = (ours, src.tip(a, tip))
    floor = min(src.rest_pos(n)[2] for n in ("LeftToeEnd", "RightToeEnd"))
    leg = src.rest_pos("LeftLeg")[2] - floor
    k = bones["thigh_l"]["head"][2] / leg
    yaw = None
    frames = []
    for f in range(len(src.frames)):
        rots, pos = src.pose(f)
        if yaw is None:
            across = pos[src.index["LeftLeg"]] - pos[src.index["RightLeg"]]
            yaw = rot_axis([0, 0, 1], -math.degrees(math.atan2(across[1], across[0])))
            fitted = {b: between(o, yaw @ s) for b, (o, s) in align.items()}
        world = {}
        for b in order:
            a, _ = SOURCE_OF[b]
            world[b] = yaw @ rots[src.index[a]] @ yaw.T @ fitted[b]
        amp = spec.get("amp", {})
        for b, s in amp.items():
            p = bones[b]["parent"]
            base = world[p] if p else np.eye(3)
            world[b] = base @ scaled(base.T @ world[b], s)
        for side, arm in ((1, "arm_l"), (-1, "arm_r")):
            rest_dir = bones[arm]["tail"] - bones[arm]["head"]
            rest_dir = rest_dir / np.linalg.norm(rest_dir)
            world[arm] = clear_arm(world[arm], rest_dir, world["chest"], side, spec.get("arm_clear", 20))
        local = {}
        for b in order:
            p = bones[b]["parent"]
            local[b] = world[b] if p is None else world[p].T @ world[b]
        src_low = min(p[2] for p in pos) - floor
        frames.append((local, max(0.0, src_low) * k))
    trim = spec.get("trim")
    if trim:
        lo, hi = trim
        frames = frames[lo:hi]
    if spec.get("loop"):
        i, j = best_loop([f[0] for f in frames], *spec.get("search", (0.1, 0.7)), spec.get("span", (12, 35)))
        frames = frames[i:j] + [frames[i]]
        print(name, "loop", i, j, file=sys.stderr)
    lift_mode = spec.get("lift", "ground")
    out = [f"clip {name} fps=30"]
    for t, (local, target) in enumerate(frames):
        wr, wt = fk(bones, order, local)
        low = lowest(bones, wr, wt)
        if lift_mode == "ground":
            lift = target - low
        elif lift_mode == "plant":
            lift = -low
        else:
            lift = 0.0
        time = f"{t / 30:.4f}".rstrip("0").rstrip(".")
        for b in order:
            ang, axis = axis_angle(local[b])
            ax = ",".join(f"{v:.4f}" for v in axis)
            out.append(f"key t={time} bone={b} rotate={ang:.3f} axis={ax}")
        out.append(f"key t={time} bone=hips_b move=0,0,{lift:.3f}")
    print(name, "frames", len(frames), file=sys.stderr)
    return out


def main():
    spec = json.loads((HERE / "clips.json").read_text())
    bones, order = read_rig(HERE / "hero.gcad")
    lines = []
    for name in sorted(spec):
        lines += retarget(name, spec[name], bones, order)
    (HERE / "motion.gcad").write_text("\n".join(lines) + "\n")


main()
