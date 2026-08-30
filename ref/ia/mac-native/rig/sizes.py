#!/usr/bin/env python3
"""State 11: the passage at every text size the app offers.

The Text Size menu steps rather than names a value, so the range is walked from
the bottom until the geometry stops moving. Per size: the line pitch, the cell
advance measured off a 20-cell selection fill (never by counting characters), the
left edge of the measure, the band's height, and the caret's own box.
"""
import json
import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import fast
import iarig as R
import states as S

OUT, THEME = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
S.REGION = (0, 48, 1470, 700)
FILLC = (17, 61, 82) if THEME == "dark" else (204, 237, 248)


def size(step):
    subprocess.run(["osascript", "-e",
                    'tell application "System Events" to tell process "iA Writer" to click menu item '
                    f'"{step}" of menu 1 of menu item "Text Size" of menu 1 of '
                    'menu bar item "View" of menu bar 1'], capture_output=True)
    time.sleep(0.55)


def burst_fast(out, n=9, iv=0.09):
    fs = []
    for i in range(n):
        f = f"{out}.{i}.png"
        R.shoot(S.REGION, f)
        fs.append(f)
        time.sleep(iv)
    best = max(fs, key=fast.accent_count)
    shutil.copyfile(best, out)
    for f in fs:
        os.remove(f)


def measure(idx):
    S.reset(); time.sleep(0.35)
    plain = f"{OUT}/size-{idx:02d}-plain.png"
    S.shoot_state(plain, burst=False)
    g, ls = fast.ink_lines(plain)
    body = [l for l in ls if 10 < l["h"] < 70]
    tops = [l["y0"] for l in body]
    steps = sorted(b - a for a, b in zip(tops, tops[1:]))
    pitch = steps[0] if steps else None
    left = min((l["x0"] for l in body), default=None)
    inkh = body[1]["h"] if len(body) > 1 else None

    S.goto(downs=2, shift_rights=20); time.sleep(0.45)
    sel = f"{OUT}/size-{idx:02d}-sel.png"
    S.shoot_state(sel, burst=False)
    fb = fast.fill_box(sel, FILLC)
    cell = round((fb["w"] - 2) / 20.0, 3) if fb else None

    S.reset(); S.goto(downs=2, rights=0); time.sleep(0.35)
    car = f"{OUT}/size-{idx:02d}-caret.png"
    burst_fast(car)
    cb = fast.caret_box(car)
    row = dict(step=idx, pitch=pitch, ink_h=inkh, cell=cell,
               band_h=fb["h"] if fb else None, fill_x0=fb["x0"] if fb else None,
               col_left=left, caret_w=cb["w"] if cb else None,
               caret_h=cb["h"] if cb else None, caret_x=cb["centre"] if cb else None)
    print(json.dumps(row), flush=True)
    return row


for _ in range(16):
    size("Make Text Smaller")
rows, prev = [], None
for i in range(18):
    r = measure(i)
    if prev and r["cell"] == prev["cell"] and r["pitch"] == prev["pitch"]:
        print("# top of range reached", flush=True)
        break
    rows.append(r); prev = r
    size("Make Text Bigger")
json.dump(rows, open(f"{OUT}/sizes-{THEME}.json", "w"), indent=1)
print(f"# distinct sizes: {len(rows)}", flush=True)
subprocess.run(["osascript", "-e",
                'tell application "System Events" to tell process "iA Writer" to click menu item '
                '"Make Text Normal Size" of menu 1 of menu item "Text Size" of menu 1 of '
                'menu bar item "View" of menu bar 1'], capture_output=True)
