from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).parent / "out"
S = 4
N = 384 * S
R = int(0.22 * N)


def rounded(size, radius):
    m = Image.new("L", (size, size), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, size - 1, size - 1), radius, fill=255)
    return m


def gradient(top, bottom):
    g = Image.new("RGB", (N, N))
    d = ImageDraw.Draw(g)
    for y in range(N):
        k = y / N
        d.line([(0, y), (N, y)], fill=tuple(int(top[i] + (bottom[i] - top[i]) * k) for i in range(3)))
    return g


def tile(pic, box, size, radius):
    t = pic.crop(box).resize((size, size), Image.LANCZOS).convert("RGBA")
    t.putalpha(rounded(size, radius))
    return t


def shadow(canvas, tl, size, radius, blur, dy):
    m = Image.new("L", canvas.size, 0)
    ImageDraw.Draw(m).rounded_rectangle(
        (tl[0], tl[1] + dy, tl[0] + size, tl[1] + size + dy), radius, fill=120
    )
    m = m.filter(ImageFilter.GaussianBlur(blur))
    dark = Image.new("RGBA", canvas.size, (40, 10, 60, 255))
    dark.putalpha(m)
    canvas.alpha_composite(dark)


pic = Image.open(OUT / "menu_bg.png").convert("RGB")
w, h = pic.size
side = min(w, h)
x0 = (w - side) // 2
art = pic.crop((x0, 0, x0 + side, side))
half = side // 2

canvas = gradient((255, 214, 120), (240, 140, 80)).convert("RGBA")
cell = int(N * 0.37)
gap = int(N * 0.03)
radius = int(cell * 0.22)
left = (N - 2 * cell - gap) // 2
top = int(N * 0.15)
spots = [(left, top), (left + cell + gap, top), (left, top + cell + gap), (left + cell + gap, top + cell + gap)]
for i, (cx, cy) in enumerate(spots):
    box = ((i % 2) * half, (i // 2) * half, (i % 2) * half + half, (i // 2) * half + half)
    if i == 3:
        cx, cy = int(N * 0.60), int(N * 0.58)
    shadow(canvas, (cx, cy), cell, radius, 14 * S / 4, 12)
    t = tile(art, box, cell, radius)
    if i == 3:
        t = t.rotate(-12, resample=Image.BICUBIC, expand=True)
        cx -= (t.width - cell) // 2
        cy -= (t.height - cell) // 2
    canvas.alpha_composite(t, (cx, cy))

canvas.putalpha(rounded(N, R))
canvas.resize((384, 384), Image.LANCZOS).save(OUT / "iconr_jigsaw.png")
