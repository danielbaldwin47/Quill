#!/usr/bin/env python3
"""#354, state 24: the Style Check strikethrough, on both grounds and under
Focus, Syntax highlight and a selection.

No committed capture has Style Check on, so the mark's colour, thickness and
position have never been measured. This shoots the passage `ref/style.md` in
every state the ticket names, each beside a control frame with Style Check off
and everything else the same, so "what the mark did" is a difference between two
frames rather than a reading of one.

The Focus menu will not say which lists are on: the four list items carry no
`AXMenuItemMarkChar` in either state, and the menu drawn open shows no check
beside them either. The four parents *are* verbs — "Enable/Disable Style Check",
"Enable/Disable Focus Mode", "Show/Hide Syntax", "Show/Hide Authors" — so a
parent's state is read off its name. A list's state is therefore measured, not
asked: a click is kept only if the frame moved the way the click should move it,
which is what `set_list()` does, and `all_lists_off()` walks the four from
unknown to known the same way.

    python3 rig/run_style.py <raw-dir> <normalised-dir>

Run from the repository root, with iA Writer open on a scratch document, Mono,
System - Default typography, Normal text size, Library and Preview hidden,
Typewriter off, and the window at {0, 33, 1512, 982}.
"""
import json
import os
import shutil
import subprocess
import sys
import time

import numpy as np
from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast
import iarig as R
import states as S

RAW, NORM = sys.argv[1], sys.argv[2]
REGION = (0, 33, 1512, 949)          # the whole window, in logical points
SAMPLE = "ref/style.md"
SETTLE = 2.6                         # a Focus-menu toggle re-tags the document asynchronously
PROC = 'tell application "System Events" to tell process "iA Writer" to '

# The caret and the selection, as offsets on the second body row,
# "of ready to get down to brass tacks. Against all odds the team".
ROW2 = 3                             # down-presses from the top of the document
AGAINST = 36                         # "Against" starts here
SEL_CELLS = len("Against all odds")
CARET_IN_S2 = AGAINST + len("Against all odds t")   # inside "the", an unstruck word


def osa(script):
    r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True)
    return (r.stdout or r.stderr).strip()


def menu_names(bar):
    return osa(PROC + f'return name of every menu item of menu 1 of menu bar item '
                      f'"{bar}" of menu bar 1').split(", ")


def click(bar, item, settle=SETTLE):
    osa(PROC + f'click menu item "{item}" of menu 1 of menu bar item "{bar}" of menu bar 1')
    time.sleep(settle)


def verb(bar, a, b):
    """Whichever of two verbs the menu is showing — the state of what it toggles."""
    names = menu_names(bar)
    on = [n for n in (a, b) if n in names]
    assert len(on) == 1, (bar, a, b, names)
    return on[0]


def want(bar, off_verb, on_verb, on):
    """`off_verb` is the verb shown while the thing is off."""
    have = verb(bar, off_verb, on_verb)
    if (have == off_verb) != on:
        return
    click(bar, have)
    assert verb(bar, off_verb, on_verb) != have, (bar, off_verb, on_verb, on)


def style(on):
    want("Focus", "Enable Style Check", "Disable Style Check", on)


def focus(on):
    want("Focus", "Enable Focus Mode", "Disable Focus Mode", on)


def syntax(on):
    want("Focus", "Show Syntax", "Hide Syntax", on)


def list_names():
    """The four list items, taken by position so no accented name is retyped."""
    names = menu_names("Focus")
    i = names.index(verb("Focus", "Enable Style Check", "Disable Style Check"))
    return names[i + 1:i + 5]


# ---------------------------------------------------------------- measurement

SCRATCH = os.path.join(RAW, "scratch")   # `screencapture` will not write a dotted name


def scratch(tag):
    os.makedirs(SCRATCH, exist_ok=True)
    p = os.path.join(SCRATCH, f"{tag}.png")
    R.shoot(REGION, p)
    return p


