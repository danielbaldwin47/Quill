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


DATA = os.path.join("ref", "ia", "mac-native")
MARGINS = os.path.join(DATA, "narrow-419-margins-steps.json")


def ladders():
    """The pitch per step of each class, off `ladder_419.py`'s continuous walks.

    Read rather than transcribed, so the guard below cannot drift from the
    evidence it guards. Default is step 5: the walk's `smaller-n` is step 5 − n
    and its `bigger+n` step 5 + n, and the rungs past the floor and the ceiling
    are repeats the walk takes to prove it has reached them.
    """
    out = {}
    for w in (960, 400):
        path = os.path.join(DATA, f"narrow-419-ladder-{w}.json")
        if not os.path.exists(path):
            continue
        by = {}
        for r in json.load(open(path)):
            lab = r["label"]
            step = (5 if lab.startswith("normal") else
                    5 - int(lab.split("-")[1]) if lab.startswith("smaller") else
                    5 + int(lab.split("+")[1]))
            if 0 <= step <= 13:
                by[step] = r["pitch"]
        out[w] = [by.get(s) for s in range(14)]
    return out


LADDER = ladders()


def ladder_for(width):
    """The class's ladder, since a class is one ladder at every width in it.

    440 pt and below is the narrowest class, walked at 400; 441 to 1250 is the
    middle class, walked at 960. A width outside both is the wide class, whose
    ladder is § State 11's and not this run's to guard.
    """
    return LADDER.get(400) if width <= 440 else LADDER.get(960) if width <= 1250 else None


SLACK = 1                            # a fractional pitch lands on two whole numbers


def off_ladder(readings):
    """Rows whose plain frame does not sit on its class's rung.

    A state runner that reaches a text size by counting menu clicks drops one
    now and then — it is how the first run's step 1 and the second run's steps
    10 to 12 went out — so nothing here prints until every row is on its rung.

    Within a pixel, because a pitch of 43.5 is read as 43 off one frame and 44
    off another. The rungs are three pixels apart at their closest, so a pixel
    of slack still catches every dropped click.
    """
    off = []
    for label, width, step, pitch in readings:
        lad = ladder_for(width)
        want = lad[step] if lad else None
        if want is not None and abs(pitch - want) > SLACK:
            off.append(f"{label} pitch {pitch} wants {want}")
    return off


def margin_rule(advance_px, K):
    """The narrowest class's side margin in device px: max(5, round(K − advance)) pt."""
    return max(5, round(K - advance_px / 2)) * 2


def check_margins(rs):
    """The published rule against every narrowest-class margin measured.

    The advance is the class's own at that step, off the 400 pt states; K is
    17.5 pt in a window up to 390 pt and 22.5 pt from 391 to 440.
    """
    if not os.path.exists(MARGINS):
        return []
    adv = {r["step"]: r["advance"] for r in rs if r["width_pt"] == 400}
    seen = [(r["width_pt"], r["step"], r["container"]["x0"]) for r in rs if r["width_pt"] <= 440]
    seen += [(m["width_pt"], m["step"], m["margin"]) for m in json.load(open(MARGINS))
             if m["width_pt"] <= 440]
    bad = []
    for w, s, got in seen:
        want = margin_rule(adv[s], 17.5 if w <= 390 else 22.5)
        if got != want:
            bad.append(f"w{w} step{s} margin {got} wants {want}")
    print(f"\nmargin rule checked on {len(seen)} narrowest-class readings: "
          f"{'all fit' if not bad else '; '.join(bad)}")
    return bad


def rows(path):
    rs = [r for r in json.load(open(path)) if "error" not in r]
    off = off_ladder([(r["tag"], r["width_pt"], r["step"], r["pitch"]) for r in rs])
    if os.path.exists(MARGINS):
        off += off_ladder([(f"margins w{m['width_pt']} step{m['step']}", m["width_pt"],
                            m["step"], m["pitch"]) for m in json.load(open(MARGINS))])
    if off:
        raise SystemExit("off the ladder: " + "; ".join(off))
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


def margin_ladder(rs):
    """The table the report publishes: the margin at every width and step, against the rule."""
    if not os.path.exists(MARGINS):
        return
    by = {(m["width_pt"], m["step"]): m["margin"] for m in json.load(open(MARGINS))}
    by.update({(r["width_pt"], r["step"]): r["container"]["x0"] for r in rs
               if r["width_pt"] <= 440 and r["container"]})
    adv = {r["step"]: r["advance"] for r in rs if r["width_pt"] == 400}
    ws = [240, 320, 400, 440]
    print("\n### The narrowest class's margin at every width and step\n")
    print("| step | " + " | ".join(f"{w} pt" for w in ws) +
          " | `max(5, round(17.5 − advance_pt))` | `max(5, round(22.5 − advance_pt))` |")
    print("|---:|" + "---:|" * (len(ws) + 2))
    for s in range(14):
        cells = [str(by.get((w, s), "—")) for w in ws]
        print(f"| {s} | " + " | ".join(cells) +
              f" | {margin_rule(adv[s], 17.5)} | {margin_rule(adv[s], 22.5)} |")


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
    margin_ladder(rs)
    wrapped(rs)
    if check_margins(rs):
        raise SystemExit("the margin rule does not fit every reading")


if __name__ == "__main__":
    main()
