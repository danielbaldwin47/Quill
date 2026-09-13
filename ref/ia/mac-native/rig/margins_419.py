#!/usr/bin/env python3
"""#419: the narrowest class's side margin at every step, one width at a time.

The step-5 bisect puts the margin at 16 px in a window up to 390 pt and 26 px
from 391 to 440, where the middle class keeps 10 px at every width; the 400 pt
ladder puts it at 32 px at step 0 falling to 10 px by step 10. Those two
readings cross, so the margin is a function of the window and the size
together. A margin needs only the container's own left edge, which one
select-all frame gives, so the remaining widths are walked cheaply here rather
than shot as states.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/margins_419.py <scratch-dir> <width-pt> ...

Run from the repository root, light, Mono, limit 64.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast
import iarig as R
import states as S

OUT = sys.argv[1]
WIDTHS = [int(w) for w in sys.argv[2:]]
FILL = (204, 237, 248)
os.makedirs(OUT, exist_ok=True)


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout


def item(name):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{name}" of menu 1 of menu item "Text Size" of menu 1 of menu bar item "View" '
        f'of menu bar 1')
    time.sleep(0.45)


rows = []
for w in WIDTHS:
    osa(f'tell application "iA Writer" to set bounds of window 1 to {{0, 33, {w}, 982}}')
    time.sleep(0.9)
    S.REGION = (0, 33, w, 500)
    for _ in range(16):
        item("Make Text Smaller")
    for st in range(14):
        if st:
            item("Make Text Bigger")
        S.reset()
        time.sleep(0.3)
        S.keys("a", "command down")
        time.sleep(0.45)
        png = os.path.join(OUT, f"margin-{w}-{st:02d}.png")
        R.shoot(S.REGION, png)
        colour.normalise(png)
        b = fast.fill_box(png, FILL)
        g, lines = fast.ink_lines(png)
        body = [l for l in lines if not (l["y0"] < 30 and l["x1"] - l["x0"] > 0.9 * w * 2)]
        tops = [l["y0"] for l in body[1:]]
        gaps = [b2 - a for a, b2 in zip(tops, tops[1:])]
        row = dict(width_pt=w, step=st, window_px=w * 2, pitch=min(gaps) if gaps else None,
                   container_w=b["w"] if b else None, margin=b["x0"] if b else None,
                   right_margin=(w * 2 - (b["x0"] + b["w"])) if b else None)
        print(json.dumps(row), flush=True)
        rows.append(row)
json.dump(rows, open(os.path.join("ref", "ia", "mac-native", "narrow-419-margins-steps.json"), "w"),
          indent=1)
osa('tell application "iA Writer" to set bounds of window 1 to {0, 33, 1512, 982}')
