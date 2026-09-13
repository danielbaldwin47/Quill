#!/usr/bin/env python3
"""#381's reader: the bar at the foot of the window, off the pairs it was shot in.

Every state was shot with the bar and again with `View > Toolbar > Hide` and
nothing else changed, so the bar is exactly where the two frames differ — and,
more to the point, the **gutter above it** is read against a page that has no
bar under it at all, which is the one question a single frame cannot answer.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/measure_stats_381.py

Run from the repository root.
"""
import json
import os
import sys

import numpy as np
from PIL import Image

SHOTS = os.path.join("ref", "ia", "shots", "mac-native")
PREFIX = "mac-native-27"
STATE = os.path.join("ref", "ia", "mac-native", "stats-381.json")
PAPER = {"light": (0xf7, 0xf7, 0xf7), "dark": (0x1a, 0x1a, 0x1a)}


def arr(name):
    return np.asarray(Image.open(os.path.join(SHOTS, name)).convert("RGB")).astype(np.int16)


def hexof(p):
    return "#%02x%02x%02x" % tuple(int(v) for v in p)


def band_of(on, off, tol=20):
    """The bar's own rows: the bottom run of rows the two frames differ on."""
    d = np.abs(arr(on) - arr(off)).sum(axis=2) > tol
    rows = np.where(d.any(axis=1))[0]
    if not len(rows):
        return None
    y1 = y0 = int(rows.max())
    for r in rows[::-1]:
        if y0 - int(r) > 8:
            break
        y0 = int(r)
    return y0, y1


def rule_above(a, y0, ground):
    """The hairline over the bar: the rows above it that are neither paper nor bar.

    Read as a row's median across the middle half of the width, so a glyph
    reaching down into it cannot be mistaken for a rule.
    """
    w = a.shape[1]
    rows = []
    for y in range(y0 - 6, y0 + 4):
        med = np.median(a[y, w // 4:3 * w // 4], axis=0)
        rows.append((int(y), hexof(med), bool(np.abs(med - ground).sum() > 6)))
    return rows


def ink_runs(a, y0, y1, ground, min_w=30):
    """The bar's labels, as column groups of ink wider than the window's corner."""
    band = a[y0 + 6:y1 - 5]
    ink = (np.abs(band - ground).sum(axis=2) > 90).sum(axis=0) >= 2
    cols = np.where(ink)[0]
    out, prev = [], None
    for c in cols:
        if prev is None or c - prev > 30:
            out.append([int(c), int(c)])
        else:
            out[-1][1] = int(c)
        prev = c
    return [r for r in out if r[1] - r[0] + 1 >= min_w]


def darkest(a, y0, y1, x0, x1, ground):
    """The ink a label carries: the value furthest from the ground it stands on."""
    sub = a[y0:y1 + 1, x0:x1 + 1].reshape(-1, 3)
    far = np.abs(sub - ground).sum(axis=1)
    vals, counts = np.unique(sub[far > 60], axis=0, return_counts=True)
    if not len(vals):
        return None
    keep = counts >= max(4, counts.max() // 10)
    i = int(np.argmax(np.where(keep, np.abs(vals - ground).sum(axis=1), -1)))
    return hexof(vals[i])


def last_ink_row(a, y_stop, ground, left, right):
    """The lowest row of page ink above `y_stop`, inside the text column."""
    sub = a[:y_stop, left:right]
    ink = (np.abs(sub - ground).sum(axis=2) > 60).sum(axis=1) >= 2
    rows = np.where(ink)[0]
    return int(rows.max()) if len(rows) else None


def read(tag, ground):
    on, off = f"{PREFIX}-{ground}-stats-{tag}.png", f"{PREFIX}-{ground}-stats-{tag}-nobar.png"
    if not os.path.exists(os.path.join(SHOTS, on)):
        return None
    a, b = arr(on), arr(off)
    band = band_of(on, off)
    if band is None:
        return dict(tag=tag, ground=ground, bar=None)
    y0, y1 = band
    ground_px = np.array(PAPER[ground])
    bar_ground = np.median(a[y0 + 6:y1 - 5].reshape(-1, 3), axis=0)
    runs = ink_runs(a, y0, y1, bar_ground)
    # The text column: the container of § The grid, 512 … 2511 at the default.
    left, right = 512, 2512
    with_bar = last_ink_row(a, y0, ground_px, left, right)
    without = last_ink_row(b, y0, ground_px, left, right)
    return dict(
        tag=tag, ground=ground,
        bar=dict(rows=[y0, y1], height_px=y1 - y0 + 1, height_pt=(y1 - y0 + 1) / 2,
                 ground=hexof(bar_ground),
                 ground_is_paper=bool(np.abs(bar_ground - ground_px).sum() <= 3)),
        rule_above=rule_above(a, y0, bar_ground),
        labels=[dict(cols=r, ink=darkest(a, y0, y1, r[0], r[1], bar_ground)) for r in runs],
        counts=dict(cols=runs[-1] if runs else None,
                    ink=darkest(a, y0, y1, runs[-1][0], runs[-1][1], bar_ground) if runs else None),
        last_ink_row=dict(with_bar=with_bar, without_bar=without),
        gutter_px=(y0 - with_bar) if with_bar else None,
        air_below_last_row_px=(y1 - without) if without else None)


def main():
    state = json.load(open(STATE)) if os.path.exists(STATE) else {}
    out = {"observations": state.get("observations", {}), "states": {}}
    for ground in ("light", "dark"):
        for tag in ("c1-top", "c2-end", "c3-empty", "c6-selection"):
            got = read(tag, ground)
            if got:
                out["states"][f"{ground}/{tag}"] = got
    # The counts alone, on the frames that carry no control beside them.
    for name, ground in ((f"{PREFIX}-light-stats-c5-hover.png", "light"),
                         (f"{PREFIX}-dark-stats-c7-typewriter.png", "dark"),
                         (f"{PREFIX}-light-stats-o1-typing.png", "light"),
                         (f"{PREFIX}-light-stats-fade-at-rest.png", "light"),
                         (f"{PREFIX}-light-stats-fade-typing.png", "light")):
        if not os.path.exists(os.path.join(SHOTS, name)):
            continue
        a = arr(name)
        y0, y1 = 1818, 1886
        bar_ground = np.median(a[y0 + 6:y1 - 5].reshape(-1, 3), axis=0)
        runs = ink_runs(a, y0, y1, bar_ground)
        out["states"][name[:-4]] = dict(
            bar_ground=hexof(bar_ground), labels=len(runs),
            counts_ink=darkest(a, y0, y1, runs[-1][0], runs[-1][1], bar_ground) if runs else None,
            counts_cols=runs[-1] if runs else None)
    json.dump(out, open(os.path.join("ref", "ia", "mac-native", "stats-381-read.json"), "w"),
              indent=1)

    for key, s in out["states"].items():
        if "bar" in s:
            b = s["bar"]
            if not b:
                print(f"{key}: no bar")
                continue
            print(f"{key}: bar rows {b['rows']} h {b['height_px']} px ({b['height_pt']} pt) "
                  f"ground {b['ground']} paper={b['ground_is_paper']} "
                  f"labels {len(s['labels'])} counts {s['counts']['ink']} "
                  f"gutter {s['gutter_px']} air {s['air_below_last_row_px']}")
            print("   rule rows:", [(y, h) for y, h, differs in s["rule_above"] if differs])
        else:
            print(f"{key}: bar ground {s['bar_ground']} labels {s['labels']} "
                  f"counts {s['counts_ink']} at {s['counts_cols']}")
    print("\nobservations:", out["observations"])


if __name__ == "__main__":
    main()
