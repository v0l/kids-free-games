#!/usr/bin/env python3
import random
from PIL import Image, ImageDraw, ImageFilter

PPM = 40
W, L = int(5.4 * PPM), 8 * PPM
random.seed(29)
im = Image.new("RGB", (W, L), (88, 92, 112))
px = im.load()
for y in range(L):
    for x in range(W):
        n = random.randint(-9, 9)
        r, g, b = px[x, y]
        px[x, y] = (r + n, g + n, b + n)
im = im.filter(ImageFilter.GaussianBlur(0.6))
d = ImageDraw.Draw(im)
for lane_edge in (-0.8, 0.8):
    x = int((2.7 + lane_edge) * PPM)
    d.rectangle([x - 3, int(L * 0.15), x + 3, int(L * 0.6)], fill=(250, 246, 232))
for side in (-2.55, 2.55):
    x = int((2.7 + side) * PPM)
    d.rectangle([x - 4, 0, x + 4, L], fill=(255, 214, 90))
im.save("art/gen/out/runner_road_tile.png")
