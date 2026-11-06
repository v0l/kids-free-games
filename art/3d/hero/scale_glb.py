import json, struct, sys

src, dst, height = sys.argv[1], sys.argv[2], float(sys.argv[3])
d = open(src, "rb").read()
n = struct.unpack("<I", d[12:16])[0]
j = json.loads(d[20:20 + n])
rest = d[20 + n:]
k = 1.0 / height
roots = j["scenes"][0]["nodes"]
j["nodes"].append({"name": "hero", "children": roots, "scale": [k, k, k]})
j["scenes"][0]["nodes"] = [len(j["nodes"]) - 1]
js = json.dumps(j, separators=(",", ":")).encode()
js += b" " * (-len(js) % 4)
body = d[12:12] + struct.pack("<II", len(js), 0x4E4F534A) + js + rest
open(dst, "wb").write(b"glTF" + struct.pack("<II", 2, 12 + len(body)) + body)
