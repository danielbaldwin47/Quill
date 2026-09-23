#!/usr/bin/env python3
"""State 17: what colour iA Writer draws a Markdown marker in, at rest.

`passage-markup.md` holds every mark kind #198 asks about in one screenful, so
one frame per ground carries the whole reading. The caret is driven to the end
of the document before every frame — the caret's own line may lift its marker
ink, and resting ink is what this measures — and one control frame per ground
puts the caret back on the heading so the lift can be seen or ruled out.

    python3 rig/run_markup.py <output-dir>

Run from the repository root, with the passage already open in the app.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import colour
import iarig as R
import states as S

OUT = sys.argv[1]
REGION = (0, 90, 1470, 800)          # logical points: the whole passage, no chrome
PASSAGE = "dev/ref/ia/mac-native/passage-markup.md"


def osa(script):
    return subprocess.run(["osascript", "-e", script],
                          capture_output=True, text=True).stdout.strip()


def view_items():
    return osa('tell application "System Events" to tell process "iA Writer" to '
               'return name of every menu item of menu 1 of menu bar item "View" '
               'of menu bar 1')


def theme():
    return "dark" if "Disable Dark Mode" in view_items() else "light"


def set_theme(want):
    if theme() == want:
        return
    item = "Disable Dark Mode" if want == "light" else "Enable Dark Mode"
    osa(f'tell application "System Events" to tell process "iA Writer" to '
        f'click menu item "{item}" of menu 1 of menu bar item "View" of menu bar 1')
    time.sleep(1.2)


def park_at_end():
    S.keyc(125, "command down")       # end of document
    time.sleep(0.6)


def park_on_heading():
    S.keyc(126, "command down")       # top of document, the H1's own line
    time.sleep(0.6)


def shoot(name, ground):
    p = os.path.join(OUT, name)
    os.makedirs(OUT, exist_ok=True)
    R.shoot(REGION, p)
    colour.normalise(p)
    colour.check(p, ground)
    return p


S.SAMPLE = PASSAGE
S.REGION = REGION

os.makedirs(OUT, exist_ok=True)
for ground in ("dark", "light"):
    set_theme(ground)
    S.reset()
    park_at_end()
    print("rest   ", shoot(f"mac-native-17-{ground}-marks.png", ground))
    park_on_heading()
    print("caret  ", shoot(f"mac-native-17-{ground}-marks-caret-on-heading.png", ground))
set_theme("dark")
