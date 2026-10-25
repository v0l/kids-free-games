#!/usr/bin/env python3
import os

import numpy as np
from PIL import Image

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "out")


def load(name):
    return Image.open(os.path.join(OUT, name)).convert("RGBA")


def save(im, name):
    im.save(os.path.join(OUT, name))


def mirror(im):
    w, h = im.size
    out = Image.new("RGBA", (w * 2, h))
    out.paste(im, (0, 0))
    out.paste(im.transpose(Image.FLIP_LEFT_RIGHT), (w, 0))
    return out


def seamless(im, blend=0.2):
    a = np.asarray(im).astype(np.float32)
    w = a.shape[1]
    n = int(w * blend)
    out = a[:, : w - n].copy()
    tail = a[:, w - n :]
    t = np.linspace(0, 1, n)[None, :, None]
    t = t * t * (3 - 2 * t)
    out[:, :n] = tail * (1 - t) + out[:, :n] * t
    return Image.fromarray(out.clip(0, 255).astype(np.uint8))


def knock_out_white(im, lo=225.0, hi=250.0):
    a = np.asarray(im).astype(np.float32)
    m = a[:, :, :3].min(axis=2)
    alpha = np.clip((hi - m) / (hi - lo), 0, 1)
    solid = np.maximum.accumulate(alpha > 0.5, axis=0)
    alpha = np.where(solid, 1.0, alpha)
    safe = np.maximum(alpha, 1e-3)[:, :, None]
    a[:, :, :3] = np.clip((a[:, :, :3] - 255.0 * (1 - safe)) / safe, 0, 255)
    a[:, :, 3] = alpha * 255
    out = Image.fromarray(a.astype(np.uint8))
    return out.crop(out.getchannel("A").getbbox())


def flatten(im):
    a = np.asarray(im).astype(np.float32)
    rows = a[:, :, :3].mean(axis=(1, 2), keepdims=True)
    cols = a[:, :, :3].mean(axis=(0, 2), keepdims=True)
    k = a[:, :, :3].mean() / rows * a[:, :, :3].mean() / cols
    a[:, :, :3] = a[:, :, :3] * k
    return Image.fromarray(a.clip(0, 255).astype(np.uint8))


def main():
    save(mirror(knock_out_white(load("jumper_hills.png"))), "jumper_hills_tile.png")
    far = knock_out_white(load("jumper_hills_far.png"), 205.0, 245.0)
    ramp = np.clip(np.linspace(-0.3, 1.0, far.height) / 0.55, 0, 1)[:, None]
    fa = np.asarray(far).copy()
    fa[:, :, 3] = (fa[:, :, 3] * ramp).astype(np.uint8)
    far = Image.fromarray(fa)
    save(mirror(far), "jumper_hills_far_tile.png")
    grass = load("crossy_grass.png").crop((280, 150, 1060, 320))
    save(mirror(grass), "crossy_grass_tile.png")
    kerb = mirror(load("crossy_kerb.png")).crop((0, 78, 2688, 192))
    save(kerb, "crossy_kerb_strip.png")
    for name in ["water", "rail"]:
        save(mirror(load(f"crossy_{name}.png")), f"crossy_{name}_tile.png")
    save(mirror(load("crossy_asphalt.png")), "crossy_asphalt_tile.png")
    wagon = load("crossy_wagon_cut.png")
    save(wagon.crop((95, 0, wagon.width, wagon.height)), "crossy_wagon_body.png")


if __name__ == "__main__":
    main()
