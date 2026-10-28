#!/usr/bin/env python3
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter, ImageFont

OUT = Path(__file__).resolve().parent / "gen/out"
SS = 4


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def egg():
    n = 192 * SS
    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    cx, cy = n / 2, n * 0.54
    for i in range(40):
        t = i / 39
        rx, ry = n * 0.36 * (1 - t * 0.55), n * 0.44 * (1 - t * 0.55)
        col = lerp((214, 140, 20), (255, 232, 110), t)
        d.ellipse([cx - rx - t * n * 0.05, cy - ry - t * n * 0.06, cx + rx - t * n * 0.05, cy + ry - t * n * 0.06], fill=col + (255,))
    d.ellipse([n * 0.28, n * 0.2, n * 0.42, n * 0.36], fill=(255, 255, 255, 200))
    d.ellipse([n * 0.44, n * 0.74, n * 0.6, n * 0.8], fill=(255, 255, 255, 60))
    ol = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    od = ImageDraw.Draw(ol)
    od.ellipse([cx - n * 0.375, cy - n * 0.455, cx + n * 0.345, cy + n * 0.445], outline=(150, 80, 10, 255), width=SS * 3)
    mask = im.getchannel("A")
    ol.putalpha(Image.composite(ol.getchannel("A"), Image.new("L", (n, n), 0), mask.filter(ImageFilter.MaxFilter(SS * 4 + 1))))
    im = Image.alpha_composite(ol, im)
    im.resize((192, 192), Image.LANCZOS).save(OUT / "clash_egg.png")


def tuft():
    n = 128 * SS
    im = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    for dx, h, lean, col in [(-0.2, 0.55, -0.12, (70, 150, 50)), (0.2, 0.5, 0.12, (70, 150, 50)), (0, 0.75, 0.0, (110, 190, 70)), (-0.1, 0.62, -0.05, (90, 170, 60)), (0.1, 0.6, 0.06, (90, 170, 60))]:
        bx = n * (0.5 + dx * 0.5)
        pts = [(bx - n * 0.05, n * 0.9), (bx + n * lean * 2, n * (0.9 - h)), (bx + n * 0.05, n * 0.9)]
        d.polygon(pts, fill=col + (255,))
    im.resize((128, 128), Image.LANCZOS).save(OUT / "clash_tuft.png")


def snake_path(n, pts, r, c1, c2, d):
    for i, (x, y) in enumerate(pts):
        t = i / (len(pts) - 1)
        rr = r * (0.55 + 0.45 * t)
        col = lerp(c1, c2, 0.5 + 0.5 * math.sin(i * 0.9))
        d.ellipse([x - rr - 4, y - rr - 4, x + rr + 4, y + rr + 4], fill=(40, 30, 60, 255))
    for i, (x, y) in enumerate(pts):
        t = i / (len(pts) - 1)
        rr = r * (0.55 + 0.45 * t)
        col = lerp(c1, c2, 0.5 + 0.5 * math.sin(i * 0.9))
        d.ellipse([x - rr, y - rr, x + rr, y + rr], fill=col + (255,))
        d.ellipse([x - rr * 0.6, y - rr * 0.85, x + rr * 0.1, y - rr * 0.35], fill=(255, 255, 255, 70))


def head(d, x, y, r, col):
    d.ellipse([x - r - 5, y - r - 5, x + r + 5, y + r + 5], fill=(40, 30, 60, 255))
    d.ellipse([x - r, y - r, x + r, y + r], fill=col + (255,))
    d.ellipse([x - r * 0.65, y - r * 0.9, x + r * 0.05, y - r * 0.4], fill=(255, 255, 255, 80))
    for s in (-1, 1):
        ex = x + s * r * 0.42
        d.ellipse([ex - r * 0.34, y - r * 0.5 - r * 0.34, ex + r * 0.34, y - r * 0.5 + r * 0.4], fill=(255, 255, 255, 255), outline=(40, 30, 60, 255), width=3)
        d.ellipse([ex - r * 0.16 + s * 2, y - r * 0.45 - r * 0.12, ex + r * 0.16 + s * 2, y - r * 0.45 + r * 0.22], fill=(30, 20, 40, 255))
    d.arc([x - r * 0.45, y - r * 0.05, x + r * 0.45, y + r * 0.55], 15, 165, fill=(40, 30, 60, 255), width=4)


def icon():
    n = 384
    im = Image.new("RGBA", (n, n))
    px = im.load()
    for y in range(n):
        for x in range(n):
            t = (y / n) * 0.7 + (x / n) * 0.3
            px[x, y] = lerp((120, 230, 190), (40, 150, 170), t) + (255,)
    d = ImageDraw.Draw(im)
    for i in range(14):
        a = i * 0.9
        d.ellipse([60 + (i * 97) % 300, 40 + (i * 53) % 300, 80 + (i * 97) % 300, 60 + (i * 53) % 300], fill=(255, 255, 255, 40))
    big = []
    for i in range(24):
        t = i / 23
        big.append((70 + t * 220 + math.sin(t * 6.5) * 18, 300 - math.sin(t * 3.1) * 90 - t * 30 + math.cos(t * 9) * 12))
    snake_path(n, big, 38, (150, 90, 230), (190, 140, 255), d)
    head(d, 300, 168, 62, (160, 100, 240))
    small = [(40 + t * 90, 340 - math.sin(t * 4) * 12) for t in [i / 8 for i in range(9)]]
    snake_path(n, small, 16, (255, 150, 60), (255, 190, 90), d)
    head(d, 146, 336, 22, (255, 160, 70))
    d.ellipse([268, 30, 348, 110], fill=(255, 215, 60, 255), outline=(40, 30, 60, 255), width=5)
    try:
        f = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 52)
    except OSError:
        f = ImageFont.load_default()
    d.text((308, 70), "7", font=f, fill=(90, 40, 10, 255), anchor="mm")
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, n - 1, n - 1], radius=84, fill=255)
    im.putalpha(mask)
    im.save(OUT / "iconr_clash.png")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    egg()
    tuft()
    icon()
