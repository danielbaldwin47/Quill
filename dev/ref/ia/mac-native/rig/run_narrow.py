#!/usr/bin/env python3
"""#344: when the window narrows, which gives — the type or the measure.

#328 saw the Editor's type shrink at 960 and 1040 pt on the Dell rig, whose own
page-top control missed by 32 px. This walks the same question on the original
rig and adds the frames that separate the two hypotheses container overflow and
a width-of-window size class: the same widths at a step where 78 cells already
overflow the widest window, one width with the line-length limit moved (which
moves the measure and not the window), and the pair either side of the
threshold the sweep found.

Per configuration: the passage plain, selection fills of 10, 20 and 40 cells on
one body row (30 at the larger step, where 40 cells no longer fit a row), and
select-all. The advance is fitted across the fills, so no side bearing and no
selection edge enters it; the container is the select-all fill; the pitch is the
line tops of the plain frame.

    python3 rig/run_narrow.py <raw-dir> <normalised-dir> [tag ...]

Naming a tag re-shoots only those configurations and keeps the rest of the
manifest as it stands.

Run from the repository root, with iA Writer in light appearance, Mono, System —
Default, Library and Preview hidden, Focus, Typewriter, Syntax, Style Check and
Authors off.
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

RAW, NORM, ONLY = sys.argv[1], sys.argv[2], set(sys.argv[3:])
HEIGHT_PT = 500                      # 13 body rows at step 5, 6 at step 8
FILL = (204, 237, 248)               # light selection band, § 4.2
CHROME = 120                         # device rows of window chrome, skipped when a fill is boxed

# (window width pt, text-size step, line-length limit, cell counts to select).
# 1250 and 1251 are the two sides of the threshold; the sweep that found it is
# in the report. Ordered so the limit is set twice, not nine times.
MAIN5 = [10, 20, 40]
MAIN8 = [10, 20, 30]                 # 40 cells overflow a row at step 8
EDGE = [10, 20]
CONFIGS = ([(w, 5, 64, EDGE) for w in (440, 441)] +
           [(w, 5, 64, MAIN5) for w in (960, 1040, 1200, 1512)] +
           [(w, 5, 64, EDGE) for w in (1250, 1251)] +
           [(w, 8, 64, MAIN8) for w in (960, 1040, 1200, 1512)] +
           [(w, 8, 64, EDGE) for w in (1250, 1251)] +
           [(1040, 5, 80, MAIN5)] +
           [(w, 5, 80, EDGE) for w in (1250, 1251)])


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout.strip()


def submenu(bar, parent, item):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{item}" of menu 1 of menu item "{parent}" of menu 1 of menu bar item "{bar}" '
        f'of menu bar 1')


def width(w):
    osa(f'tell application "iA Writer" to set bounds of window 1 to {{0, 33, {w}, 982}}')
    time.sleep(0.9)


def step(n):
    """Text Size step, counted the way State 11 counts it: Default is 5."""
    submenu("View", "Text Size", "Make Text Normal Size")
    time.sleep(0.5)
    for _ in range(max(0, n - 5)):
        submenu("View", "Text Size", "Make Text Bigger")
        time.sleep(0.45)
    for _ in range(max(0, 5 - n)):
        submenu("View", "Text Size", "Make Text Smaller")
        time.sleep(0.45)


def limit(n):
    """Line length limit, from the Editor pane of Settings."""
    osa('tell application "System Events" to tell process "iA Writer" to keystroke "," '
        'using {command down}')
    time.sleep(1.3)
    for act in (f'click pop up button 4 of scroll area 1 of window "Editor"',
                f'click menu item "{n}" of menu 1 of pop up button 4 of scroll area 1 of '
                f'window "Editor"'):
        osa(f'tell application "System Events" to tell process "iA Writer" to {act}')
        time.sleep(0.6)
    got = osa('tell application "System Events" to tell process "iA Writer" to return value of '
              'pop up button 4 of scroll area 1 of window "Editor"')
    osa('tell application "System Events" to tell process "iA Writer" to click button 1 of '
        'window "Editor"')
    time.sleep(0.8)
    assert got == str(n), f"line length limit is {got!r}, wanted {n}"


def ax():
    """The window and its one scroll area, so the container is read against something."""
    return dict(window=osa('tell application "System Events" to tell process "iA Writer" to '
                           'return {position, size} of window 1'),
                scroll_area=osa('tell application "System Events" to tell process "iA Writer" '
                                'to return {position, size} of scroll area 1 of '
                                'splitter group 1 of window 1'))


def shoot(name, region):
    R.shoot(region, os.path.join(RAW, name))
    colour.normalise(os.path.join(RAW, name), os.path.join(NORM, name))
    return os.path.join(NORM, name)


def fill_box(png, top=CHROME):
    """The selection fill's box, ignoring stray pixels and the window chrome."""
    m = fast.near_mask(fast.arr(png), FILL, 12)
    m[:top] = False
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


