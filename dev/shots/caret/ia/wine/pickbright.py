#!/usr/bin/env python3
"""Keep the frame of a burst that has the most caret in it.

The caret blinks, so a single shot is a coin toss. Counting the accent-strength
pixels across the text band picks the frame where the bar is on; a state that
never draws one scores the same everywhere and keeps the first frame.

    pickbright.py <out.png> <frame.png> [<frame.png> ...]
"""
import shutil
import subprocess
import sys

OUT, FRAMES = sys.argv[1], sys.argv[2:]
X0, X1, Y0, Y1 = 970, 2520, 150, 310


def accent_pixels(png):
    w = int(subprocess.run(["identify", "-format", "%w", png],
                           capture_output=True, text=True, check=True).stdout)
    raw = subprocess.run(
        ["magick", png, "-crop", f"{X1 - X0}x{Y1 - Y0}+{X0}+{Y0}", "+repage",
         "-depth", "8", "RGB:-"], capture_output=True, check=True).stdout
    n = 0
    for i in range(0, len(raw), 3):
        r, g, b = raw[i], raw[i + 1], raw[i + 2]
        if b > 180 and b - r > 90 and g > 100:
            n += 1
    return n


best = max(FRAMES, key=accent_pixels)
shutil.copyfile(best, OUT)
print(f"{best.split('/')[-1]} -> {OUT.split('/')[-1]} ({accent_pixels(best)} accent px)")
