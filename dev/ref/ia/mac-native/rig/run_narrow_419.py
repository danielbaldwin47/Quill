#!/usr/bin/env python3
"""#419, state 25: the two narrower size classes at every text-size step.

[#344](../CAPTURE-2026-09-10.md#344--the-window-narrows) measured the narrowest
and the middle class at steps 5 and 8 only, and the wide-to-middle scale is not
constant across those two, so the other twelve steps cannot be derived. This
walks both classes across the whole ladder — 960 pt and 400 pt at steps 0 … 13 —
and adds three widths at step 5 (240, 320, 440) to see what the narrowest
class's margins follow.

Per configuration: the passage plain, select-all for the container, and four
selection fills on one body row, fitted for the advance. The fills are chosen
per configuration rather than fixed, because a 40-cell run does not fit a row at
step 13 of a 400 pt window and a 10-cell run is too short a lever at step 0 of a
960 pt one: the 4- and 8-cell fills give a first advance, the container says how
many cells a row holds, and two wider fills are shot inside that.

The body row is found the same way: the heading wraps to two or three rows in a
narrow window, so the number of Down presses that lands on a body row is probed
per configuration rather than assumed.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/run_narrow_419.py <raw-dir> <norm-dir> [tag ...]
    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/run_narrow_419.py <raw-dir> <norm-dir> --remeasure

Run from the repository root, with iA Writer in light appearance, Mono,
System — Default, limit 64, Library and Preview hidden, Focus, Typewriter,
Syntax, Style Check and Authors off, on a scratch document.
"""
import json
import os
import subprocess
import sys
import time

import numpy as np

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast
import iarig as R
import states as S

# Set by main(); left unset so that the three scripts beside this one can import
# it for the driver and the readings without standing in for its arguments.
RAW = NORM = None
ONLY = set()
HEIGHT_PT = 500
FILL = (204, 237, 248)
CHROME = 120                         # device rows of window chrome, never a fill
LIMIT = 64
OUT_JSON = os.path.join("dev", "ref", "ia", "mac-native", "narrow-419.json")
PREFIX = "mac-native-25-light-narrow"

# (window width pt, text-size step). The two classes across the whole ladder,
# then the three widths that ask what the narrowest class's margins follow.
CONFIGS = ([(960, s) for s in range(14)] +
           [(400, s) for s in range(14)] +
           [(240, 5), (320, 5), (440, 5)])


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout.strip()


def submenu(bar, parent, item):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{item}" of menu 1 of menu item "{parent}" of menu 1 of menu bar item "{bar}" '
        f'of menu bar 1')


def size(item, settle=0.45):
    """One click of the Text Size menu."""
    submenu("View", "Text Size", item)
    time.sleep(settle)


def width(w):
    """Set the window that wide, and answer the bounds the app gave back.

    The app refuses to go below 240 pt, so the answer is not always the ask —
    `sweep_narrow.py` already assumed that and #419 records it.
    """
    osa(f'tell application "iA Writer" to set bounds of window 1 to {{0, 33, {w}, 982}}')
    time.sleep(0.9)
    return osa('tell application "iA Writer" to get bounds of window 1')


def step(n):
    """Text Size step, counted the way State 11 counts it: Default is 5.

    Walked up from the floor rather than out from Normal. The ladder stops at
    its bottom, so the Smaller clicks are self-correcting — a dropped one
    changes nothing — and only the n Bigger clicks after it can miss. Reaching
    step 1 as four Smaller clicks from Normal is what dropped a click in the
    first run of both widths (`ladder_419.py` caught it).

    Sixteen clicks, not eight: the previous configuration may have left the app
    at the top of a fourteen-step ladder, and eight from there lands on step 1
    rather than the floor — which is what put the second run's steps 10 to 12
    one and two rungs high.
    """
    for _ in range(16):
        size("Make Text Smaller", 0.35)
    for _ in range(n):
        size("Make Text Bigger", 0.5)


def ax():
    return dict(window=osa('tell application "System Events" to tell process "iA Writer" to '
                           'return {position, size} of window 1'),
                scroll_area=osa('tell application "System Events" to tell process "iA Writer" '
                                'to return {position, size} of scroll area 1 of '
                                'splitter group 1 of window 1'))


def shoot(name, region):
    R.shoot(region, os.path.join(RAW, name))
    colour.normalise(os.path.join(RAW, name), os.path.join(NORM, name))
    return os.path.join(NORM, name)


