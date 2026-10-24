#!/usr/bin/env python3
import sys
from pathlib import Path

import numpy as np
from PIL import Image

PACK = Path.home() / ".cache/kids-games/kenney/food-kit"
MODELS = PACK / "Models/OBJ format"
OUT = PACK / "Render"
SIZE = 192
SS = 3
LIGHT = np.array([-0.4, 0.7, 0.6]) / np.linalg.norm([-0.4, 0.7, 0.6])

_tex = np.asarray(Image.open(MODELS / "Textures/colormap.png").convert("RGB")).astype(np.float32) / 255


def load(name):
    v, vt, vn, tris = [], [], [], []
    for line in (MODELS / f"{name}.obj").read_text().splitlines():
        p = line.split()
        if not p:
            continue
        if p[0] == "v":
            v.append([float(x) for x in p[1:4]])
        elif p[0] == "vt":
            vt.append([float(x) for x in p[1:3]])
        elif p[0] == "vn":
            vn.append([float(x) for x in p[1:4]])
        elif p[0] == "f":
            idx = [tuple(int(i) - 1 if i else None for i in (q.split("/") + ["", ""])[:3]) for q in p[1:]]
            for i in range(1, len(idx) - 1):
                tris.append((idx[0], idx[i], idx[i + 1]))
    return np.array(v), np.array(vt), np.array(vn), tris


def rot(a, axis):
    c, s = np.cos(a), np.sin(a)
    return [
        np.array([[1, 0, 0], [0, c, -s], [0, s, c]]),
        np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]]),
        np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]]),
    ][axis]


def render(name, pitch=30, yaw=0, roll=0, cut=False):
    v, vt, vn, tris = load(name)
    centre = (v.min(0) + v.max(0)) / 2
    local = v - centre
    r = rot(np.radians(pitch), 0) @ rot(np.radians(yaw), 1) @ rot(np.radians(roll), 2)
    p = local @ r.T
    n_all = vn @ r.T
    big = SIZE * SS
    scale = big * 0.9 / (np.abs(p[:, :2]).max() * 2.0)
    sx = p[:, 0] * scale + big / 2
    sy = -p[:, 1] * scale + big / 2
    z = p[:, 2]
    img = np.zeros((big, big, 4), np.float32)
    back = np.zeros((big, big), bool)
    zbuf = np.full((big, big), -1e9, np.float32)
    for t in tris:
        ia = [a[0] for a in t]
        x, y, zz = sx[ia], sy[ia], z[ia]
        xm = local[ia, 0]
        x0, x1 = int(max(x.min(), 0)), int(min(x.max() + 1, big - 1))
        y0, y1 = int(max(y.min(), 0)), int(min(y.max() + 1, big - 1))
        if x1 <= x0 or y1 <= y0:
            continue
        d = (y[1] - y[2]) * (x[0] - x[2]) + (x[2] - x[1]) * (y[0] - y[2])
        if abs(d) < 1e-9:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
        w0 = ((y[1] - y[2]) * (gx - x[2]) + (x[2] - x[1]) * (gy - y[2])) / d
        w1 = ((y[2] - y[0]) * (gx - x[2]) + (x[0] - x[2]) * (gy - y[2])) / d
        w2 = 1 - w0 - w1
        m = (w0 >= -1e-6) & (w1 >= -1e-6) & (w2 >= -1e-6)
        if cut:
            m &= (w0 * xm[0] + w1 * xm[1] + w2 * xm[2]) <= 0
        pz = w0 * zz[0] + w1 * zz[1] + w2 * zz[2]
        sub = zbuf[y0 : y1 + 1, x0 : x1 + 1]
        m &= pz > sub
        if not m.any():
            continue
        sub[m] = pz[m]
        uv = np.array([vt[a[1]] for a in t])
        u = w0 * uv[0, 0] + w1 * uv[1, 0] + w2 * uv[2, 0]
        vv = w0 * uv[0, 1] + w1 * uv[1, 1] + w2 * uv[2, 1]
        tx = np.clip((u * _tex.shape[1]).astype(int), 0, _tex.shape[1] - 1)
        ty = np.clip(((1 - vv) * _tex.shape[0]).astype(int), 0, _tex.shape[0] - 1)
        col = _tex[ty, tx]
        nn = [n_all[a[2]] for a in t]
        n = w0[..., None] * nn[0] + w1[..., None] * nn[1] + w2[..., None] * nn[2]
        n /= np.linalg.norm(n, axis=-1, keepdims=True) + 1e-9
        lam = np.clip(n @ LIGHT, 0, 1) * 0.45 + 0.62
        col = np.clip(col * lam[..., None], 0, 1)
        out = img[y0 : y1 + 1, x0 : x1 + 1]
        out[m, :3] = col[m]
        out[m, 3] = 1
        back[y0 : y1 + 1, x0 : x1 + 1][m] = (n[..., 2] < 0)[m]
    im = Image.fromarray((img * 255).astype(np.uint8), "RGBA").resize((SIZE, SIZE), Image.LANCZOS)
    face = Image.fromarray((back * 255).astype(np.uint8)).resize((SIZE, SIZE), Image.LANCZOS)
    return im, np.asarray(face) > 127


FLESH = {
    "apple": (255, 232, 205),
    "orange": (255, 170, 50),
    "watermelon": (236, 70, 84),
    "pineapple": (252, 214, 84),
    "lemon": (255, 238, 140),
    "strawberry": (240, 92, 104),
    "coconut": (246, 246, 240),
    "pear": (238, 238, 176),
}
WHOLE = list(FLESH)


def halves(im, flesh):
    a = np.asarray(im).copy()
    alpha = a[..., 3] > 128
    xs = np.nonzero(alpha.any(0))[0]
    mid = (xs.min() + xs.max() + 1) // 2
    out = []
    for side in ("l", "r"):
        h = a.copy()
        if side == "l":
            h[:, mid:] = 0
            rim = np.zeros_like(alpha)
            rim[:, mid - 14 : mid] = True
            edge = np.zeros_like(alpha)
            edge[:, mid - 2 : mid] = True
        else:
            h[:, :mid] = 0
            rim = np.zeros_like(alpha)
            rim[:, mid : mid + 14] = True
            edge = np.zeros_like(alpha)
            edge[:, mid : mid + 2] = True
        keep = (h[..., 3] > 128)
        paint = keep & rim
        h[paint, :3] = flesh
        pale = keep & edge
        h[pale, :3] = (255, 255, 255)
        out.append(Image.fromarray(h, "RGBA"))
    return out


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for name in WHOLE:
        whole = render(name)[0]
        whole.save(OUT / f"{name}.png")
        left, right = halves(whole, FLESH[name])
        left.save(OUT / f"{name}-l.png")
        right.save(OUT / f"{name}-r.png")
    print(OUT)


if __name__ == "__main__":
    sys.exit(main())