def moved(png, ref):
    """Pixels this frame holds that the reference does not, ignoring the caret.

    The caret blinks, so its own accent is dropped from both frames before the
    comparison; everything else that differs is what the toggle did.
    """
    a, b = fast.arr(png), fast.arr(ref)
    keep = ~(fast.bar_mask(a) | fast.bar_mask(b))
    return int(((np.abs(a - b).max(axis=2) > 8) & keep).sum())


CARET_PX = 2000   # the caret's own fringe, the most two frames of one state differ by


def set_list(name, on, ref, tag):
    """Click a list and keep the click only if the frame moved the right way.

    A list whose words are not in the passage — Custom, which is empty — moves
    the frame not at all, and the click is taken back rather than guessed at.
    """
    before = moved(scratch(tag + "-a"), ref)
    click("Focus", name)
    after = moved(scratch(tag + "-b"), ref)
    row = dict(list=name, want=on, px_before=before, px_after=after, clicks=1)
    if abs(after - before) <= CARET_PX:
        click("Focus", name)
        return dict(row, clicks=2, ambiguous="no visible change either way — left as found")
    if (after > before) == on:
        return row
    click("Focus", name)
    return dict(row, clicks=2, px_reverted=moved(scratch(tag + "-c"), ref))


def all_lists_off(ref):
    """Walk the four lists from unknown to off, against a Style-Check-off frame."""
    out = []
    for i, name in enumerate(list_names()):
        out.append(set_list(name, False, ref, f"off{i}"))
    rest = moved(scratch("off-final"), ref)
    assert rest <= CARET_PX, f"{rest} px still differ from the Style-Check-off frame"
    return out + [dict(all_off_residual_px=rest)]


def assert_struck(ref, tag):
    """Style Check on, with the three lists it was left with, still marks the passage."""
    n = moved(scratch(tag), ref)
    assert n > 10 * CARET_PX, f"{tag}: only {n} px differ — the lists did not come back"
    return n


# ------------------------------------------------------------------- the rig

def place(downs=0, rights=0, shift_rights=0):
    osa('tell application "System Events" to key code 126 using {command down}')
    time.sleep(0.4)
    for _ in range(downs):
        osa('tell application "System Events" to key code 125')
    time.sleep(0.25)
    osa('tell application "System Events" to key code 123 using {command down}')
    time.sleep(0.35)
    for n, mods in ((rights, ""), (shift_rights, " using {shift down}")):
        if n:
            osa(f'tell application "System Events" to repeat {n} times\n'
                f' key code 124{mods}\nend repeat')
    time.sleep(0.7)


def at_end():
    osa('tell application "System Events" to key code 125 using {command down}')
    time.sleep(0.5)


def shoot(name, burst=True):
    raw = os.path.join(RAW, name)
    if burst:
        R.burst(REGION, raw)
    else:
        R.shoot(REGION, raw)
    norm = os.path.join(NORM, name)
    colour.normalise(raw, norm)
    return norm


def frame(name, ground, **extra):
    norm = shoot(name)
    colour.check(norm, ground)
    im = Image.open(norm)
    assert im.size == (2 * REGION[2], 2 * REGION[3]), (name, im.size)
    print(f"  {name}")
    return name, dict(ground=ground, colour_check="pass", **extra)


def appearance(ground):
    want("View", "Enable Dark Mode", "Disable Dark Mode", ground == "dark")
    time.sleep(1.0)


