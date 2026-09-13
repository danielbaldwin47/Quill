#!/usr/bin/env python3
"""#400, state 26: the misspelling mark, on both grounds and under everything.

No committed capture shows a misspelling — every state before this one was shot
on `ref/sample.md`, which has none — and on iA Mac the mark is macOS's own,
drawn by the text system rather than by iA. So its colour, its shape, its
thickness and where it sits against the baseline have never been measured, and
neither has what the app does around it. `quill-engine`'s `spell` Role ships a
provisional red under Pango's `error` underline until this lands (ADR 0017).

It follows [#354](../CAPTURE-2026-09-10-STYLE.md)'s shape, because the question
is the same shape: **the four measured states are shot as pairs**, once with Check
Spelling While Typing on and once with it off and nothing else changed, so every
number is a difference between two frames and nothing has to be assumed about
what the paper under a mark would otherwise hold. S5, S6 and the autocorrect run
carry no control and none is possible: each of them changes the text, so there is
no second frame of the same page to difference against.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_spell_400.py <raw-dir> <norm-dir>

Run from the repository root, with iA Writer in Mono, System — Default, Normal
text size, limit 64, Library and Preview hidden, Authors off, on a scratch
document. The script sets the appearance, Focus, Syntax and the spelling
switches itself.
"""
import json
import os
import subprocess
import sys
import time

from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast
import iarig as R
import states as S

RAW = NORM = None                    # set by main(), so the module can be imported
REGION = (0, 33, 1512, 949)          # the whole window, in logical points
SAMPLE = "ref/spell.md"
PREFIX = "mac-native-26"
SETTLE = 2.0                         # a Focus-menu toggle re-tags the document asynchronously
PROC = 'tell application "System Events" to tell process "iA Writer" to '
SPELL_MENU = ("Edit", "Spelling and Grammar", "Check Spelling While Typing")
AUTO_MENU = ("Edit", "Spelling and Grammar", "Correct Spelling Automatically")

TEXT = open(SAMPLE, encoding="utf-8").read()
AT_RECIEVED = TEXT.index("recieved the notes")
SEL_LEN = len("recieved the notes")
AT_SECOND = TEXT.index("ready") + 2          # inside a correctly spelled word of sentence two
AT_DEFINATELY = TEXT.index("definately")


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


def checked(bar, parent, item):
    """A checkbox menu item's own mark, which is how the spelling switches read."""
    got = osa(PROC + f'return value of attribute "AXMenuItemMarkChar" of menu item "{item}" of '
                     f'menu 1 of menu item "{parent}" of menu 1 of menu bar item "{bar}" of '
                     f'menu bar 1')
    return got not in ("missing value", "")


def set_checked(bar, parent, item, on):
    """Set a checkbox menu item, and read it back rather than trusting the click."""
    if checked(bar, parent, item) == on:
        return
    osa(PROC + f'click menu item "{item}" of menu 1 of menu item "{parent}" of menu 1 of '
               f'menu bar item "{bar}" of menu bar 1')
    time.sleep(1.2)
    assert checked(bar, parent, item) == on, (item, on)


def spell(on):
    """Set Check Spelling While Typing, and lay the passage down again under it.

    The switch alone does not re-check a document that is already on the screen,
    and `Check Document Now` is no substitute: it finds the *next* misspelling
    rather than marking them all, which left the first run of this capture with
    one mark on the page instead of seven. Pasting the passage while the switch
    is set is what marks every word — which is also how a writer meets the mark.
    """
    set_checked(*SPELL_MENU, on)
    time.sleep(0.8)
    S.reset()
    time.sleep(1.2)


def focus(on):
    want("Focus", "Enable Focus Mode", "Disable Focus Mode", on)


def syntax(on):
    want("Focus", "Show Syntax", "Hide Syntax", on)


def appearance(ground):
    want("View", "Enable Dark Mode", "Disable Dark Mode", ground == "dark")
    time.sleep(1.0)


def place(offset=0, shift=0):
    """Caret at a character offset from the document start, by plain Right presses.

    Not by row and column: the passage is one wrapped paragraph, so where a word
    falls on a row is the wrap's answer and not a number this script can hold.
    """
    osa('tell application "System Events" to key code 126 using {command down}')
    time.sleep(0.4)
    for n, mods in ((offset, ""), (shift, " using {shift down}")):
        if n:
            osa(f'tell application "System Events" to repeat {n} times\n'
                f' key code 124{mods}\nend repeat')
            time.sleep(0.3)
    time.sleep(0.6)


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


