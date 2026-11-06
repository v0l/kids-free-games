import json
import struct
import sys

src, dst = sys.argv[1], sys.argv[2]
d = open(src, "rb").read()
n = struct.unpack("<I", d[12:16])[0]
j = json.loads(d[20:20 + n])
rest = d[20 + n:]
positions = [p["attributes"]["POSITION"] for m in j["meshes"] for p in m["primitives"]]
lo = [min(j["accessors"][a]["min"][k] for a in positions) for k in range(3)]
hi = [max(j["accessors"][a]["max"][k] for a in positions) for k in range(3)]
k = 0.01 if "metres" in sys.argv[3:] else 1.0 / (hi[1] - lo[1])
centre = "centre" in sys.argv[3:]
floor = -lo[1] * k
offset = [-(lo[0] + hi[0]) / 2 * k, floor, -(lo[2] + hi[2]) / 2 * k] if centre else [0.0, floor, 0.0]
roots = j["scenes"][0]["nodes"]
j["nodes"].append({"name": "model", "children": roots, "scale": [k, k, k], "translation": offset})
j["scenes"][0]["nodes"] = [len(j["nodes"]) - 1]
js = json.dumps(j, separators=(",", ":")).encode()
js += b" " * (-len(js) % 4)
body = struct.pack("<II", len(js), 0x4E4F534A) + js + rest
open(dst, "wb").write(b"glTF" + struct.pack("<II", 2, 12 + len(body)) + body)