def main():
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    S.SAMPLE = SAMPLE
    S.REGION = REGION
    out = {"rig": dict(region_pt=list(REGION), passage=SAMPLE, state=24, ticket=354,
                       window_bounds="{0,33,1512,982}", editor="Mono, System - Default, "
                       "Normal text size"),
           "lists": {}, "frames": {}}

    for ground in ("dark", "light"):
        print(ground)
        appearance(ground)
        style(False)
        focus(False)
        syntax(False)
        S.reset()
        at_end()

        # The control every reading on this ground is a difference against.
        name, meta = frame(f"mac-native-24-{ground}-style-off.png", ground,
                           state="S0 control", style_check=False, lists=[],
                           focus=False, syntax=False, selection=None, caret="end of document")
        out["frames"][name] = meta
        ctrl = os.path.join(RAW, name)

        style(True)
        if ground == "dark":
            out["lists"]["walk_to_off"] = all_lists_off(ctrl)
        else:
            out["lists"]["walk_to_off_light"] = all_lists_off(ctrl)
        lists = list_names()

        # S2, light only: one list at a time, so which phrase each list owns is read
        # off a frame whose list is certain.
        if ground == "light":
            for i, l in enumerate(lists[:3]):
                out["lists"][f"on-{l}"] = set_list(l, True, ctrl, f"on{i}")
                at_end()
                slug = ("fillers", "cliches", "redundancies")[i]
                name, meta = frame(f"mac-native-24-light-style-list-{slug}.png", "light",
                                   state="S2 one list", style_check=True, lists=[l],
                                   focus=False, syntax=False, selection=None,
                                   caret="end of document")
                out["frames"][name] = meta
                out["lists"][f"off-{l}"] = set_list(l, False, ctrl, f"onoff{i}")

        for i, l in enumerate(lists[:3]):
            out["lists"][f"{ground}-on-{l}"] = set_list(l, True, ctrl, f"{ground}all{i}")
        three = lists[:3]
        out["lists"][f"{ground}-three-on-px"] = assert_struck(ctrl, f"{ground}-three")

        # S1: the passage, all three lists, caret at the end.
        at_end()
        name, meta = frame(f"mac-native-24-{ground}-style-all.png", ground,
                           state="S1 all three lists", style_check=True, lists=three,
                           focus=False, syntax=False, selection=None, caret="end of document")
        out["frames"][name] = meta

        # S3: Focus Sentence, caret in the second sentence — dim over strike.
        for on, tag in ((False, "focus-sentence-nostyle"), (True, "style-focus-sentence")):
            style(on)
            focus(True)
            click("Focus", "Sentence")
            place(downs=ROW2, rights=CARET_IN_S2)
            name, meta = frame(f"mac-native-24-{ground}-{tag}.png", ground,
                               state="S3 Focus Sentence", style_check=on,
                               lists=three if on else [], focus="Sentence", syntax=False,
                               selection=None, caret="second sentence, in 'the' of 'the team'")
            out["frames"][name] = meta
            focus(False)
        style(True)
        out["lists"][f"{ground}-after-S3-px"] = assert_struck(ctrl, f"{ground}-afterS3")

        # S4: Syntax highlight, all five Categories — strike over colour.
        for on, tag in ((False, "syntax-nostyle"), (True, "style-syntax")):
            style(on)
            syntax(True)
            at_end()
            name, meta = frame(f"mac-native-24-{ground}-{tag}.png", ground,
                               state="S4 Syntax highlight", style_check=on,
                               lists=three if on else [], focus=False,
                               syntax="all five Categories", selection=None,
                               caret="end of document")
            out["frames"][name] = meta
            syntax(False)
        style(True)
        out["lists"][f"{ground}-after-S4-px"] = assert_struck(ctrl, f"{ground}-afterS4")

        # S5: a selection held over "Against all odds" — the strike over the fill.
        for on, tag in ((False, "selection-nostyle"), (True, "style-selection")):
            style(on)
            place(downs=ROW2, rights=AGAINST, shift_rights=SEL_CELLS)
            name, meta = frame(f"mac-native-24-{ground}-{tag}.png", ground,
                               state="S5 selection", style_check=on,
                               lists=three if on else [], focus=False, syntax=False,
                               selection="Against all odds", caret="held selection")
            out["frames"][name] = meta
        style(True)

    shutil.rmtree(SCRATCH, ignore_errors=True)
    json.dump(out, open("ref/ia/mac-native/style-354.json", "w"), indent=1)
    print(f"{len(out['frames'])} frames")


if __name__ == "__main__":
    main()
