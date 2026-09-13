#!/usr/bin/env python3
"""#419: walk the Text Size menu one click at a time and read the pitch.

`run_narrow_419.py` reaches each step from Make Text Normal Size, so a dropped
click shows up as two steps of one size. This walks the ladder continuously —
Normal, then Smaller to the floor, then Normal, then Bigger to the ceiling — and
shoots a plain frame after every single click, so a repeat here is the app's
ladder and not the driver's.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/ladder_419.py <scratch-dir> <window-pt>

Run from the repository root, light, Mono, limit 64.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import fast
import iarig as R
import states as S

OUT, W = sys.argv[1], int(sys.argv[2])
os.makedirs(OUT, exist_ok=True)
S.REGION = (0, 33, W, 500)


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout


def item(name):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{name}" of menu 1 of menu item "Text Size" of menu 1 of menu bar item "View" '
        f'of menu bar 1')
    time.sleep(0.9)


def read(label):
    png = os.path.join(OUT, f"ladder-{W}-{label}.png")
    R.shoot(S.REGION, png)
    g, lines = fast.ink_lines(png)
    wide = 0.9 * fast.arr(png).shape[1]
    body = [l for l in lines if not (l["y0"] < 30 and l["x1"] - l["x0"] > wide)]
    tops = [l["y0"] for l in body[1:]]
    gaps = [b - a for a, b in zip(tops, tops[1:])]
    row = dict(label=label, pitch=min(gaps) if gaps else None,
               heading_h=body[0]["h"], body_h=body[1]["h"] if len(body) > 1 else None,
               body_x0=body[1]["x0"] if len(body) > 1 else None)
    print(json.dumps(row), flush=True)
    return row


osa(f'tell application "iA Writer" to set bounds of window 1 to {{0, 33, {W}, 982}}')
time.sleep(0.9)
S.reset()
time.sleep(0.5)

rows = []
item("Make Text Normal Size")
rows.append(read("normal+0"))
for i in range(1, 9):
    item("Make Text Smaller")
    rows.append(read(f"smaller-{i}"))
item("Make Text Normal Size")
rows.append(read("normal+0b"))
for i in range(1, 12):
    item("Make Text Bigger")
    rows.append(read(f"bigger+{i}"))
item("Make Text Normal Size")
json.dump(rows, open(os.path.join(OUT, f"ladder-{W}.json"), "w"), indent=1)
