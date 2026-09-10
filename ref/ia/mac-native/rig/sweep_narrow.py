#!/usr/bin/env python3
"""#344: find the window width the Editor's type changes at, by bisection.

The pitch alone separates the two size classes, so one plain frame a width is
enough and the search can be a bisection rather than a sweep. This writes the
widths it tried and what each gave; the two sides of the threshold it finds are
then shot as full states by `run_narrow.py`, which is where the committed
evidence for it lives. Frames go to a scratch directory and are not kept.

    python3 rig/sweep_narrow.py <scratch-dir> <out.json>

Run from the repository root, with the app in the state `run_narrow.py` wants.
"""
import json
import os
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import iarig as R
import run_narrow as N
import states as S

SCRATCH, OUT = sys.argv[1], sys.argv[2]
COARSE = [240, 400, 500, 700, 900, 1040, 1200, 1300, 1512]  # 240 pt is the app's
# narrowest window; the list only has to straddle every threshold, the bisection
# does the rest.


def pitch(w):
    """The body line pitch at one window width, from a frame at the document top."""
    N.width(w)
    S.keyc(126, "command down")
    time.sleep(0.45)
    png = os.path.join(SCRATCH, f"w{w:04d}.png")
    R.shoot((0, 33, w, 400), png)
    _, lines = N.body_lines(png)
    body = [l for l in lines[1:] if l["h"] > 8]
    tops = [l["y0"] for l in body]
    gaps = sorted(b - a for a, b in zip(tops, tops[1:]))
    os.remove(png)
    return gaps[0] if gaps else None


def bisect(lo, hi, seen):
    """The greatest width that still gives the narrow pitch, and the one above it."""
    while hi - lo > 1:
        mid = (lo + hi) // 2
        seen[mid] = pitch(mid)
        if seen[mid] == seen[lo]:
            lo = mid
        else:
            hi = mid
    return lo, hi


def main():
    os.makedirs(SCRATCH, exist_ok=True)
    runs = {}
    for st, lim in ((5, 64), (8, 64), (5, 80)):
        if lim != 64:
            N.limit(lim)
        N.step(st)
        S.reset()
        time.sleep(0.4)
        seen = {}
        widths = COARSE
        for w in widths:
            seen[w] = pitch(w)
            print(f"step {st} limit {lim} width {w:5} pitch {seen[w]}", flush=True)
        edges = sorted(w for w in widths[:-1]
                       if seen[w] != seen[widths[widths.index(w) + 1]])
        found = []
        for w in edges:
            lo, hi = bisect(w, widths[widths.index(w) + 1], seen)
            found.append(dict(narrow_at=lo, wide_at=hi,
                              narrow_pitch=seen[lo], wide_pitch=seen[hi]))
            print(f"  threshold: {seen[lo]} px pitch up to {lo} pt, {seen[hi]} px from {hi} pt",
                  flush=True)
        runs[f"step{st}-limit{lim}"] = dict(pitch_by_width={str(k): v for k, v in sorted(seen.items())},
                                            thresholds=found)
        if lim != 64:
            N.limit(64)
    json.dump(runs, open(OUT, "w"), indent=1)


if __name__ == "__main__":
    main()
