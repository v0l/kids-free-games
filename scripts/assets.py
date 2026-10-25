#!/usr/bin/env python3
import io
import os
import re
import struct
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
CACHE = Path(os.environ.get("KENNEY_CACHE", Path.home() / ".cache/kids-games/kenney"))
MUSIC_CACHE = Path(os.environ.get("MUSIC_CACHE", Path.home() / ".cache/kids-games/music"))
ART_IN = ROOT / "art"
ART_OUT = ROOT / "assets/art"
SND_IN = ROOT / "sound.txt"
SND_OUT = ROOT / "assets/sound"
PAGE = 2048
PAD = 2
RATE = 44100


def pack_dir(pack):
    d = CACHE / pack
    if d.exists():
        return d
    page = urllib.request.urlopen(f"https://kenney.nl/assets/{pack}").read().decode()
    m = re.search(rf"https://kenney.nl/media/pages/assets/{pack}/[^\"]*\.zip", page)
    if not m:
        sys.exit(f"no download for kenney pack {pack}")
    data = urllib.request.urlopen(m.group(0)).read()
    d.mkdir(parents=True)
    zipfile.ZipFile(io.BytesIO(data)).extractall(d)
    return d


GEN = ROOT / "art/gen/out"


def source(path):
    if path.startswith("gen:"):
        p = GEN / path[4:]
        if not p.exists():
            sys.exit(f"missing {p}; generate it with art/gen/flux.py")
        return p
    if path.startswith("repo:"):
        p = ROOT / path[5:]
        if not p.exists():
            sys.exit(f"missing {p}")
        return p
    pack, rest = path.split("/", 1)
    p = pack_dir(pack) / rest
    if not p.exists():
        sys.exit(f"missing {p}")
    return p


def parse(path):
    out = []
    for n, line in enumerate(path.read_text().splitlines(), 1):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [p.strip() for p in line.split("|")]
        if len(parts) < 2:
            sys.exit(f"{path}:{n}: want `name | source | max=N solo flip`")
        opts = {}
        for o in (parts[2].split() if len(parts) > 2 else []):
            k, _, v = o.partition("=")
            opts[k] = v or True
        out.append((parts[0], parts[1], opts))
    return out


def load_image(src, opts):
    im = Image.open(source(src)).convert("RGBA")
    if "crop" in opts:
        x, y, w, h = map(int, opts["crop"].split(","))
        im = im.crop((x, y, x + w, y + h))
    if "flip" in opts:
        im = im.transpose(Image.FLIP_LEFT_RIGHT)
    bbox = im.getchannel("A").getbbox()
    if bbox and "keep" not in opts:
        im = im.crop(bbox)
    cap = int(opts.get("max", 256))
    if max(im.size) > cap:
        k = cap / max(im.size)
        im = im.resize((max(1, round(im.width * k)), max(1, round(im.height * k))), Image.LANCZOS)
    return im


def shifts(a):
    h, w = a.shape[:2]
    p = np.pad(a, ((1, 1), (1, 1)) + ((0, 0),) * (a.ndim - 2))
    return [p[1 + dy : 1 + dy + h, 1 + dx : 1 + dx + w] for dy in (-1, 0, 1) for dx in (-1, 0, 1)]


def bleed(im, rounds=PAD + 2):
    a = np.asarray(im).copy()
    known = a[..., 3] > 8
    if known.all() or not known.any():
        return im
    rgb = a[..., :3].astype(np.float32) * known[..., None]
    for _ in range(rounds):
        k = known.astype(np.float32)
        s = sum(shifts(rgb * k[..., None]))
        n = sum(shifts(k))
        grow = (n > 0) & ~known
        rgb[grow] = s[grow] / n[grow][:, None]
        known = known | grow
    out = a.copy()
    soft = a[..., 3] <= 8
    out[..., :3][soft] = np.clip(rgb[soft], 0, 255).astype(np.uint8)
    return Image.fromarray(out)


