#!/usr/bin/env python3
"""#419: walk the Text Size menu one click at a time and read the pitch.

`run_narrow_419.py` reaches each step by counting clicks, so a dropped one shows
up as two steps of one size. This walks the ladder continuously — Normal, then
Smaller to the floor, then Normal, then Bigger to the ceiling — and shoots a
plain frame after every single click, so a repeat here is the app's ladder and
not the driver's. `measure_419.py` reads what it writes and refuses to print a
table whose frames sit off it.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/ladder_419.py <scratch-dir> <window-pt>

The frames are scratch; the readings are kept as
`ref/ia/mac-native/narrow-419-ladder-<window-pt>.json`. Run from the repository
root, light, Mono, limit 64.
"""
import json
import os
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import iarig as R
import run_narrow_419 as N
import states as S

OUT, W = sys.argv[1], int(sys.argv[2])
KEEP = os.path.join("ref", "ia", "mac-native", f"narrow-419-ladder-{W}.json")
os.makedirs(OUT, exist_ok=True)
S.REGION = (0, 33, W, N.HEIGHT_PT)


def read(label):
    png = os.path.join(OUT, f"ladder-{W}-{label}.png")
    R.shoot(S.REGION, png)
    _, lines = N.body_lines(png)
    body = [l for l in lines if l["h"] > 8]
    row = dict(label=label, pitch=N.pitch_of(png), heading_h=body[0]["h"],
               body_h=body[1]["h"] if len(body) > 1 else None,
               body_x0=body[1]["x0"] if len(body) > 1 else None)
    print(json.dumps(row), flush=True)
    return row


N.width(W)
S.reset()
time.sleep(0.5)

rows = []
N.size("Make Text Normal Size", 0.9)
rows.append(read("normal+0"))
for i in range(1, 9):
    N.size("Make Text Smaller", 0.9)
    rows.append(read(f"smaller-{i}"))
N.size("Make Text Normal Size", 0.9)
rows.append(read("normal+0b"))
for i in range(1, 12):
    N.size("Make Text Bigger", 0.9)
    rows.append(read(f"bigger+{i}"))
N.size("Make Text Normal Size", 0.9)
json.dump(rows, open(KEEP, "w"), indent=1)
print(f"{KEEP}: {len(rows)} readings", flush=True)
