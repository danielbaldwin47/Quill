#!/usr/bin/env python3
"""Every distinct ink a line is drawn in, and the columns each one covers.

`marks.py` reports one row per glyph run, which is the right unit to measure in
and the wrong one to read: a body line comes back as fifty identical rows. This
groups a line's runs by the ink they carry, so a line that is drawn in one
colour prints one row and a line that changes ink prints one row per ink, with
the column span each holds and the cell indices that span works out to.

Cells are counted off the body column and the advance the grid was measured at
(NOTES § The grid), so a span can be read back as "characters 8 … 12" without
counting pixels.
"""
import os
import sys

sys.path.insert(0, sys.path[0] or ".")
import iarig as R
import marks

BODY_X = 691.0   # NOTES § The text container
ADVANCE = 25.6   # NOTES § The grid at the default text size
TOL = 6          # two inks within this per channel are one ink


def cells(x):
    return (x - BODY_X) / ADVANCE


def group(runs):
    """Runs collapsed into ink groups, in column order, merging near-equal inks."""
    out = []
    for r in runs:
        c = r["colour"]
        if out and all(abs(out[-1]["colour"][i] - c[i]) <= TOL for i in range(3)):
            out[-1]["runs"].append(r)
        else:
            out.append(dict(colour=c, runs=[r]))
    for g in out:
        g["x0"] = min(r["x0"] for r in g["runs"])
        g["x1"] = max(r["x1"] for r in g["runs"])
        g["npx"] = sum(r["npx"] for r in g["runs"])
        # the ink of the group is the one its widest run carries
        g["colour"] = max(g["runs"], key=lambda r: r["npx"])["colour"]
    return out


def report(png, paper=None):
    ground, ls = marks.runs_of(png)
    paper = paper or ground
    print(f"\n{os.path.basename(png)} ground={R.hexof(ground)} lines={len(ls)}")
    for i, l in enumerate(ls):
        gs = group(l["runs"])
        print(f"  line{i:2d} y={l['y0']}..{l['y1']}")
        for g in gs:
            print(f"      {R.hexof(g['colour'])} cr={marks.contrast(g['colour'], paper):5.2f} "
                  f"alpha={marks.alpha_of(g['colour'], INK, paper):.3f} "
                  f"x={g['x0']}..{g['x1']} cells {cells(g['x0']):5.1f}..{cells(g['x1']):5.1f} "
                  f"runs={len(g['runs'])}")
    return ground, ls


INK = (0xcc, 0xcc, 0xcc)

if __name__ == "__main__":
    args = list(sys.argv[1:])
    if args and args[0].startswith("#"):
        h = args.pop(0).lstrip("#")
        INK = tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))
    for p in args:
        report(p)
