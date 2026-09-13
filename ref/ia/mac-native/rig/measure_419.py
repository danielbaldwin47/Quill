#!/usr/bin/env python3
"""#419's reader: the two narrower ladders, off the frames the run kept.

Prints the three tables [`../CAPTURE-2026-09-13.md`](../CAPTURE-2026-09-13.md)
carries — the middle class across the ladder, the narrowest across it, and the
three step-5 widths that ask what the narrowest class's margins follow — and the
derived columns the report argues from: the scale each class holds against the
wide ladder of § State 11, `pitch / em`, and the container against both terms of
the oracle's rule.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/measure_419.py [narrow-419.json]

`--verify` re-reads every number off the committed frames first, so the tables
cannot drift from the pixels.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = os.path.join("ref", "ia", "mac-native", "narrow-419.json")

# § State 11, the wide ladder on this rig: step -> (cell px, pitch px).
WIDE = {0: (17.4, 49), 1: (18.3, 52), 2: (19.4, 56), 3: (20.6, 59), 4: (23.1, 66),
        5: (25.6, 73), 6: (30.7, 86), 7: (35.7, 98), 8: (40.7, 109), 9: (45.7, 120),
        10: (53.1, 135), 11: (60.4, 149), 12: (67.8, 161), 13: (75.1, 172)}


# The ladders `ladder_419.py` walked one click at a time, pitch per step. A
# configuration whose plain frame does not sit on its width's rung was shot at
# the wrong text size, which is how the first run's step 1 and the second run's
# steps 10 to 12 were caught.
LADDER = {960: [43, 46, 49, 53, 56, 63, 69, 81, 92, 103, 113, 126, 139, 150],
          400: [37, 40, 44, 47, 50, 53, 59, 65, 76, 86, 96, 105, 118, 129]}


def rows(path):
    rs = [r for r in json.load(open(path)) if "error" not in r]
    off = [(r["tag"], r["pitch"], LADDER[r["width_pt"]][r["step"]]) for r in rs
           if r["width_pt"] in LADDER and r["pitch"] != LADDER[r["width_pt"]][r["step"]]]
    if off:
        raise SystemExit("off the ladder: " +
                         "; ".join(f"{t} pitch {got} wants {want}" for t, got, want in off))
    return rs


def f(v, n=3):
    return "—" if v is None else f"{v:.{n}f}"


def ladder(rs, width):
    print(f"\n### {width} pt\n")
    print("| step | advance | em | pitch | pitch / em | scale vs wide | container | 78 cells | "
          "window − 20 | gutter | measure |")
    print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for r in sorted((x for x in rs if x["width_pt"] == width), key=lambda x: x["step"]):
        a, em, p = r["advance"], r["em"], r["pitch"]
        wide_cell = WIDE[r["step"]][0]
        # `em` is in points; `pitch / em` is read in device pixels, the way
        # § State 11 reads it, so the em there is the advance at 0.6 em a cell.
        print(f"| {r['step']} | {f(a)} | {f(em)} | {p} | {f(p / (a / 0.6) if a else None)} | "
              f"{f(a / wide_cell if a else None, 4)} | {r['container_w']} | {f(r['requested'], 1)} | "
              f"{r['window_minus_20']} | {f(r['gutter_cells'], 2)} | {f(r['measure'], 2)} |")


def margins(rs):
    print("\n### The narrowest class's margins, step 5\n")
    print("| window pt | window px | advance | container | 78 cells | window − 20 | "
          "left margin px | left margin cells | container / window |")
    print("|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for r in sorted((x for x in rs if x["step"] == 5), key=lambda x: x["width_pt"]):
        c = r["container"]
        left = c["x0"] if c else None
        print(f"| {r['width_pt']} | {r['window_px']} | {f(r['advance'])} | {r['container_w']} | "
              f"{f(r['requested'], 1)} | {r['window_minus_20']} | {left} | "
              f"{f(left / r['advance'] if left is not None and r['advance'] else None, 2)} | "
              f"{f(r['container_w'] / r['window_px'] if r['container_w'] else None, 4)} |")


def wrapped(rs):
    bad = [(r["tag"], r["wrapped"]) for r in rs if r["wrapped"]]
    print("\n### Fills that wrapped (dropped from the fit)\n")
    print("none" if not bad else "\n".join(f"- {t}: {w}" for t, w in bad))


def main():
    path = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else PATH
    if "--verify" in sys.argv:
        subprocess.run([sys.executable, os.path.join(HERE, "run_narrow_419.py"),
                        "ref/ia/shots/mac-native/raw-2026-09-13", "ref/ia/shots/mac-native",
                        "--remeasure"], check=True, stdout=subprocess.DEVNULL)
    rs = rows(path)
    print(f"{len(rs)} configurations")
    ladder(rs, 960)
    ladder(rs, 400)
    margins(rs)
    wrapped(rs)


if __name__ == "__main__":
    main()
