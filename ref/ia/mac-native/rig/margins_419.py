#!/usr/bin/env python3
"""#419: the narrowest class's side margin at every step, one width at a time.

The step-5 bisect puts the margin at 16 px in a window up to 390 pt and 26 px
from 391 to 440, where the middle class keeps 10 px at every width; the 400 pt
ladder puts it at 32 px at step 0 falling to 10 px by step 10. Those two
readings cross, so the margin is a function of the window and the size
together. A margin needs only the container's own left edge, which one
select-all frame gives, so the remaining widths are walked cheaply here rather
than shot as states.

The walk is continuous — the floor, then one Bigger click a step — and each step
keeps a **plain** frame beside the select-all one, because a pitch read off a
frame whose text is all selected is the fill's gaps and not the line's. That
plain frame is what puts the row on its rung, and `measure_419.py` refuses the
table if any row sits off it.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/margins_419.py <scratch-dir> <width-pt> ...

Run from the repository root, light, Mono, limit 64.
"""
import json
import os
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import colour
import iarig as R
import run_narrow_419 as N
import states as S

OUT = sys.argv[1]
WIDTHS = [int(w) for w in sys.argv[2:]]
KEEP = os.path.join("ref", "ia", "mac-native", "narrow-419-margins-steps.json")
os.makedirs(OUT, exist_ok=True)


def shoot(w, st, kind):
    png = os.path.join(OUT, f"margin-{w}-{st:02d}-{kind}.png")
    R.shoot(S.REGION, png)
    colour.normalise(png)
    return png


rows = []
for w in WIDTHS:
    bounds = N.width(w)
    S.REGION = (0, 33, w, N.HEIGHT_PT)
    for _ in range(16):
        N.size("Make Text Smaller", 0.35)
    for st in range(14):
        if st:
            N.size("Make Text Bigger", 0.5)
        S.reset()
        time.sleep(0.3)
        plain = shoot(w, st, "plain")
        S.keys("a", "command down")
        time.sleep(0.45)
        b = N.fill_box(shoot(w, st, "all"))
        row = dict(width_pt=w, step=st, window_px=w * 2, got_bounds=bounds,
                   pitch=N.pitch_of(plain), margin=b["x0"] if b else None,
                   container_w=b["w"] if b else None,
                   right_margin=(w * 2 - (b["x0"] + b["w"])) if b else None)
        print(json.dumps(row), flush=True)
        rows.append(row)
json.dump(rows, open(KEEP, "w"), indent=1)
print(f"{KEEP}: {len(rows)} readings", flush=True)
N.width(1512)
N.step(5)
