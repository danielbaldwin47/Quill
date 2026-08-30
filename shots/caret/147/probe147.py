#!/usr/bin/env python3
"""THROWAWAY (#147): per-column pixel counts across one band.

So a claim about a bar standing on a glyph can be checked against the pixels
rather than against a downscaled contact sheet — which is how `solid` and `cut`
in `measure147.py` were confirmed, and how the first reading of them (counting
columns, which called a `t`'s three-row crossbar a smear) was caught.

    python3 shots/caret/147/probe147.py <shot.png> <y> <h> <x0> <x1>

Prints one line per column: how many rows are the caret's blue, how many are
glyph ink, and what the darkest pixel in that column is. The thresholds are
`measure147.py`'s `is_bar` and `is_ink`, repeated rather than imported so this
stays a file somebody can run on its own.
"""
import subprocess
import sys

png, y, h, x0, x1 = sys.argv[1], *map(int, sys.argv[2:6])
iw = int(subprocess.run(["identify", "-format", "%w", png],
                        capture_output=True, text=True, check=True).stdout)
raw = subprocess.run(
    ["magick", png, "-crop", f"{iw}x{h}+0+{y}", "+repage", "-depth", "8", "RGB:-"],
    capture_output=True, check=True).stdout

for x in range(x0, x1 + 1):
    blue = ink = 0
    darkest = (255, 255, 255)
    for row in range(h):
        i = (row * iw + x) * 3
        r, g, b = raw[i], raw[i + 1], raw[i + 2]
        if r <= 90 and 120 <= g <= 220 and b >= 200:
            blue += 1
        if r + g + b < 330:
            ink += 1
        if r + g + b < sum(darkest):
            darkest = (r, g, b)
    print(f"x={x:5} blue={blue:3} ink={ink:3} darkest={darkest}")
