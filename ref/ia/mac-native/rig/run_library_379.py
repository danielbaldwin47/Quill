#!/usr/bin/env python3
"""#379, state 28: the Library pane — its list, rows, excerpts, search and bars.

No committed capture shows the Library at all: `NOTES.md` § The rig reads
*Library hidden* for every state, and `VERDICTS.md` 4.2.13 has the library list
down as **still unknown**. Quill's own pane is drawn to numbers the JavaScript
app measured off iA's once, second-hand, and never checked against a frame.

`library_fixture_379.py --add` puts `shots/oracle/library/` without its
`manifest.json` into the Library first — six files, a `Drafts` folder with two,
`.archive` with one — so a judged state can pair against a frame by name, and
`sea-storm.md` is the open document. It is **added**, never swapped in: whatever
the Library already held is left where it is, the frames carry those rows too,
and `--remove` takes away the list `--add` wrote and nothing else.

L1 is shot twice, once with the Library hidden. **Not for the pane's width** —
showing the pane reflows the page, so the pair differs nearly everywhere and a
box drawn round that difference is the window; `measure_library_379.py` reads
the width off the pane's own ground instead, and an earlier pass that took it
off this pair aimed its drag 900 pt wrong. The control is here because a state
with no pane is worth having beside one with it. The rest are single frames:
each of them changes what the pane holds, so a control of the same pane is not
a thing that exists.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_library_379.py <raw-dir> <norm-dir>

Run from the repository root, with iA Writer in Mono, System — Default, Normal
text size, limit 64, Preview hidden, Focus, Typewriter, Syntax, Style Check and
Authors off.
"""
import json
import os
import subprocess
import sys
import time

from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import colour
import iarig as R

RAW = NORM = None
REGION = (0, 33, 1512, 949)          # the whole window, in logical points
PREFIX = "mac-native-28"
SETTLE = 1.2
PROC = 'tell application "System Events" to tell process "iA Writer" to '
LIB = os.path.expanduser("~/Library/Mobile Documents/27N4MQEA55~pro~writer/Documents")
OPEN_DOC = "sea-storm.md"


def osa(script, timeout=20):
    """One AppleScript, with a clock on it — a menu left open blocks `activate`."""
    try:
        r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True,
                           timeout=timeout)
        return (r.stdout or r.stderr).strip()
    except subprocess.TimeoutExpired:
        print("  ! osascript timed out:", script[:70], flush=True)
        dismiss()
        return ""


def dismiss():
    for _ in range(2):
        subprocess.run(["osascript", "-e", 'tell application "System Events" to key code 53'],
                       capture_output=True, timeout=10)
        time.sleep(0.25)


def menu_names(bar):
    return osa(PROC + f'return name of every menu item of menu 1 of menu bar item '
                      f'"{bar}" of menu bar 1').split(", ")


def click(bar, item, settle=SETTLE):
    dismiss()
    osa(PROC + f'click menu item "{item}" of menu 1 of menu bar item "{bar}" of menu bar 1')
    time.sleep(settle)


def verb(bar, a, b):
    names = menu_names(bar)
    on = [n for n in (a, b) if n in names]
    assert len(on) == 1, (bar, a, b, names)
    return on[0]


def want(bar, off_verb, on_verb, on):
    have = verb(bar, off_verb, on_verb)
    if (have == off_verb) != on:
        return
    click(bar, have)
    assert verb(bar, off_verb, on_verb) != have, (bar, off_verb, on_verb, on)


def library(on):
    want("View", "Show Library", "Hide Library", on)
    time.sleep(0.8)


def appearance(ground):
    want("View", "Enable Dark Mode", "Disable Dark Mode", ground == "dark")
    time.sleep(1.0)


def open_doc(name=OPEN_DOC):
    osa(f'tell application "iA Writer" to open POSIX file "{os.path.join(LIB, name)}"')
    time.sleep(1.8)
    osa('tell application "iA Writer" to set bounds of window 1 to {0, 33, 1512, 982}')
    time.sleep(0.8)
    osa('tell application "System Events" to key code 126 using {command down}')
    time.sleep(0.5)


