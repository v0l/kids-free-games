#!/usr/bin/env python3
import sys

import numpy as np
from PIL import Image


def seamless(src, dst, blend=0.18):
    im = Image.open(src).convert("RGBA")
    a = np.asarray(im).astype(np.float32)
    w = a.shape[1]
    n = int(w * blend)
    out = a[:, : w - n].copy()
    tail = a[:, w - n :]
    head = out[:, :n]
    t = np.linspace(0, 1, n)[None, :, None]
    t = t * t * (3 - 2 * t)
    out[:, :n] = tail * (1 - t) + head * t
    Image.fromarray(out.clip(0, 255).astype(np.uint8)).save(dst)


if __name__ == "__main__":
    seamless(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 0.18)