def frame(name, ground, burst=True, **extra):
    norm = shoot(name, burst)
    colour.check(norm, ground)
    im = Image.open(norm)
    assert im.size == (2 * REGION[2], 2 * REGION[3]), (name, im.size)
    print(f"  {name}", flush=True)
    return name, dict(ground=ground, colour_check="pass", **extra)


def pair(out, ground, tag, state, place_it, **extra):
    """The state with the mark and the same state without it, in that order.

    The control is shot second so that the last thing the app did before it was
    the same navigation, and nothing but the switch differs between the two.
    """
    for on in (True, False):
        spell(on)
        place_it()
        name, meta = frame(f"{PREFIX}-{ground}-spell-{tag}{'' if on else '-nospell'}.png",
                           ground, state=state, spell_check=on, **extra)
        out["frames"][name] = meta


def type_text(s):
    osa(f'tell application "System Events" to keystroke "{s}"')
    time.sleep(0.9)


def first_wave(on_png, off_png):
    """Where the first mark is, in logical points on the screen.

    Read off the difference rather than computed from a cell and a row: the
    passage is one wrapped paragraph, so which row a word falls on is the wrap's
    answer. The point returned is inside the word's own glyphs, a little above
    its mark, which is where a right-click has to land.
    """
    import numpy as np
    a, b = fast.arr(on_png).astype(int), fast.arr(off_png).astype(int)
    d = np.abs(a - b).sum(axis=2) > 20
    rows, cols = np.where(d)
    top = rows.min()
    band = (rows >= top) & (rows <= top + 12)          # the first marked row only
    c = np.unique(cols[band])
    # The first *word*, not the row: the mark is dotted, so the row's columns run
    # from the first word's first dot to the last word's last one, and their
    # midpoint is a word in between. A space is a whole cell; a dot's gap is a
    # few pixels. Aiming at the midpoint of the row is what put the first run's
    # right-click on `Tuesday` and opened a data detector's menu instead.
    cut = np.where(np.diff(c) > 12)[0]
    first = c[:cut[0] + 1] if len(cut) else c
    x = (first.min() + first.max()) / 2
    return dict(x_pt=round(x / 2, 1), y_pt=round((top - 20) / 2 + REGION[1], 1),
                mark_top_px=int(top), mark_cols_px=[int(first.min()), int(first.max())],
                row_cols_px=[int(c.min()), int(c.max())])


def left_click(x_pt, y_pt):
    """A left-click at a point on the screen.

    The one reliable way to end a word being typed: while the pending-correction
    pill is up it takes Escape and it takes ⌘↑, and the first two runs of S5 shot
    the caret still sitting in the word because of it.
    """
    import Quartz
    pos = Quartz.CGPointMake(x_pt, y_pt)
    for kind in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        ev = Quartz.CGEventCreateMouseEvent(None, kind, pos, Quartz.kCGMouseButtonLeft)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
        time.sleep(0.2)
    time.sleep(1.2)


def right_click(x_pt, y_pt):
    """A right-click at a point on the screen, which AppleScript cannot send."""
    import Quartz
    pos = Quartz.CGPointMake(x_pt, y_pt)
    for kind in (Quartz.kCGEventRightMouseDown, Quartz.kCGEventRightMouseUp):
        ev = Quartz.CGEventCreateMouseEvent(None, kind, pos, Quartz.kCGMouseButtonRight)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
        time.sleep(0.25)
    time.sleep(1.4)


