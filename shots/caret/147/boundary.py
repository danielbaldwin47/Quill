#!/usr/bin/env python3
"""#147 acceptance: the bar's centre against the advance boundary.

The boundary is read from the selection fill, which `Editor::selection` builds
from the same `iter_location` x values and which this ticket does not touch:
a fill from offset a to offset b spans exactly boundary(a) .. boundary(b).
The bar is read from the caret shots at those same two offsets.
"""
import subprocess
import sys


def load(png):
    w, h = (int(v) for v in subprocess.run(
        ["identify", "-format", "%w %h", png],
        capture_output=True, text=True, check=True).stdout.split())
    raw = subprocess.run(["magick", png, "-depth", "8", "RGB:-"],
                         capture_output=True, check=True).stdout
    return w, h, raw


def columns(png, hit):
    """The x range of the columns holding a pixel `hit` accepts, and its rows."""
    w, h, raw = load(png)
    xs, ys = [], []
    for y in range(h):
        base = y * w * 3
        for x in range(w):
            i = base + x * 3
            if hit(raw[i], raw[i + 1], raw[i + 2]):
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return min(xs), max(xs), min(ys), max(ys)


def bar(r, g, b):
    """The caret's accent: saturated cyan-blue, whatever the exact hex."""
    return b > 180 and r < 110 and g > 120 and b - r > 100


def fill(r, g, b):
    """The light theme's active selection fill, a pale accent wash."""
    return 190 < r < 225 and 225 < g < 245 and b > 240


shots = dict(z.split("=", 1) for z in sys.argv[1:])
sel = columns(shots["select"], fill)
print(f"fill              x {sel[0]}..{sel[1]}  (boundary a = {sel[0]}, boundary b = {sel[1] + 1})")
lo_boundary, hi_boundary = sel[0], sel[1] + 1
for name, boundary in (("a", lo_boundary), ("b", hi_boundary)):
    b = columns(shots[name], bar)
    x0, x1 = b[0], b[1]
    centre = (x0 + x1 + 1) / 2
    print(f"bar at offset {name}   x {x0}..{x1}  w {x1 - x0 + 1}  centre {centre}  "
          f"boundary {boundary}  delta {centre - boundary:+.1f}")
