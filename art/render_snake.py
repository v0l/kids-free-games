#!/usr/bin/env python3
from pathlib import Path

import numpy as np
from PIL import Image

PACK = Path.home() / ".cache/kids-games/kenney/animal-pack/PNG"
OUT = PACK / "Derived"


def shift(im, turn=0.32):
    hsv = np.asarray(im.convert("HSV")).astype(np.int32)
    hue = hsv[..., 0] / 255.0
    green = (hue > 0.18) & (hue < 0.42) & (hsv[..., 1] > 60)
    hsv[..., 0] = np.where(green, ((hue + turn) % 1.0) * 255, hsv[..., 0])
    out = Image.fromarray(hsv.astype(np.uint8), "HSV").convert("RGBA")
    out.putalpha(im.getchannel("A"))
    return out


def disk(top, bottom, size=128):
    yy, xx = np.mgrid[0:size, 0:size].astype(np.float32)
    c = (size - 1) / 2
    d = np.hypot(xx - c, yy - c)
    k = np.clip(yy / size, 0, 1)[..., None]
    rgb = np.array(top, np.float32) * (1 - k) + np.array(bottom, np.float32) * k
    alpha = np.clip((size / 2 - 1 - d) * 1.5, 0, 1)
    out = np.dstack([rgb, alpha * 255]).astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def main():
    OUT.mkdir(exist_ok=True)
    shift(Image.open(PACK / "Round (outline)/snake.png").convert("RGBA")).save(OUT / "head.png")
    disk((86, 120, 250), (52, 84, 214)).save(OUT / "body.png")
    print(OUT)


if __name__ == "__main__":
    main()