def shelf_pack(items):
    pages = []
    cur = None
    for name, im in sorted(items, key=lambda t: -t[1].height):
        w, h = im.width, im.height
        if cur is None or cur["x"] + w > PAGE:
            if cur is not None:
                cur["y"] += cur["row"]
                cur["x"] = 0
                cur["row"] = 0
            if cur is None or cur["y"] + h > PAGE:
                cur = {"img": [], "x": 0, "y": 0, "row": 0, "used_h": 0}
                pages.append(cur)
        cur["img"].append((name, im, cur["x"], cur["y"]))
        cur["x"] += w
        cur["row"] = max(cur["row"], h)
        cur["used_h"] = max(cur["used_h"], cur["y"] + h)
    return pages


def pow2_near(n):
    p = 64
    while p * 1.5 < n:
        p *= 2
    return p


def pow2(n):
    p = 64
    while p < n:
        p *= 2
    return p


def png_bytes(im):
    b = io.BytesIO()
    im.save(b, "PNG", optimize=True)
    return b.getvalue()


def page_bytes(im, jpg=False, colors=0, webp=False):
    if webp:
        b = io.BytesIO()
        if im.mode == "RGBA" and im.getchannel("A").getextrema()[0] == 255:
            im = im.convert("RGB")
        im.save(b, "WEBP", quality=88, method=6, alpha_quality=95)
        return b.getvalue()
    if jpg:
        b = io.BytesIO()
        im.convert("RGB").save(b, "JPEG", quality=86, optimize=True, progressive=False)
        return b.getvalue()
    if colors:
        q = im.quantize(colors=colors, method=Image.Quantize.FASTOCTREE, dither=Image.Dither.NONE)
        b = io.BytesIO()
        q.save(b, "PNG", optimize=True)
        return b.getvalue()
    return png_bytes(im)


def build_sheet(manifest):
    entries = parse(manifest)
    solo, packed = [], []
    for name, src, opts in entries:
        im = load_image(src, opts)
        if "solo" in opts:
            solo.append((name, im, "jpg" in opts))
            continue
        im = Image.fromarray(np.pad(np.asarray(im), ((PAD, PAD), (PAD, PAD), (0, 0))))
        packed.append((name, bleed(im)))
    pages = []
    index = []
    for p in shelf_pack(packed):
        used_w = max(x + im.width for _, im, x, _ in p["img"])
        canvas = Image.new("RGBA", (pow2(used_w), pow2(p["used_h"])), (0, 0, 0, 0))
        for name, im, x, y in p["img"]:
            canvas.paste(im, (x, y))
            index.append(f"{name} {len(pages)} {x + PAD} {y + PAD} {im.width - 2 * PAD} {im.height - 2 * PAD}")
        pages.append(canvas)
    colors = int(next((o.get("colors", 0) for _, _, o in entries if "colors" in o), 0))
    atlas_jpg = any("atlasjpg" in o for _, _, o in entries)
    webp = not any("png" in o for _, _, o in entries)
    blobs = [page_bytes(p, jpg=atlas_jpg, colors=colors, webp=webp) for p in pages]
    for name, im, jpg in solo:
        index.append(f"{name} {len(blobs)} 0 0 {im.width} {im.height}")
        sq = im.resize((pow2_near(im.width), pow2_near(im.height)), Image.LANCZOS)
        index[-1] = f"{name} {len(blobs)} 0 0 {sq.width} {sq.height} {im.width} {im.height}"
        blobs.append(page_bytes(sq, jpg=jpg and not webp, webp=webp))
    out = bytearray(b"KGA1")
    out += struct.pack("<I", len(blobs))
    for b in blobs:
        out += struct.pack("<I", len(b)) + b
    idx = "\n".join(index).encode()
    out += struct.pack("<I", len(idx)) + idx
    ART_OUT.mkdir(parents=True, exist_ok=True)
    dst = ART_OUT / f"{manifest.stem}.kga"
    dst.write_bytes(out)
    return dst, len(entries), len(pages), len(out)