def fill_box(png):
    """The selection fill's box, ignoring stray pixels and the window chrome.

    `fast.fill_box` reads the whole frame; the window's own chrome can hold a
    pixel near the band's colour, so the top rows are cut and a column has to
    carry four of them before it counts.
    """
    m = fast.near_mask(fast.arr(png), FILL, 12)
    m[:CHROME] = False
    rows = np.where(m.sum(axis=1) >= 4)[0]
    cols = np.where(m.sum(axis=0) >= 4)[0]
    if not len(rows) or not len(cols):
        return None
    return dict(x0=int(cols.min()), x1=int(cols.max()), y0=int(rows.min()), y1=int(rows.max()),
                w=int(cols.max() - cols.min() + 1), h=int(rows.max() - rows.min() + 1))


def body_lines(png):
    """Ink rows, without the window's own top edge, which runs the frame's width."""
    g, lines = fast.ink_lines(png)
    wide = 0.9 * fast.arr(png).shape[1]
    return g, [l for l in lines if not (l["y0"] < 30 and l["x1"] - l["x0"] > wide)]


JOIN = 8                             # device rows: narrower than any line separation


def pitch_of(png):
    """The body line pitch off a plain frame: the shortest gap between line tops.

    Two things have to be undone first, and both of them put a wrong rung in an
    earlier run. A row of ascender tips can fall away from its own line's body
    for two or three rows, and `fast.ink_lines` reads that as two lines, which
    moves the line's top down and shortens the gap before it — so bands closer
    than `JOIN` rows are rejoined. Anything left under 9 px tall is antialiasing
    rather than a line, and letting one through gives a pitch of five or six.

    The shortest gap, not the median: a fractional pitch lands alternately on
    two whole numbers, and the ladders are written in the lower one.
    """
    _, lines = body_lines(png)
    joined = []
    for l in lines:
        if joined and l["y0"] - joined[-1]["y1"] <= JOIN:
            joined[-1] = dict(joined[-1], y1=l["y1"], h=l["y1"] - joined[-1]["y0"] + 1)
        else:
            joined.append(dict(l))
    tops = [l["y0"] for l in joined[1:] if l["h"] > 8]
    gaps = [b - a for a, b in zip(tops, tops[1:])]
    return min(gaps) if gaps else None


def fit(fills):
    ns = sorted(fills)
    if len(ns) < 2:
        return None, None
    a, b = np.polyfit(ns, [fills[n]["w"] for n in ns], 1)
    return round(float(a), 3), round(float(b), 2)


def home():
    S.keyc(126, "command down")
    time.sleep(0.35)


def name_for(tag, kind):
    return f"{PREFIX}-{tag}-{kind}.png"


def counts_for(container_w, advance, gutter_px):
    """Two more fills inside the row the container leaves, the wider one last.

    The room is the container less its two gutters, which the 4-cell fill's own
    left edge measures: the narrowest class holds a one-cell gutter where the
    middle class holds seven, so a fixed allowance would either wrap a fill or
    leave the fit a two-point lever.
    """
    if not container_w or not advance:
        return []
    room = int((container_w - 2 * gutter_px) / advance) - 1
    top = max([c for c in (40, 30, 24, 20, 16, 12) if c <= room], default=None)
    if top is None:
        return []
    mid = top // 2
    return [c for c in (mid, top) if c > 8]


