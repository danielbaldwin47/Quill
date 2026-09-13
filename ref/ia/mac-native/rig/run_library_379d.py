#!/usr/bin/env python3
"""#379, fourth pass: L4's Favorite, and O1's drag aimed at the real divider.

Two states the earlier passes did not deliver, and one of them not honestly.

**L4** asks for a file *made a Favorite*, so that the Favorites row — the Pinned
section's counterpart — is measured. The second and third passes only opened a
row's context menu and shot it; no Favorite was ever made, and the Favorites
section stayed empty prose in every frame.

**O1** asks whether the pane drags. The third pass took the divider from the L1
pair's difference, which is nearly the whole window because showing the pane
reflows the page, and so dragged from **1275 pt** — the middle of the text. The
second pass dragged from 368 pt, 8 pt past the edge. Neither touched the
divider, so neither could say anything, and `divider_drags` was written twice
from two wrong numbers. The divider is at **360 pt**, where the pane's own
ground gives way to the page's, and this drags from there.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_library_379d.py <raw-dir> <norm-dir>

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
import iarig as R
import run_library_379 as N
import measure_library_379 as M

RAW = NORM = None
REGION = N.REGION
PREFIX = N.PREFIX
PROC = N.PROC
STATE = os.path.join("ref", "ia", "mac-native", "library-379.json")


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


def drag(x0, y0, x1, y1, steps=8):
    import Quartz
    seq = [(Quartz.kCGEventMouseMoved, (x0, y0)), (Quartz.kCGEventLeftMouseDown, (x0, y0))]
    seq += [(Quartz.kCGEventLeftMouseDragged, (x0 + (x1 - x0) * i / steps, y0))
            for i in range(1, steps + 1)]
    seq.append((Quartz.kCGEventLeftMouseUp, (x1, y1)))
    for kind, pos in seq:
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, Quartz.CGEventCreateMouseEvent(
            None, kind, Quartz.CGPointMake(*pos), Quartz.kCGMouseButtonLeft))
        time.sleep(0.18)
    time.sleep(1.4)


def name_rows(png, x0=330, x1=700, gap=30):
    """The list's rows, by the ink each name stands on, below the title bar.

    One gap, the reader's: a name and its own excerpt are 16 px apart and one row
    and the next are 40, so anything under 30 reads three bands where there is
    one row.
    """
    import fast
    a = fast.arr(png).astype(int)
    ground = np.median(a[300:1700, x0:x1].reshape(-1, 3), axis=0)
    ink = (np.abs(a[:, x0:x1] - ground).sum(axis=2) > 120).sum(axis=1) >= 3
    ink[:180] = False
    ys = np.where(ink)[0]
    out, prev = [], None
    for y in ys:
        if prev is None or y - prev > gap:
            out.append([int(y), int(y)])
        else:
            out[-1][1] = int(y)
        prev = y
    return [r for r in out if r[1] - r[0] >= 6]


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    out = json.load(open(STATE))
    rest = os.path.join(NORM, f"{PREFIX}-light-library-l1-rest.png")
    pane = M.pane_of(f"{PREFIX}-light-library-l1-rest.png")
    divider_pt = (pane["pane_right_px"] + 1) / 2
    rows = name_rows(rest)
    out["observations"]["divider_pt"] = divider_pt
    print(f"  divider at {divider_pt} pt; {len(rows)} name rows", flush=True)

    N.appearance("light")
    N.library(True)
    N.open_doc()

    # L4: harbour-lights.md made a Favorite, so the Favorites section is a row
    # rather than the prose it shows while empty.
    target = 5                       # harbour-lights.md, counting Drafts as row 1
    y = (rows[target][0] + rows[target][1]) / 2
    N.mouse(200, y / 2 + REGION[1], "right")
    time.sleep(0.6)
    got = N.osa(PROC + 'click menu item "Favorite" of menu 1 of window 1')
    if "menu item Favorite" not in got:
        # The row menu is not a window's menu; press it where it stands instead.
        N.osa(PROC + 'keystroke "f"')
    time.sleep(1.6)
    N.dismiss()
    out["observations"]["favorite_click"] = got[:120]
    frame(out, f"{PREFIX}-light-library-l4-favorite.png", "light",
          state="L4 harbour-lights.md made a Favorite", library="shown",
          favorited=f"the row at y {y}")

    # O1: the divider, aimed at the column the pane's own ground gives way at.
    before = pane["pane_width_pt"]
    drag(divider_pt, 500, divider_pt + 140, 500)
    got_png = frame(out, f"{PREFIX}-light-library-o1-dragged.png", "light",
                    state="O1 the divider dragged 140 pt right", library="shown",
                    dragged_from_pt=divider_pt)
    after = M.pane_of(os.path.basename(got_png))["pane_width_pt"]
    out["observations"]["pane_width_pt"] = before
    out["observations"]["pane_width_after_drag_pt"] = after
    out["observations"]["divider_drags"] = bool(after is not None and abs(after - before) > 2)
    print(f"  {before} pt -> {after} pt (drags={out['observations']['divider_drags']})", flush=True)

    drag(divider_pt + 140 if out["observations"]["divider_drags"] else divider_pt, 500,
         divider_pt, 500)
    back = frame(out, f"{PREFIX}-light-library-o1-restored.png", "light",
                 state="O1 the divider dragged back", library="shown")
    out["observations"]["pane_width_restored_pt"] = M.pane_of(os.path.basename(back))[
        "pane_width_pt"]

    json.dump(out, open(STATE, "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
