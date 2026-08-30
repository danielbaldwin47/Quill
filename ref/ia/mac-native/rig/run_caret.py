#!/usr/bin/env python3
"""States 1, 2, 8 and 10: the free caret and the plain selection, one theme."""
import sys
sys.path.insert(0, sys.path[0] or ".")
import iarig as R
import states as S

OUT = sys.argv[1]
THEME = sys.argv[2]


def measure(png, label):
    g, bars = R.feature(png, R.is_bar)
    fp = R.is_fill_dark if THEME == "dark" else R.is_fill_light
    _, fills = R.feature(png, fp)
    fills = [f for f in fills if f["npx"] > 400]
    print(f"\n== {label} == ground={R.hexof(g)}")
    for f in bars:
        a, b = f["runs"][0][0], f["runs"][-1][1]
        print(f"   bar  x={a}..{b-1} w={b-a} y={f['y0']}..{f['y1']} h={f['h']} "
              f"colour={R.hexof(f['colour'])} npx={f['npx']}")
    for f in fills:
        rs = " ".join(f"x{a}..{b-1}(w{b-a})" for a, b in f["runs"])
        print(f"   fill y={f['y0']}..{f['y1']} h={f['h']} colour={R.hexof(f['colour'])} :: {rs}")
    if not bars:
        print("   NO ACCENT PIXEL IN FRAME")
    return g, bars, fills


# 1 — caret mid-word
S.reset(); S.goto(downs=2, rights=5)
p = f"{OUT}/mac-native-01-{THEME}-caret-midword.png"
S.shoot_state(p); measure(p, "01 caret mid-word")

# 2 — caret at end of a line
S.reset(); S.goto(downs=2)
S.keyc(124, "command down")          # end of the wrapped line
import time; time.sleep(0.6)
p = f"{OUT}/mac-native-02-{THEME}-caret-line-end.png"
S.shoot_state(p); measure(p, "02 caret line end")

# 8 — selection inside one row
S.reset(); S.goto(downs=2, shift_rights=18)
p = f"{OUT}/mac-native-08-{THEME}-selection-inline.png"
S.shoot_state(p); measure(p, "08 selection inside one row")

# 10 — selection running through a trailing newline
S.reset(); S.goto(downs=2, rights=52)
S.repeat(124, 20, "shift down")       # runs past the hard line end into the next line
time.sleep(0.6)
p = f"{OUT}/mac-native-10-{THEME}-selection-trailing-newline.png"
S.shoot_state(p); measure(p, "10 selection through a trailing newline")
