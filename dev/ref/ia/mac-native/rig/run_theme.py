#!/usr/bin/env python3
"""Every caret-and-selection state, for whichever appearance the app is in.

Run once per theme. The fill is matched by colour within a tolerance rather than
by a hue rule, because the light theme's band and the dark theme's band are not
the same kind of colour and one predicate for both reads glyph edges as fill.
"""
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import iarig as R
import states as S

OUT, THEME = sys.argv[1], sys.argv[2]
SAMPLE = "dev/ref/sample.md"
PASSAGE = "dev/ref/ia/mac-native/passage-blocks.md"


def near(c, tol=14):
    return lambda p: all(abs(p[i] - c[i]) <= tol for i in range(3))


FILL = near((17, 61, 82)) if THEME == "dark" else near((204, 237, 248))


def deact():
    subprocess.run(["osascript", "-e", 'tell application "Finder" to close every window'],
                   capture_output=True)
    subprocess.run(["osascript", "-e", 'tell application "Finder" to activate'],
                   capture_output=True)
    time.sleep(1.6)


def bars_of(p):
    _, bs = R.feature(p, R.is_bar)
    return [(f["runs"][0][0], f["runs"][-1][1], f["y0"], f["y1"], f["colour"]) for f in bs]


def fills_of(p, pred=None, minpx=400):
    _, fs = R.rowbands(p, pred or FILL, gap=2)
    return [(f["runs"][0][0], f["runs"][-1][1], f["y0"], f["y1"], f["colour"], f["npx"])
            for f in fs if f["npx"] > minpx]


def line(tag, p):
    g, _ = R.ground_of(p)
    bs, fs = bars_of(p), fills_of(p)
    print(f"\n[{tag}] ground={R.hexof(g)}")
    for a, b, y0, y1, c in bs:
        print(f"    bar  x={a}..{b-1} w={b-a} y={y0}..{y1} h={y1-y0+1} {R.hexof(c)} "
              f"centre={(a+b)/2}")
    for a, b, y0, y1, c, n in fs:
        print(f"    fill x={a}..{b-1} w={b-a} y={y0}..{y1} h={y1-y0+1} {R.hexof(c)}")
    if not bs:
        print("    NO ACCENT PIXEL IN FRAME")


S.SAMPLE = SAMPLE
S.REGION = (0, 48, 1470, 380)

S.reset(); S.goto(downs=2, rights=5)
p = f"{OUT}/mac-native-01-{THEME}-caret-midword.png"; S.shoot_state(p); line("01 caret mid-word", p)

S.reset(); S.goto(downs=7); S.keyc(124, "command down"); time.sleep(0.6)
p = f"{OUT}/mac-native-02-{THEME}-caret-line-end.png"; S.shoot_state(p); line("02 caret line end", p)

S.reset(empty=True); time.sleep(0.5)
p = f"{OUT}/mac-native-03-{THEME}-caret-empty-document.png"; S.shoot_state(p); line("03 caret empty doc", p)

S.reset(); S.goto(downs=2, rights=5); time.sleep(0.4); deact()
p = f"{OUT}/mac-native-06-{THEME}-deactivated-caret.png"
best, sc = R.burst(S.REGION, p); line("06 deactivated caret", p)
print(f"    burst accent scores: {sc}")

S.reset(); S.goto(downs=2, shift_rights=18); time.sleep(0.4); deact()
p = f"{OUT}/mac-native-07-{THEME}-deactivated-selection.png"; S.shoot_state(p, burst=False)
idle = near((70, 70, 70), 8) if THEME == "dark" else None
line("07 deactivated selection", p)
if idle:
    for a, b, y0, y1, c, n in fills_of(p, idle, 4000):
        print(f"    idle fill x={a}..{b-1} w={b-a} y={y0}..{y1} h={y1-y0+1} {R.hexof(c)}")

S.reset(); S.goto(downs=2, shift_rights=18)
p = f"{OUT}/mac-native-08-{THEME}-selection-inline.png"; S.shoot_state(p, burst=False)
line("08 selection inside one row", p)

S.reset(); S.goto(downs=7); S.keyc(124, "command down"); time.sleep(0.4)
S.repeat(124, 1, "shift down"); time.sleep(0.6)
p = f"{OUT}/mac-native-10-{THEME}-selection-trailing-newline.png"; S.shoot_state(p, burst=False)
line("10 selection through a hard newline", p)