def light_only(out):
    """S5 the word being typed, S6 the correction menu, and the autocorrect run.

    All three are behaviour rather than colour, so one ground answers them.
    """
    focus(False)
    syntax(False)
    spell(True)

    # S5: the word the caret stands in, typed and not yet ended. Three frames:
    # the word just typed, the caret still in it; the caret moved away; and a
    # space typed after it, which is also the first half of the autocorrect run.
    S.reset()
    at_end()
    osa('tell application "System Events" to keystroke return')
    time.sleep(0.5)
    type_text("comittee")
    name, meta = frame(f"{PREFIX}-light-spell-s5-typing.png", "light", state="S5 caret in the "
                       "word being typed", spell_check=True, focus=False, syntax=False,
                       selection=None, caret="inside 'comittee', no space typed yet")
    out["frames"][name] = meta
    type_text(" ")
    name, meta = frame(f"{PREFIX}-light-spell-s5-space.png", "light",
                       state="S5 the same word, a space typed after it", spell_check=True,
                       focus=False, syntax=False, selection=None, caret="after the space")
    out["frames"][name] = meta
    left_click(300, 300)             # the first body row, well away from the typed word
    name, meta = frame(f"{PREFIX}-light-spell-s5-caret-away.png", "light",
                       state="S5 the same word, the caret clicked away from it",
                       spell_check=True, focus=False, syntax=False, selection=None,
                       caret="clicked into the first body row")
    out["frames"][name] = meta

    # O2: the autocorrect run. `teh ` and `recieve ` typed with a space after
    # each, then one Backspace, on a document of their own.
    for word in ("teh", "recieve"):
        # A fresh document per word: the first run typed the second word onto
        # what the first one's Backspace had left, and read `Terecieve`.
        S.reset()
        at_end()
        osa('tell application "System Events" to keystroke return')
        time.sleep(0.5)
        type_text(word)
        name, meta = frame(f"{PREFIX}-light-spell-o2-{word}-typed.png", "light",
                           state=f"O2 '{word}' typed, no space", spell_check=True, focus=False,
                           syntax=False, selection=None, caret=f"end of '{word}'")
        out["frames"][name] = meta
        type_text(" ")
        name, meta = frame(f"{PREFIX}-light-spell-o2-{word}-space.png", "light",
                           state=f"O2 '{word}' with the space typed", spell_check=True,
                           focus=False, syntax=False, selection=None, caret="after the space")
        out["frames"][name] = meta
        osa('tell application "System Events" to key code 51')
        time.sleep(0.9)
        name, meta = frame(f"{PREFIX}-light-spell-o2-{word}-backspace.png", "light",
                           state=f"O2 one Backspace after '{word}'", spell_check=True,
                           focus=False, syntax=False, selection=None, caret="after the Backspace")
        out["frames"][name] = meta

    # S6: the correction menu over the first misspelling. The pair shot for it is
    # the bare page, so the menu is exactly where the two frames differ.
    S.reset()
    at_end()
    on_png = os.path.join(NORM, f"{PREFIX}-light-spell-s1-rest.png")
    off_png = os.path.join(NORM, f"{PREFIX}-light-spell-s1-rest-nospell.png")
    where = first_wave(on_png, off_png)
    out["rig"]["right_click_at"] = where
    print("  right-click at", where, flush=True)
    right_click(where["x_pt"], where["y_pt"])
    name, meta = frame(f"{PREFIX}-light-spell-s6-menu.png", "light",
                       state="S6 the correction menu", spell_check=True, focus=False,
                       syntax=False, selection=None,
                       caret=f"right-click on the first misspelling at {where['x_pt']},"
                             f"{where['y_pt']} pt", burst=False)
    out["frames"][name] = meta
    osa('tell application "System Events" to key code 53')
    time.sleep(0.6)


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    only_light = "--light-only" in sys.argv[3:]
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    S.SAMPLE = SAMPLE
    S.REGION = REGION
    out = {"rig": dict(region_pt=list(REGION), passage=SAMPLE, state=26, ticket=400,
                       window_bounds="{0,33,1512,982}",
                       editor="Mono, System - Default, Normal text size, limit 64",
                       autocorrect_at_rest=checked(*AUTO_MENU)),
           "offsets": dict(recieved=AT_RECIEVED, sel_len=SEL_LEN, second_sentence=AT_SECOND,
                           definately=AT_DEFINATELY),
           "frames": {}}

    for ground in () if only_light else ("light", "dark"):
        print(ground, flush=True)
        appearance(ground)
        focus(False)
        syntax(False)
        S.reset()
        at_end()

        # S1: the passage at rest, caret at the end of the document.
        pair(out, ground, "s1-rest", "S1 at rest", at_end,
             focus=False, syntax=False, selection=None, caret="end of document")

        # S2: Focus Sentence, caret in the second sentence, so the mark on a
        # dimmed misspelling is read directly.
        focus(True)
        click("Focus", "Sentence")
        pair(out, ground, "s2-focus", "S2 Focus Sentence",
             lambda: place(AT_SECOND), focus="Sentence", syntax=False, selection=None,
             caret="second sentence, inside 'ready'")
        focus(False)

        # S3: Syntax highlight, every Category, so mark over colour is read.
        syntax(True)
        pair(out, ground, "s3-syntax", "S3 Syntax highlight", at_end,
             focus=False, syntax="all five Categories", selection=None,
             caret="end of document")
        syntax(False)

        # S4: a selection held over 'recieved the notes'.
        pair(out, ground, "s4-selection", "S4 over a selection",
             lambda: place(AT_RECIEVED, SEL_LEN), focus=False, syntax=False,
             selection="recieved the notes", caret="end of the selection")

    appearance("light")
    light_only(out)
    path = os.path.join("ref", "ia", "mac-native", "spell-400.json")
    if only_light and os.path.exists(path):
        kept = json.load(open(path))
        kept["frames"].update(out["frames"])
        kept["rig"].update(out["rig"])
        out = kept
    json.dump(out, open(path, "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
