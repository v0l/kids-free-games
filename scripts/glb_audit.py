#!/usr/bin/env python3
import json
import struct
import sys

MAX_TRIS = int(sys.argv[2]) if len(sys.argv) > 2 else 6000
MAX_BONES = 64
MAX_TEX = 1024


def load(path):
    data = open(path, "rb").read()
    magic, version, _ = struct.unpack("<4sII", data[:12])
    if magic != b"glTF" or version != 2:
        sys.exit("not a glb v2")
    off = 12
    doc = None
    while off < len(data):
        n, kind = struct.unpack("<I4s", data[off:off + 8])
        if kind == b"JSON":
            doc = json.loads(data[off + 8:off + 8 + n])
        off += 8 + n
    if doc is None:
        sys.exit("no JSON chunk")
    return doc


def audit(doc):
    errors = []
    tris = 0
    lo = [1e9] * 3
    hi = [-1e9] * 3
    for mesh in doc.get("meshes", []):
        for prim in mesh["primitives"]:
            attrs = prim["attributes"]
            for need in ("POSITION", "NORMAL", "TEXCOORD_0"):
                if need not in attrs:
                    errors.append(f"mesh {mesh.get('name')} missing {need}")
            if "indices" in prim:
                tris += doc["accessors"][prim["indices"]]["count"] // 3
            pos = doc["accessors"][attrs["POSITION"]]
            for i in range(3):
                lo[i] = min(lo[i], pos["min"][i])
                hi[i] = max(hi[i], pos["max"][i])
    if tris > MAX_TRIS:
        errors.append(f"{tris} tris over budget {MAX_TRIS}")
    for skin in doc.get("skins", []):
        if len(skin["joints"]) > MAX_BONES:
            errors.append(f"{len(skin['joints'])} bones over {MAX_BONES}")
    for i, anim in enumerate(doc.get("animations", [])):
        if not anim.get("name"):
            errors.append(f"animation {i} unnamed")
    if hi[1] - lo[1] > 0 and abs(lo[1]) > 0.05 * (hi[1] - lo[1]):
        errors.append(f"feet not at origin: min y {lo[1]:.3f}")
    for img in doc.get("images", []):
        if "bufferView" not in img and "uri" not in img:
            errors.append("image without data")
    print(
        f"tris={tris} bones={[len(s['joints']) for s in doc.get('skins', [])]} "
        f"clips={[a.get('name') for a in doc.get('animations', [])]} "
        f"size={[round(hi[i] - lo[i], 3) for i in range(3)]} "
        f"textures={len(doc.get('images', []))} (max {MAX_TEX}px, check visually)"
    )
    return errors


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit("usage: glb_audit.py file.glb [max_tris]")
    errs = audit(load(sys.argv[1]))
    for e in errs:
        print("FAIL", e)
    sys.exit(1 if errs else 0)