def read(tag, w, st, cells, probe, ax_geometry):
    """Every number this state carries, read off its normalised frames."""
    frames = {k: os.path.join(NORM, name_for(tag, k))
              for k in ["plain", "all"] + [f"sel{n:02d}" for n in cells]}
    for f in frames.values():
        colour.check(f, "light")
    fills = {n: fill_box(frames[f"sel{n:02d}"]) for n in cells}
    container = fill_box(frames["all"])
    g, lines = body_lines(frames["plain"])
    heading = lines[0]
    body = [l for l in lines[1:] if l["h"] > 8]
    tops = [l["y0"] for l in body]
    gaps = [b - a for a, b in zip(tops, tops[1:])]
    pitch = pitch_of(frames["plain"])
    single = {n: b for n, b in fills.items()
              if b and pitch and b["h"] <= 1.4 * pitch}
    advance, edge = fit(single)
    one = single[min(single)] if single else None
    gutter = (round((one["x0"] - container["x0"]) / advance, 2)
              if one and container and advance else None)
    cells_in = round(container["w"] / advance, 2) if container and advance else None
    return dict(tag=tag, width_pt=w, step=st, limit=LIMIT, region_pt=[0, 33, w, HEIGHT_PT],
                ground=list(g), probe_downs=probe,
                heading_ink=dict(x0=heading["x0"], y0=heading["y0"], y1=heading["y1"]),
                body_y0=tops[:8], gaps=gaps[:7], pitch=pitch,
                band_h=min((b["h"] for b in single.values()), default=None),
                fills={str(n): b for n, b in fills.items()},
                wrapped=[n for n in fills if n not in single],
                advance=advance, fill_edge=edge,
                em=round(advance / 1.2, 3) if advance else None,
                container=container, container_w=container["w"] if container else None,
                container_centre=(container["x0"] + container["x1"] + 1) / 2 if container else None,
                requested=round((LIMIT + 14) * advance, 1) if advance else None,
                window_px=w * 2, window_minus_20=w * 2 - 20,
                gutter_cells=gutter,
                container_cells=cells_in,
                measure=(round(cells_in - 2 * gutter, 2)
                         if cells_in is not None and gutter is not None else None),
                ax=ax_geometry, frames={k: os.path.basename(v) for k, v in frames.items()})


def run(w, st):
    tag = f"w{w:04d}-step{st:02d}"
    region = (0, 33, w, HEIGHT_PT)
    S.REGION = region

    S.reset()
    time.sleep(0.4)
    plain = shoot(name_for(tag, "plain"), region)
    g, lines = body_lines(plain)
    heading_y1 = lines[0]["y1"]
    pitch = pitch_of(plain) or 60

    S.keys("a", "command down")
    time.sleep(0.6)
    container = fill_box(shoot(name_for(tag, "all"), region))

    # The Down presses that land on a body row: the heading wraps in a narrow
    # window, and a blank line follows it.
    probe = None
    for d in range(2, 8):
        home()
        S.goto(downs=d, shift_rights=4)
        time.sleep(0.4)
        b = fill_box(shoot(name_for(tag, "sel04"), region))
        if b and b["y0"] > heading_y1 and b["h"] <= 1.4 * pitch:
            probe = d
            break
    if probe is None:
        print(json.dumps(dict(tag=tag, error="no body row found")), flush=True)
        return dict(tag=tag, width_pt=w, step=st, error="no body row found")

    cells = [4, 8]
    home()
    S.goto(downs=probe, shift_rights=8)
    time.sleep(0.4)
    shoot(name_for(tag, "sel08"), region)
    b4 = fill_box(os.path.join(NORM, name_for(tag, "sel04")))
    b8 = fill_box(os.path.join(NORM, name_for(tag, "sel08")))
    a0 = (b8["w"] - b4["w"]) / 4.0 if b4 and b8 else None
    gutter_px = (b4["x0"] - container["x0"]) if b4 and container else 0
    for n in counts_for(container["w"] if container else None, a0, gutter_px):
        home()
        S.goto(downs=probe, shift_rights=n)
        time.sleep(0.4)
        shoot(name_for(tag, f"sel{n:02d}"), region)
        cells.append(n)

    row = read(tag, w, st, cells, probe, ax())
    print(json.dumps(row), flush=True)
    return row


def main():
    global RAW, NORM, ONLY
    RAW, NORM, ONLY = sys.argv[1], sys.argv[2], set(sys.argv[3:])
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    kept = {r["tag"]: r for r in (json.load(open(OUT_JSON)) if os.path.exists(OUT_JSON) else [])}
    if ONLY == {"--remeasure"}:
        rows = [read(r["tag"], r["width_pt"], r["step"], [int(n) for n in r["fills"]],
                     r["probe_downs"], r["ax"]) for r in kept.values() if "error" not in r]
        json.dump(rows, open(OUT_JSON, "w"), indent=1)
        for r in rows:
            print(json.dumps(r), flush=True)
        return
    rows = []
    for w, st in CONFIGS:
        tag = f"w{w:04d}-step{st:02d}"
        if ONLY and tag not in ONLY:
            if tag in kept:
                rows.append(kept[tag])
            continue
        width(w)
        step(st)
        rows.append(run(w, st))
        json.dump(rows, open(OUT_JSON, "w"), indent=1)
    width(1512)
    step(5)
    json.dump(rows, open(OUT_JSON, "w"), indent=1)


if __name__ == "__main__":
    main()
