#!/usr/bin/env python3
import sys

from PIL import Image

im = Image.open(sys.argv[1]).convert("RGBA")
w, h = im.size
out = Image.new("RGBA", (w * 2, h))
out.paste(im, (0, 0))
out.paste(im.transpose(Image.FLIP_LEFT_RIGHT), (w, 0))
out.save(sys.argv[2])
