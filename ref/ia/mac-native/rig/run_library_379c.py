#!/usr/bin/env python3
"""#379, state 28, third pass: the three states the second pass aimed wrong.

The second pass took the first ink band of a column for the list's first row; it
is the File List's own title bar, so the folder click and the row's context menu
both landed there. And it dragged from 8 pt right of the divider, which is the
page rather than the divider, so the pane did not move and the drag said nothing.

This pass finds the divider as the column the pane's ground gives way at, and
finds a row by the y a name's ink stands on rather than by its place in a list
of bands.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_library_379c.py <raw-dir> <norm-dir>

Run from the repository root, after the first pass.
"""
import json
import os
import sys
import time

import numpy as np
from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast
import iarig as R
import run_library_379 as N

RAW = NORM = None
REGION = N.REGION
PREFIX = N.PREFIX
PROC = N.PROC
STATE = os.path.join("ref", "ia", "mac-native", "library-379.json")


def divider(rest, nolib):
    """The column the pane gives way to the page at, off the L1 pair.

    The two frames are the same window with and without the pane, so the last
    column they differ on is the pane's own right edge — which is the one number
    a drag has to be aimed at, and the one #379 asks for as the pane's width.
    """
    a, b = fast.arr(rest).astype(int), fast.arr(nolib).astype(int)
    d = np.abs(a - b).sum(axis=2) > 20
    cols = np.where(d[200:1700].any(axis=0))[0]
    return int(cols.min()), int(cols.max())


def name_rows(rest, x0=330, x1=700):
    """The list's rows, by the ink each name stands on, below the title bar."""
    a = fast.arr(rest).astype(int)
    ground = np.median(a[300:1700, x0:x1].reshape(-1, 3), axis=0)
    ink = (np.abs(a[:, x0:x1] - ground).sum(axis=2) > 120).sum(axis=1) >= 3
    ink[:180] = False                # the title bar and the Sort pill are above
    ys = np.where(ink)[0]
    out, prev = [], None
    for y in ys:
        if prev is None or y - prev > 14:
            out.append([int(y), int(y)])
        else:
            out[-1][1] = int(y)
        prev = y
    return [r for r in out if r[1] - r[0] >= 6]


def shoot(name):
    raw = os.path.join(RAW, name)
    R.shoot(REGION, raw)
    norm = os.path.join(NORM, name)
    colour.normalise(raw, norm)
    return norm


def frame(out, name, ground, **extra):
    norm = shoot(name)
    colour.check(norm, ground)
    im = Image.open(norm)
    assert im.size == (2 * REGION[2], 2 * REGION[3]), (name, im.size)
    out["frames"][name] = dict(ground=ground, colour_check="pass", **extra)
    print(f"  {name}", flush=True)
    return norm


def drag(x0, y0, x1, y1):
    import Quartz
    steps = [(Quartz.kCGEventMouseMoved, (x0, y0)), (Quartz.kCGEventLeftMouseDown, (x0, y0))]
    for i in range(1, 6):
        steps.append((Quartz.kCGEventLeftMouseDragged, (x0 + (x1 - x0) * i / 5, y0)))
    steps.append((Quartz.kCGEventLeftMouseUp, (x1, y1)))
    for kind, pos in steps:
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(
            None, kind, Quartz.CGPointMake(*pos), Quartz.kCGMouseButtonLeft))
        time.sleep(0.2)
    time.sleep(1.4)


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    out = json.load(open(STATE))
    rest = os.path.join(NORM, f"{PREFIX}-light-library-l1-rest.png")
    nolib = os.path.join(NORM, f"{PREFIX}-light-library-l1-rest-nolib.png")

    x0, x1 = divider(rest, nolib)
    rows = name_rows(rest)
    out["observations"]["pane_cols_px"] = [x0, x1]
    out["observations"]["pane_width_pt"] = round((x1 - x0 + 1) / 2, 1)
    out["observations"]["name_rows_px"] = rows[:14]
    print(f"  pane cols {x0}…{x1} = {out['observations']['pane_width_pt']} pt", flush=True)
    print(f"  name rows {rows[:10]}", flush=True)

    def click(i, kind="left"):
        y = (rows[i][0] + rows[i][1]) / 2
        N.mouse(200, y / 2 + REGION[1], kind)
        return y

    N.appearance("light")
    N.library(True)
    N.open_doc()

    # L3: the first row of the list is the `Drafts` folder.
    y = click(0)
    out["observations"]["drafts_row_px"] = y
    frame(out, f"{PREFIX}-light-library-l3-folder.png", "light",
          state="L3 the Drafts folder opened", library="shown", clicked=f"first name row, y {y}")
    N.dismiss()
    N.osa(PROC + 'click menu item "Back in Library" of menu 1 of menu bar item "Go" of menu bar 1')
    time.sleep(1.6)
    frame(out, f"{PREFIX}-light-library-l3-back.png", "light",
          state="L3 the way back out of the folder", library="shown")

    # L4: a file row's own context menu, which is where a Favorite is made.
    N.open_doc()
    yy = click(5, "right")
    out["observations"]["row_menu_at_px"] = yy
    frame(out, f"{PREFIX}-light-library-l4-row-menu.png", "light",
          state="L4 a file row's own context menu", library="shown",
          clicked=f"right-click on a name row, y {yy}")
    N.dismiss()

    # O1: the divider, aimed at the column the pair says it is on.
    N.open_doc()
    dx = (x1 + 1) / 2
    drag(dx, 500, dx + 120, 500)
    got = frame(out, f"{PREFIX}-light-library-o1-dragged.png", "light",
                state="O1 the divider dragged 120 pt right", library="shown",
                dragged_from_pt=dx)
    N.library(False)
    after_nolib = shoot(f"{PREFIX}-light-library-o1-dragged-nolib.png")
    N.library(True)
    gx0, gx1 = divider(got, after_nolib)
    out["observations"]["pane_cols_after_drag_px"] = [gx0, gx1]
    out["observations"]["pane_width_after_drag_pt"] = round((gx1 - gx0 + 1) / 2, 1)
    out["observations"]["divider_drags"] = bool(abs(gx1 - x1) > 4)
    print(f"  after the drag: {out['observations']['pane_width_after_drag_pt']} pt "
          f"(drags={out['observations']['divider_drags']})", flush=True)
    drag(dx + 120, 500, dx, 500)
    frame(out, f"{PREFIX}-light-library-o1-restored.png", "light",
          state="O1 the divider dragged back", library="shown")

    json.dump(out, open(STATE, "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
