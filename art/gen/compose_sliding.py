#!/usr/bin/env python3
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parent / "out"
SIZE = 512

PICTURES = [
    ("lion", "memory_lion_cut", "memory_bg"),
    ("panda", "memory_panda_cut", "count_bg"),
    ("penguin", "memory_penguin_cut", "jumper_sky"),
    ("whale", "w_whale_cut", "jumper_sky"),
    ("rocket", "w_rocket_cut", "merge_bg"),
    ("frog", "w_frog_cut", "garden_bg"),
]


def square(im, bottom=False):
    side = min(im.size)
    x = (im.width - side) // 2
    y = im.height - side if bottom else (im.height - side) // 2
    return im.crop((x, y, x + side, y + side)).resize((SIZE, SIZE), Image.LANCZOS)


def picture(animal, bg, flip_bg_bottom):
    base = square(Image.open(OUT / f"{bg}.png").convert("RGBA"), flip_bg_bottom)
    cut = Image.open(OUT / f"{animal}.png").convert("RGBA")
    k = SIZE * 0.78 / max(cut.size)
    cut = cut.resize((round(cut.width * k), round(cut.height * k)), Image.LANCZOS)
    x = (SIZE - cut.width) // 2
    y = SIZE - cut.height - round(SIZE * 0.07)
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).ellipse(
        (x + cut.width * 0.1, y + cut.height * 0.88, x + cut.width * 0.9, y + cut.height * 1.02),
        fill=(30, 20, 60, 110),
    )
    base.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(10)))
    base.alpha_composite(cut, (x, y))
    return base.convert("RGB")


def rounded(im, radius):
    mask = Image.new("L", im.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, im.width - 1, im.height - 1), radius, fill=255)
    out = im.convert("RGBA")
    out.putalpha(mask)
    return out


def icon(pics):
    s = 384
    base = Image.new("RGB", (s, s), (255, 190, 70))
    d = ImageDraw.Draw(base)
    d.rectangle((0, 0, s, s // 2), fill=(255, 150, 60))
    pad, gap = 28, 8
    cell = (s - 2 * pad - 2 * gap) // 3
    pic = pics[0].resize((cell * 3, cell * 3), Image.LANCZOS)
    for r in range(3):
        for c in range(3):
            if (r, c) == (2, 2):
                continue
            tile = pic.crop((c * cell, r * cell, (c + 1) * cell, (r + 1) * cell))
            x = pad + c * (cell + gap)
            y = pad + r * (cell + gap)
            d.rounded_rectangle((x - 3, y - 3, x + cell + 3, y + cell + 3), 14, fill=(255, 255, 255))
            base.paste(rounded(tile, 12), (x, y), rounded(tile, 12))
    return rounded(base, round(s * 0.22))


def main():
    pics = []
    for name, animal, bg in PICTURES:
        im = picture(animal, bg, bg in ("jumper_sky", "garden_bg"))
        im.save(OUT / f"slide_{name}.png")
        pics.append(im)
    icon(pics).save(OUT / "slide_icon.png")
    print("wrote", len(pics) + 1, "images")


if __name__ == "__main__":
    sys.exit(main())