def fit(fills):
    """Least squares through the single-row fills: width = advance × cells + edge."""
    ns = sorted(n for n, b in fills.items() if b)
    if len(ns) < 2:
        return None, None
    ws = [fills[n]["w"] for n in ns]
    a, b = np.polyfit(ns, ws, 1)
    return round(float(a), 3), round(float(b), 2)


def names_for(tag, cells):
    kinds = ["plain"] + [f"sel{n:02d}" for n in cells] + ["all"]
    return {k: f"mac-native-22-light-narrow-{tag}-{k}.png" for k in kinds}


def measure(tag, w, st, lim, cells, ax_geometry):
    """Every number this state carries, read off its normalised frames."""
    names = names_for(tag, cells)
    frames = {k: os.path.join(NORM, v) for k, v in names.items()}
    fills = {n: fill_box(frames[f"sel{n:02d}"]) for n in cells}
    container = fill_box(frames["all"])
    g, lines = body_lines(frames["plain"])
    heading = lines[0]
    body = [l for l in lines[1:] if l["h"] > 8]
    tops = [l["y0"] for l in body]
    gaps = [b - a for a, b in zip(tops, tops[1:])]
    band = min((b["h"] for b in fills.values() if b), default=None)
    single = {n: b for n, b in fills.items() if b and b["h"] <= (band or 0) + 4}
    advance, edge = fit(single)
    for f in frames.values():
        colour.check(f, "light")
    return dict(tag=tag, width_pt=w, step=st, limit=lim, region_pt=[0, 33, w, HEIGHT_PT],
                ground=list(g), heading_ink=dict(x0=heading["x0"], y0=heading["y0"]),
                body_x0=[l["x0"] for l in body[:6]], body_y0=tops[:8], gaps=gaps[:7],
                pitch=min(gaps) if gaps else None, band_h=band,
                fills={str(n): b for n, b in fills.items()},
                wrapped=[n for n in fills if n not in single],
                advance=advance, fill_edge=edge,
                container=container, container_w=container["w"] if container else None,
                container_centre=(container["x0"] + container["x1"] + 1) / 2 if container else None,
                cells_in_container=round(container["w"] / advance, 2) if container and advance else None,
                ax=ax_geometry, frames=names)


def run(w, st, lim, cells):
    tag = f"w{w:04d}-step{st:02d}" + ("" if lim == 64 else f"-limit{lim}")
    region = (0, 33, w, HEIGHT_PT)
    S.REGION = region
    names = names_for(tag, cells)

    S.reset()
    time.sleep(0.5)
    shoot(names["plain"], region)

    for n in cells:
        S.reset()
        time.sleep(0.35)
        S.goto(downs=2, shift_rights=n)
        time.sleep(0.5)
        shoot(names[f"sel{n:02d}"], region)

    S.keys("a", "command down")
    time.sleep(0.6)
    shoot(names["all"], region)

    row = measure(tag, w, st, lim, cells, ax())
    print(json.dumps(row), flush=True)
    return row


def main():
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    path = os.path.join("dev", "ref", "ia", "mac-native", "narrow-344.json")
    kept = {r["tag"]: r for r in (json.load(open(path)) if ONLY and os.path.exists(path) else [])}
    if ONLY == {"--remeasure"}:
        rows = [measure(r["tag"], r["width_pt"], r["step"], r["limit"],
                        [int(n) for n in r["fills"]], r["ax"]) for r in kept.values()]
        json.dump(rows, open(path, "w"), indent=1)
        for r in rows:
            print(json.dumps(r), flush=True)
        return
    rows, at_limit = [], 64
    for w, st, lim, cells in CONFIGS:
        tag = f"w{w:04d}-step{st:02d}" + ("" if lim == 64 else f"-limit{lim}")
        if ONLY and tag not in ONLY:
            if tag in kept:
                rows.append(kept[tag])
            continue
        if lim != at_limit:
            limit(lim)
            at_limit = lim
        width(w)
        step(st)
        rows.append(run(w, st, lim, cells))
    if at_limit != 64:
        limit(64)
    width(1512)
    step(5)
    json.dump(rows, open(path, "w"), indent=1)


if __name__ == "__main__":
    main()
