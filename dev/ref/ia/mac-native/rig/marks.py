#!/usr/bin/env python3
"""Read a marker's resting ink out of a frame, glyph run by glyph run.

State 14 measured where a marker sits; this reads what colour it is drawn in.
The unit of the reading is the **glyph run** — a contiguous group of ink columns
inside one text line — because a marker is a run the body's runs can be compared
against without knowing anything about the face.

A run's ink is the colour furthest from the ground that the run holds at least
`MIN` times, so a stem's fully covered pixels answer and its antialiasing does
not. At the default size a Mono stem is 4 device px wide, so a covered pixel
exists in every run; a run that has none (a lone comma at some sizes) comes back
with its own extreme and is marked `thin`.
"""
import os
import sys

sys.path.insert(0, sys.path[0] or ".")
import iarig as R

MIN = 6  # a covered pixel appears far more often than this in a 4 px stem


def _ink_pred(ground):
    gsum = sum(ground)
    dark = gsum < 384
    thr = gsum + 90 if dark else gsum - 90

    def is_ink(p):
        if R.is_bar(p):
            return False
        return sum(p) > thr if dark else sum(p) < thr

    return is_ink, dark


def runs_of(png, gap=6, min_run_px=4):
    """Every text line's glyph runs, each with the ink it is drawn in."""
    ground, (w, h, px) = R.ground_of(png)
    is_ink, dark = _ink_pred(ground)
    gsum = sum(ground)

    rows = {}
    for y in range(h):
        xs = [x for x in range(w) if is_ink(px[x, y])]
        if xs:
            rows[y] = xs
    bands = []
    for y in sorted(rows):
        if bands and y <= bands[-1][1] + gap:
            bands[-1][1] = y
        else:
            bands.append([y, y])

    out = []
    for y0, y1 in bands:
        cols = sorted({x for y in range(y0, y1 + 1) for x in rows.get(y, [])})
        if not cols:
            continue
        groups = []
        for x in cols:
            if groups and x <= groups[-1][1] + 1:
                groups[-1][1] = x
            else:
                groups.append([x, x])
        line = []
        for a, b in groups:
            if b - a + 1 < min_run_px:
                continue
            cnt = {}
            for y in range(y0, y1 + 1):
                for x in range(a, b + 1):
                    p = px[x, y]
                    if is_ink(p):
                        cnt[p] = cnt.get(p, 0) + 1
            if not cnt:
                continue
            def dist(p):
                return (sum(p) - gsum) if dark else (gsum - sum(p))
            solid = [p for p, n in cnt.items() if n >= MIN]
            thin = not solid
            core = max(solid or list(cnt), key=dist)
            line.append(dict(x0=a, x1=b, colour=core, npx=sum(cnt.values()), thin=thin))
        if line:
            out.append(dict(y0=y0, y1=y1, runs=line))
    return ground, out


def contrast(fg, bg):
    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    def lum(p):
        r, g, b = (lin(v) for v in p)
        return 0.2126 * r + 0.7152 * g + 0.0722 * b

    a, b = lum(fg), lum(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def alpha_of(ink, full, paper):
    """The alpha that flattens `full` over `paper` onto `ink`, per channel."""
    out = []
    for i in range(3):
        d = full[i] - paper[i]
        if d:
            out.append((ink[i] - paper[i]) / d)
    return sum(out) / len(out) if out else 0.0


def report(png, full=None):
    ground, ls = runs_of(png)
    print(f"\n{os.path.basename(png)} ground={R.hexof(ground)} lines={len(ls)}")
    for i, l in enumerate(ls):
        print(f"  line{i:2d} y={l['y0']}..{l['y1']}")
        for r in l["runs"]:
            c = r["colour"]
            extra = ""
            if full:
                extra = f" alpha={alpha_of(c, full, ground):.3f}"
            print(f"      x={r['x0']:5d}..{r['x1']:<5d} w={r['x1']-r['x0']+1:<3d} "
                  f"{R.hexof(c)} cr={contrast(c, ground):.2f}"
                  f"{' thin' if r['thin'] else ''}{extra} npx={r['npx']}")
    return ground, ls


if __name__ == "__main__":
    full = None
    args = [a for a in sys.argv[1:]]
    if args and args[0].startswith("#"):
        h = args.pop(0).lstrip("#")
        full = tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))
    for p in args:
        report(p, full)