def mouse(x_pt, y_pt, kind=None):
    import Quartz
    pos = Quartz.CGPointMake(x_pt, y_pt)
    Quartz.CGEventPost(Quartz.kCGHIDEventTap,
                       Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventMouseMoved, pos, 0))
    time.sleep(0.6)
    if kind:
        down, up, btn = ((Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp,
                          Quartz.kCGMouseButtonLeft) if kind == "left" else
                         (Quartz.kCGEventRightMouseDown, Quartz.kCGEventRightMouseUp,
                          Quartz.kCGMouseButtonRight))
        for k in (down, up):
            Quartz.CGEventPost(Quartz.kCGHIDEventTap,
                               Quartz.CGEventCreateMouseEvent(None, k, pos, btn))
            time.sleep(0.2)
        time.sleep(1.3)


def type_text(s):
    osa(f'tell application "System Events" to keystroke "{s}"')
    time.sleep(0.9)


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


def settings_library(**flags):
    """Set the Library pane of Settings, and read each checkbox back.

    The pane's checkboxes carry names, unlike the Focus menu's lists, so a
    setting can be asked for and confirmed rather than inferred from a frame.
    """
    dismiss()
    if "Library" not in osa(PROC + 'return name of every window'):
        osa(PROC + 'keystroke "," using {command down}')
        time.sleep(1.6)
        w = [x.strip() for x in osa(PROC + 'return name of every window').split(",")][0]
        osa(PROC + f'click button "Library" of toolbar 1 of window "{w}"')
        time.sleep(1.4)
    got = {}
    for name, on in flags.items():
        label = name.replace("_", " ")
        val = osa(PROC + f'return value of checkbox "{label}" of window "Library"')
        if val.isdigit() and (val == "1") != on:
            osa(PROC + f'click checkbox "{label}" of window "Library"')
            time.sleep(0.7)
        got[label] = osa(PROC + f'return value of checkbox "{label}" of window "Library"')
    return got


def close_settings():
    if "Library" in osa(PROC + 'return name of every window'):
        osa(PROC + 'click button 1 of window "Library"')
        time.sleep(1.0)


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    out = {"rig": dict(region_pt=list(REGION), state=28, ticket=379,
                       window_bounds="{0,33,1512,982}",
                       editor="Mono, System - Default, Normal text size, limit 64",
                       library_folder=LIB, open_document=OPEN_DOC,
                       fixture="shots/oracle/library without manifest.json, added to whatever "
                               "the Library already held"),
           "frames": {}, "observations": {}}

    close_settings()
    open_doc()

    for ground in ("light", "dark"):
        print(ground, flush=True)
        appearance(ground)

        # L1: the pane at rest, and the same window with no pane, so the pane's
        # own width and its divider are a difference rather than a guess.
        library(True)
        open_doc()
        frame(out, f"{PREFIX}-{ground}-library-l1-rest.png", ground, state="L1 the pane at rest",
              library="shown", caret="document start", search="empty")
        library(False)
        frame(out, f"{PREFIX}-{ground}-library-l1-rest-nolib.png", ground,
              state="L1 control, the pane hidden", library="hidden", caret="document start")
        library(True)

        # L2: the query `sea` typed into the search field, which `Edit > Find >
        # Filter Library...` puts the caret in.
        dismiss()
        osa(PROC + 'click menu item "Filter Library..." of menu 1 of menu item "Find" of menu 1 '
                   'of menu bar item "Edit" of menu bar 1')
        time.sleep(1.2)
        type_text("sea")
        frame(out, f"{PREFIX}-{ground}-library-l2-search.png", ground,
              state="L2 the query 'sea'", library="shown", search="sea")
        osa('tell application "System Events" to key code 53')
        time.sleep(0.8)

        # L5: sea-storm.md selected and the pointer resting on another row.
        open_doc()
        out["observations"].setdefault("hover_at_pt", [180, 300])
        mouse(180, 300)
        frame(out, f"{PREFIX}-{ground}-library-l5-hover.png", ground,
              state="L5 a row selected and another hovered", library="shown",
              pointer="on a row of the list")
        mouse(760, 500)

    appearance("light")
    open_doc()

    # L3: a folder opened, which under Navigation > Tree expands in place.
    frame(out, f"{PREFIX}-light-library-l3-folder-before.png", "light",
          state="L3 before the folder is opened", library="shown")

    json.dump(out, open(os.path.join("ref", "ia", "mac-native", "library-379.json"), "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
