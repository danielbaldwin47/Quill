#!/usr/bin/env python3
"""#419: bisect the window width the narrowest class's margin changes at.

The step-5 widths #419 asks for put the container 16 px inside a 240 and a
320 pt window and 26 px inside a 400 and a 440 pt one, where the middle class
keeps 10 px at every width. One select-all frame a width is enough to read a
margin, so the break is bisected the way `sweep_narrow.py` bisects the size
classes, and its frames are scratch.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/sweep_419.py <scratch-dir> [lo hi]

Run from the repository root, light, Mono, step 5, limit 64.
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
LO, HI = (int(sys.argv[2]), int(sys.argv[3])) if len(sys.argv) > 3 else (320, 400)
FILL = (204, 237, 248)
os.makedirs(OUT, exist_ok=True)


def width(w):
    subprocess.run(["osascript", "-e", f'tell application "iA Writer" to set bounds of window 1 '
                                       f'to {{0, 33, {w}, 982}}'], capture_output=True)
    time.sleep(0.8)


def margin(w):
    """The container's own left edge inside a window this wide, in device px."""
    width(w)
    S.REGION = (0, 33, w, 500)
    S.reset()
    time.sleep(0.35)
    S.keys("a", "command down")
    time.sleep(0.5)
    raw = os.path.join(OUT, f"scratch-{w:04d}.png")
    R.shoot(S.REGION, raw)
    colour.normalise(raw)
    b = fast.fill_box(raw, FILL)
    row = dict(width_pt=w, window_px=w * 2, x0=b["x0"] if b else None,
               container_w=b["w"] if b else None,
               margin=b["x0"] if b else None,
               right_margin=(w * 2 - (b["x0"] + b["w"])) if b else None)
    print(json.dumps(row), flush=True)
    return row


rows = [margin(w) for w in (200, 240, 280, 320)]
lo, hi = LO, HI
seen = {r["width_pt"]: r for r in rows}
for w in (lo, hi):
    if w not in seen:
        seen[w] = margin(w)
        rows.append(seen[w])
below = seen[lo]["margin"]
while hi - lo > 1:
    mid = (lo + hi) // 2
    r = margin(mid)
    rows.append(r)
    if r["margin"] == below:
        lo = mid
    else:
        hi = mid
print(json.dumps(dict(break_at=[lo, hi], margin_below=below,
                      margin_above=seen[HI]["margin"])), flush=True)
json.dump(rows, open(os.path.join("ref", "ia", "mac-native", "narrow-419-margins.json"), "w"),
          indent=1)
width(1512)
