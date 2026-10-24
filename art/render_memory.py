#!/usr/bin/env python3
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

PACK = Path.home() / ".cache/kids-games/kenney/boardgame-pack/PNG/Cards"
OUT = PACK / "Derived"


def main():
    OUT.mkdir(exist_ok=True)
    back = Image.open(PACK / "cardBack_blue1.png").convert("RGBA")
    a = np.asarray(back).copy()
    solid = a[..., 3] > 128
    inner = ndimage.binary_erosion(solid, iterations=9)
    rim = solid & ~ndimage.binary_erosion(solid, iterations=5)
    a[solid, :3] = (255, 255, 255)
    a[inner, :3] = (255, 250, 236)
    a[solid & ~rim & ~inner, :3] = (232, 226, 210)
    Image.fromarray(a, "RGBA").save(OUT / "face.png")
    print(OUT)


if __name__ == "__main__":
    main()