def decode(path):
    raw = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-ac", "1", "-ar", str(RATE), "-f", "f32le", "-"],
        capture_output=True,
        check=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.float32).copy()


def loudness_db(x):
    win = RATE // 50
    r = max(np.sqrt((x[i : i + win] ** 2).mean()) for i in range(0, len(x), win))
    return 20 * np.log10(max(r, 1e-6))


def trim(x, floor_db=-50):
    a = np.abs(x)
    th = 10 ** (floor_db / 20) * a.max()
    on = np.nonzero(a > th)[0]
    if len(on) == 0:
        return x
    x = x[max(on[0] - 64, 0) : on[-1] + 1]
    fade = min(len(x) // 4, RATE // 100)
    if fade:
        x[-fade:] *= np.linspace(1, 0, fade)
    return x


def encode(x, dst, quality=3):
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "f32le", "-ar", str(RATE), "-ac", "1", "-i", "-", "-c:a", "libvorbis", "-q:a", str(quality), str(dst)],
        input=x.astype(np.float32).tobytes(),
        check=True,
    )


def music_source(spec):
    p = MUSIC_CACHE / spec
    if not p.exists():
        sys.exit(f"missing music {p}; see sound.txt for its URL")
    return p


def build_sound():
    import tempfile
    tmp = Path(tempfile.mkdtemp())
    global SND_OUT
    final = SND_OUT
    SND_OUT = tmp
    count = 0
    for name, src, opts in parse(SND_IN):
        if "music" in opts:
            x = decode(music_source(src))
            if "start" in opts or "len" in opts:
                s = int(float(opts.get("start", 0)) * RATE)
                n = int(float(opts.get("len", len(x) / RATE)) * RATE)
                x = x[s : s + n]
            fade = RATE // 50
            x[:fade] *= np.linspace(0, 1, fade)
            x[-fade:] *= np.linspace(1, 0, fade)
            x *= 10 ** ((float(opts.get("db", -20)) - loudness_db(x)) / 20)
            encode(x, SND_OUT / f"{name}.ogg", quality=2)
            count += 1
            continue
        x = trim(decode(source(src)))
        if "speed" in opts:
            k = float(opts["speed"])
            x = np.interp(np.arange(0, len(x), k), np.arange(len(x)), x)
        x *= 10 ** ((float(opts.get("db", -18)) - loudness_db(x)) / 20)
        peak = np.abs(x).max()
        ceiling = 10 ** (-1 / 20)
        if peak > ceiling:
            x *= ceiling / peak
        encode(x, SND_OUT / f"{name}.ogg")
        count += 1
        for step in [int(s) for s in opts.get("ladder", "").split(",") if s]:
            k = 2 ** (step / 12)
            y = np.interp(np.arange(0, len(x), k), np.arange(len(x)), x)
            encode(y, SND_OUT / f"{name}+{step}.ogg")
            count += 1
    SND_OUT = final
    files = sorted(tmp.glob("*.ogg"))
    out = bytearray(b"KGS1") + struct.pack("<I", len(files))
    for f in files:
        name = f.stem.encode()
        data = f.read_bytes()
        out += struct.pack("<I", len(name)) + name + struct.pack("<I", len(data)) + data
    final.parent.mkdir(parents=True, exist_ok=True)
    (final.parent / "sound.kgs").write_bytes(out)
    return count


def main():
    what = sys.argv[1:] or ["art", "sound"]
    if "art" in what:
        for m in sorted(ART_IN.glob("*.txt")):
            dst, n, pages, size = build_sheet(m)
            print(f"{dst.relative_to(ROOT)}: {n} sprites, {pages} pages, {size // 1024} KB")
    if "sound" in what:
        n = build_sound()
        print(f"assets/sound.kgs: {n} sounds, {(ROOT / 'assets/sound.kgs').stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
