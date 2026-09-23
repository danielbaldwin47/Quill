#!/usr/bin/env python3
"""#379, state 28, second pass: the states that need the pane's own geometry.

`run_library_379.py` shoots the pane at rest, the search and the hover, and the
first of those is what says where everything is. This pass reads the pane out of
that frame and then drives it: a folder opened, a file made a Favorite, the Sort
control's menu, the pane with its excerpts and bars off, Quick Search, and the
divider dragged.

Every click is aimed at a row found in the frame rather than at a number written
here, because where a row falls is the list's answer and not this file's.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/run_library_379b.py <raw-dir> <norm-dir>

Run from the repository root, after the first pass.
"""
import json
import os
import subprocess
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
STATE = os.path.join("dev", "ref", "ia", "mac-native", "library-379.json")


def rows_of(png, x0, x1):
    """The list's rows, as bands of ink inside one column of the pane."""
    a = fast.arr(png).astype(int)
    ground = np.median(a[200:1700, x0:x1].reshape(-1, 3), axis=0)
    ink = (np.abs(a[:, x0:x1] - ground).sum(axis=2) > 60).sum(axis=1) >= 3
    ys = np.where(ink)[0]
    out, prev = [], None
    for y in ys:
        if prev is None or y - prev > 10:
            out.append([int(y), int(y)])
        else:
            out[-1][1] = int(y)
        prev = y
    return ground, out


def shoot(name):
    raw = os.path.join(RAW, name)
    R.shoot(REGION, raw)
    norm = os.path.join(NORM, name)
    colour.normalise(raw, norm)
    return norm


def frame(out, name, ground, check=True, **extra):
    norm = shoot(name)
    if check:
        colour.check(norm, ground)
    im = Image.open(norm)
    assert im.size == (2 * REGION[2], 2 * REGION[3]), (name, im.size)
    out["frames"][name] = dict(ground=ground, colour_check="pass" if check else "n/a", **extra)
    print(f"  {name}", flush=True)
    return norm


def drag(x0, y0, x1, y1):
    """Drag the divider, which no menu item and no AppleScript can move."""
    import Quartz
    for kind, pos in ((Quartz.kCGEventMouseMoved, (x0, y0)),
                      (Quartz.kCGEventLeftMouseDown, (x0, y0)),
                      (Quartz.kCGEventLeftMouseDragged, ((x0 + x1) / 2, y0)),
                      (Quartz.kCGEventLeftMouseDragged, (x1, y1)),
                      (Quartz.kCGEventLeftMouseUp, (x1, y1))):
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(
            None, kind, Quartz.CGPointMake(*pos), Quartz.kCGMouseButtonLeft))
        time.sleep(0.25)
    time.sleep(1.2)


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    out = json.load(open(STATE))
    rest = os.path.join(NORM, f"{PREFIX}-light-library-l1-rest.png")

    # The File List's own column, and the rows in it.
    ground, bands = rows_of(rest, 560, 700)
    out["observations"]["list_bands"] = bands[:20]
    print("  list bands:", bands[:12], flush=True)

    def click_row(i, kind="left"):
        y = (bands[i][0] + bands[i][1]) / 2
        N.mouse(240, y / 2 + REGION[1], kind)
        return y

    N.appearance("light")
    N.library(True)
    N.open_doc()

    # L3: the first row of the list is the `Drafts` folder; opening it is a click.
    y = click_row(0)
    out["observations"]["drafts_row_y_px"] = y
    frame(out, f"{PREFIX}-light-library-l3-folder.png", "light",
          state="L3 the Drafts folder opened", library="shown", clicked=f"row 1 at y {y}")
    # Back out, however the app got in.
    N.dismiss()
    N.osa(PROC + 'click menu item "Back in Library" of menu 1 of menu bar item "Go" of menu bar 1')
    time.sleep(1.4)
    frame(out, f"{PREFIX}-light-library-l3-back.png", "light",
          state="L3 the way back out of the folder", library="shown")

    # L4: a file made a Favorite, through the row's own context menu.
    N.open_doc()
    ground2, bands2 = rows_of(os.path.join(NORM, f"{PREFIX}-light-library-l3-back.png"), 560, 700)
    target = min(range(len(bands2)), key=lambda i: abs(bands2[i][0] - 830))
    yy = click_row(target, "right")
    frame(out, f"{PREFIX}-light-library-l4-row-menu.png", "light",
          state="L4 a file row's own context menu", library="shown",
          clicked=f"right-click at y {yy}")
    N.dismiss()

    # L6: the Sort control's menu, the pill under the File List's title bar.
    N.open_doc()
    N.mouse(210, 97, "left")
    frame(out, f"{PREFIX}-light-library-l6-sort-menu.png", "light",
          state="L6 the Sort control's menu", library="shown")
    N.dismiss()

    # L8: Quick Search, which is a surface of its own rather than this field.
    N.open_doc()
    N.dismiss()
    N.osa(PROC + 'click menu item "Quick Search" of menu 1 of menu bar item "Go" of menu bar 1')
    time.sleep(1.4)
    N.type_text("sea")
    frame(out, f"{PREFIX}-light-library-l8-quick-search.png", "light",
          state="L8 Quick Search on 'sea'", library="shown", search="sea")
    N.dismiss()

    # O2: a query only the contents match, against L2's, which the names match.
    N.open_doc()
    N.dismiss()
    N.osa(PROC + 'click menu item "Filter Library..." of menu 1 of menu item "Find" of menu 1 '
                 'of menu bar item "Edit" of menu bar 1')
    time.sleep(1.2)
    N.type_text("the")
    frame(out, f"{PREFIX}-light-library-o2-search-the.png", "light",
          state="O2 the query 'the', which no name holds", library="shown", search="the")
    N.dismiss()

    # L7: the pane with its excerpts and its two bars off.
    N.open_doc()
    out["observations"]["settings_off"] = N.settings_library(
        Text_excerpts=False, Sort_bar=False, Filter_bar=False)
    N.close_settings()
    time.sleep(1.0)
    frame(out, f"{PREFIX}-light-library-l7-bare.png", "light",
          state="L7 excerpts off, Sort and Filter bars hidden", library="shown")
    out["observations"]["settings_back"] = N.settings_library(
        Text_excerpts=True, Sort_bar=True, Filter_bar=True)
    N.close_settings()

    # O1: does the divider drag? The pane's width is read from the L1 pair; this
    # asks only whether it moves, and is put back.
    N.open_doc()
    drag(368, 500, 468, 500)
    frame(out, f"{PREFIX}-light-library-o1-dragged.png", "light",
          state="O1 the divider dragged 100 pt right", library="shown")
    drag(468, 500, 368, 500)
    frame(out, f"{PREFIX}-light-library-o1-restored.png", "light",
          state="O1 the divider dragged back", library="shown")

    json.dump(out, open(STATE, "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
