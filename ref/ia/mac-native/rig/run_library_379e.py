#!/usr/bin/env python3
"""#379, fifth pass: make the Favorite, which no click could reach.

L4 asks for a file *made a Favorite*, so that the Favorites section is a row
rather than the prose it carries while empty. A row's context menu is where a
Favorite is made — and that menu is **not in the accessibility tree**: with it
open the process reports zero menus, and `click menu item "Favorite" of …`
fails whatever it is asked of. So it is driven the way a hand drives it, with
the arrow keys, and the frame is what says whether it took: the Favorites
section carries a row afterwards or it does not.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_library_379e.py <raw-dir> <norm-dir>

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
import measure_library_379 as M
import run_library_379 as N

RAW = NORM = None
REGION = N.REGION
PREFIX = N.PREFIX
STATE = os.path.join("ref", "ia", "mac-native", "library-379.json")
FAVOURITE_ITEM = 5                   # Open in New Tab, Open in New Window, Get Info, Favorite


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


def organiser_ink(png, right_px):
    """How much ink the Organizer column holds — a Favorites row shows up as more."""
    a = fast.arr(png).astype(int)
    ground = np.median(a[300:1700, 20:right_px - 10].reshape(-1, 3), axis=0)
    return int((np.abs(a[200:1700, 20:right_px - 10] - ground).sum(axis=2) > 90).sum())


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    out = json.load(open(STATE))
    pane = M.pane_of(f"{PREFIX}-light-library-l1-rest.png")
    right = pane["organiser_right_px"]

    N.appearance("light")
    N.library(True)
    N.open_doc()
    before_png = os.path.join(NORM, f"{PREFIX}-light-library-l1-rest.png")
    before = organiser_ink(before_png, right)

    # harbour-lights.md, the sixth name row of the list at rest.
    N.dismiss()
    N.mouse(200, 448, "right")
    time.sleep(1.0)
    for _ in range(FAVOURITE_ITEM):
        N.osa('tell application "System Events" to key code 125')
        time.sleep(0.25)
    N.osa('tell application "System Events" to key code 36')
    time.sleep(1.8)
    N.dismiss()
    N.mouse(760, 500)

    got = frame(out, f"{PREFIX}-light-library-l4-favorite.png", "light",
                state="L4 harbour-lights.md made a Favorite", library="shown",
                favorited="the sixth name row, through its context menu by keyboard")
    after = organiser_ink(got, right)
    out["observations"]["organiser_ink_before"] = before
    out["observations"]["organiser_ink_after"] = after
    out["observations"]["favorite_took"] = bool(after > before * 1.05)
    print(f"  Organizer ink {before} -> {after} (took={out['observations']['favorite_took']})",
          flush=True)

    json.dump(out, open(STATE, "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
