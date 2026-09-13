#!/usr/bin/env python3
"""#419: bisect the window width the narrowest class's margin changes at.

The step-5 widths #419 asks for put the container 16 px inside a 240 and a
320 pt window and 26 px inside a 400 and a 440 pt one, where the middle class
keeps 10 px at every width. One select-all frame a width is enough to read a
margin, so the break is bisected the way `sweep_narrow.py` bisects the size
classes, and its frames are scratch.

It is bisected at more than one text size, because a break that moved with the
size would be a different thing from the class breaks, which the window's width
alone decides. Every requested width is recorded beside the bounds the app gave
back, so the floor the app puts on a window is evidence rather than an aside.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/sweep_419.py <scratch-dir> <out.json> [step ...]

Run from the repository root, light, Mono, limit 64.
"""
import json
import os
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import colour
import iarig as R
import run_narrow_419 as N
import states as S

SCRATCH, OUT = sys.argv[1], sys.argv[2]
STEPS = [int(s) for s in sys.argv[3:]] or [0, 5, 8]
FLOOR = [150, 180, 200, 220, 240]    # the app's own minimum width, asked for
COARSE = [240, 280, 320]             # inside the lower band, to show it is flat
LO, HI = 320, 440
os.makedirs(SCRATCH, exist_ok=True)


def margin(w, st):
    """The container's own left edge inside a window this wide, in device px."""
    bounds = N.width(w)
    S.REGION = (0, 33, w, N.HEIGHT_PT)
    S.reset()
    time.sleep(0.35)
    S.keys("a", "command down")
    time.sleep(0.5)
    png = os.path.join(SCRATCH, f"scratch-{st}-{w:04d}.png")
    R.shoot(S.REGION, png)
    colour.normalise(png)
    b = N.fill_box(png)
    row = dict(step=st, width_pt=w, window_px=w * 2, asked_bounds=f"0, 33, {w}, 982",
               got_bounds=bounds, margin=b["x0"] if b else None,
               container_w=b["w"] if b else None,
               right_margin=(w * 2 - (b["x0"] + b["w"])) if b else None)
    print(json.dumps(row), flush=True)
    return row


def bisect(st, rows):
    """The two widths the margin changes between, at one text size."""
    seen = {r["width_pt"]: r for r in rows if r["step"] == st}

    def at(w):
        if w not in seen:
            seen[w] = margin(w, st)
            rows.append(seen[w])
        return seen[w]["margin"]

    below, lo, hi = at(LO), LO, HI
    at(hi)
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if at(mid) == below:
            lo = mid
        else:
            hi = mid
    return dict(step=st, break_at=[lo, hi], margin_below=below, margin_above=seen[hi]["margin"])


def main():
    rows, breaks, floor = [], [], []
    for w in FLOOR:
        got = N.width(w)
        floor.append(dict(asked_pt=w, got_bounds=got))
        print(json.dumps(floor[-1]), flush=True)
    for st in STEPS:
        N.step(st)
        for w in COARSE:
            rows.append(margin(w, st))
        breaks.append(bisect(st, rows))
        print(json.dumps(breaks[-1]), flush=True)
    json.dump(dict(minimum_width=floor, breaks=breaks, rows=rows), open(OUT, "w"), indent=1)
    print(f"{OUT}: {len(rows)} widths, {len(breaks)} bisections", flush=True)
    N.width(1512)
    N.step(5)


if __name__ == "__main__":
    main()
