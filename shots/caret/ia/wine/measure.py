#!/usr/bin/env python3
"""Read the caret bar, the selection fill and the glyph ink out of an iA Writer frame.

The same reading `shots/caret/147/measure147.py` takes of Quill's own shots, so
the two can be put in one table: for each bar its column and width, whether it
is solid over its whole height, and how far the nearest glyph ink is either
side. The band is found rather than passed — the one row of the frame that has
glyph ink in it — so a shot that scrolled or moved is still measured correctly.

    python3 measure.py <shot.png> [<shot.png> ...]
"""
import subprocess
import sys

# The first wrapped line of the editor pane, at the zoom the comparison is taken
# at: left of X0 is the library and the file list, below Y1 the second line.
# The cell is about 71 px here, against 24 at the app's default, which is what
# makes a 2 px side bearing something to measure rather than something to argue
# about.
X0, X1 = 970, 2950
Y0, Y1 = 150, 310


def load(png):
    w, h = (int(v) for v in subprocess.run(
        ["identify", "-format", "%w %h", png],
        capture_output=True, text=True, check=True).stdout.split())
    raw = subprocess.run(["magick", png, "-depth", "8", "RGB:-"],
                         capture_output=True, check=True).stdout
    return w, h, raw


def classify(png):
    w, h, raw = load(png)

    def px(x, y):
        i = (y * w + x) * 3
        return raw[i], raw[i + 1], raw[i + 2]

    counts = {}
    for y in range(Y0, Y1):
        for x in range(X0, X1, 3):
            p = px(x, y)
            counts[p] = counts.get(p, 0) + 1
    ground = max(counts, key=counts.get)

    def is_bar(p):
        # The caret's own accent: strongly blue and bright with it.
        r, g, b = p
        return b > 180 and b - r > 90 and g > 100

    def is_fill(p):
        # The selection's tint: blue-biased but nowhere near the bar's strength.
        r, g, b = p
        return not is_bar(p) and b - r > 10 and b > 120

    def is_ink(p):
        return sum(p) < 330

    # The text band: the rows that carry glyph ink.
    rows = [y for y in range(Y0, Y1)
            if sum(1 for x in range(X0, X1) if is_ink(px(x, y))) > 3]
    if not rows:
        return png, ground, None, [], [], []
    band = (min(rows), max(rows))
    bh = band[1] - band[0] + 1

    def runs(pred, minrows):
        cols = [x for x in range(X0, X1)
                if sum(1 for y in range(band[0], band[1] + 1) if pred(px(x, y))) >= minrows]
        out = []
        for x in cols:
            if out and x == out[-1][1]:
                out[-1][1] = x + 1
            else:
                out.append([x, x + 1])
        return out

    # A bar is the line's own height; ink and fill need only show up.
    bars = runs(is_bar, max(4, bh // 2))
    fills = runs(is_fill, max(4, bh // 2))
    ink = runs(is_ink, 1)
    return png, ground, band, bars, fills, ink


def report(png):
    png, ground, band, bars, fills, ink = classify(png)
    name = png.split("/")[-1]
    if band is None:
        print(f"{name}: no text band")
        return None
    print(f"\n{name}  ground={ground} band={band[0]}..{band[1]}")
    for a, b in fills:
        print(f"   fill  x={a}..{b - 1} w={b - a}")
    for a, b in bars:
        left = [r for r in ink if r[1] <= a]
        right = [r for r in ink if r[0] >= b]
        gl = a - left[-1][1] if left else None
        gr = right[0][0] - b if right else None
        on = any(r[0] < b and r[1] > a for r in ink)
        print(f"   bar   x={a} w={b - a} gap<{gl} gap>{gr} ink-under={on}")
    return [b[0] for b in bars]


first = {}
for p in sys.argv[1:]:
    xs = report(p)
    if xs:
        first[p.split("/")[-1].replace(".png", "")] = xs

# The cell pitch, straight off the consecutive-offset carets.
offs = sorted((int(k.split("-")[1]), v[0]) for k, v in first.items()
              if k.startswith("caret-"))
if len(offs) > 1:
    print("\ncaret column by offset: " + "  ".join(f"{o}:{x}" for o, x in offs))
    steps = [b - a for (_, a), (_, b) in zip(offs, offs[1:])]
    print(f"steps between offsets: {steps}")
