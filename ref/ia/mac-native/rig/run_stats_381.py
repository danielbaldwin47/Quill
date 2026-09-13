#!/usr/bin/env python3
"""#381, state 27: the bar at the foot of the window, and the counts on it.

No committed capture shows it. `NOTES.md` § The rig never turned it on, neither
`NOTES.md` nor `VERDICTS.md` contains the word, and the only picture of it is a
2023 marketing still, which is not evidence.

The first thing the frames say is that iA has no stats bar of its own. What
stands at the foot of the window is the **Toolbar**, a format bar — Body,
Heading, List, Blockquote, Bold, Italic, Strikethrough, Link, Wikilink,
Footnote, Table, TOC — and the counts are one popup at its right end. `View >
Toolbar` offers **Fade In/Out** (the app's default), **Always Show** and
**Hide**, and a second pair, **Default** and **Stats Only**.

**Stats Only cannot be set from this rig, and is not shot.** Four ways of
pressing it — `click menu item` with the menu closed, the same with the menu
walked open, `perform action "AXPress"`, and an arrow-key walk of the open menu
— all report success, leave `Default` checked, and change **no pixel** of the
bar. The Style Check lists of #354 could at least be read by what the frame did;
this one does nothing to read. So every state here wears the **Default** bar,
which is what the app ships and what #381 asks for, and the gap is recorded.

Every state is shot with the bar and again with `Toolbar > Hide` and nothing
else changed, so the bar is exactly where the two frames differ and the gutter
above it is read against a page that has no bar under it at all.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/run_stats_381.py <raw-dir> <norm-dir>

Run from the repository root, with iA Writer in Mono, System — Default, Normal
text size, limit 64, Library and Preview hidden, Focus, Syntax, Style Check and
Authors off, on a scratch document.
"""
import json
import os
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

RAW = NORM = None
REGION = (0, 33, 1512, 949)          # the whole window, in logical points
SAMPLE = "ref/sample.md"
PREFIX = "mac-native-27"
SETTLE = 1.2
PROC = 'tell application "System Events" to tell process "iA Writer" to '


def osa(script, timeout=20):
    """One AppleScript, with a clock on it.

    A menu left open blocks `activate` for ever, and `subprocess.run` with no
    timeout waits for ever with it — which is how the first run of this capture
    wedged with the View menu standing open on the screen.
    """
    try:
        r = subprocess.run(["osascript", "-e", script], capture_output=True, text=True,
                           timeout=timeout)
        return (r.stdout or r.stderr).strip()
    except subprocess.TimeoutExpired:
        print("  ! osascript timed out:", script[:70], flush=True)
        dismiss()
        return ""


def dismiss():
    """Escape twice, so no menu is standing open when the next one is asked for."""
    for _ in range(2):
        subprocess.run(["osascript", "-e",
                        'tell application "System Events" to key code 53'],
                       capture_output=True, timeout=10)
        time.sleep(0.25)


def menu_names(bar):
    return osa(PROC + f'return name of every menu item of menu 1 of menu bar item '
                      f'"{bar}" of menu bar 1').split(", ")


def click(bar, item, settle=SETTLE):
    dismiss()
    osa(PROC + f'click menu item "{item}" of menu 1 of menu bar item "{bar}" of menu bar 1')
    time.sleep(settle)


def sub(parent, item, settle=SETTLE, bar="View"):
    dismiss()
    osa(PROC + f'click menu item "{item}" of menu 1 of menu item "{parent}" of menu 1 of '
               f'menu bar item "{bar}" of menu bar 1')
    time.sleep(settle)


def sub_marks(parent, bar="View"):
    base = (PROC + f'return %s of every menu item of menu 1 of menu item "{parent}" of menu 1 of '
                   f'menu bar item "{bar}" of menu bar 1')
    names = osa(base % "name").split(", ")
    marks = osa(base % 'value of attribute "AXMenuItemMarkChar"').split(", ")
    return {n.strip(): m.strip() not in ("missing value", "")
            for n, m in zip(names, marks) if n.strip() and n.strip() != "missing value"}


def toolbar(*items):
    """Set the Toolbar submenu's items, and read them back rather than trust the click.

    Only the fade/show/hide group is set here: `Stats Only` does not answer to
    any of the four ways of pressing it, so a state that asked for it would be a
    Default frame under another name.
    """
    for item in items:
        for _ in range(3):
            sub("Toolbar", item)
            if sub_marks("Toolbar").get(item):
                break
            print(f"  ! Toolbar > {item} did not take; again", flush=True)
    return sub_marks("Toolbar")


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


def appearance(ground):
    want("View", "Enable Dark Mode", "Disable Dark Mode", ground == "dark")
    time.sleep(1.0)


def typewriter(on):
    """Typewriter carries a mark rather than a verb, so the menu is read open."""
    assert "Typewriter" in menu_names("Focus")
    dismiss()
    osa(PROC + 'click menu bar item "Focus" of menu bar 1')
    time.sleep(0.5)
    got = osa(PROC + 'return value of attribute "AXMenuItemMarkChar" of menu item "Typewriter" '
                     'of menu 1 of menu bar item "Focus" of menu bar 1')
    dismiss()
    if (got not in ("missing value", "")) != on:
        click("Focus", "Typewriter")


def mouse(x_pt, y_pt, kind=None):
    """Move the pointer, and optionally click, where AppleScript cannot."""
    import Quartz
    pos = Quartz.CGPointMake(x_pt, y_pt)
    ev = Quartz.CGEventCreateMouseEvent(None, Quartz.kCGEventMouseMoved, pos, 0)
    Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
    time.sleep(0.5)
    if kind == "left":
        for k in (Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
            e = Quartz.CGEventCreateMouseEvent(None, k, pos, Quartz.kCGMouseButtonLeft)
            Quartz.CGEventPost(Quartz.kCGHIDEventTap, e)
            time.sleep(0.2)
        time.sleep(1.2)


def shoot(name, burst=False):
    raw = os.path.join(RAW, name)
    if burst:
        R.burst(REGION, raw)
    else:
        R.shoot(REGION, raw)
    norm = os.path.join(NORM, name)
    colour.normalise(raw, norm)
    return norm


def frame(out, name, ground, check=True, **extra):
    """One frame, normalised, and checked against the § 4.2 rows where it can be.

    `check=False` is for the empty document with the Toolbar hidden: there is no
    body ink anywhere on it, so `colour.check` has nothing to find and its
    failure would say nothing about the profile. The paper is still checked, by
    the state beside it that has ink.
    """
    norm = shoot(name)
    if check:
        colour.check(norm, ground)
    im = Image.open(norm)
    assert im.size == (2 * REGION[2], 2 * REGION[3]), (name, im.size)
    out["frames"][name] = dict(
        ground=ground,
        colour_check="pass" if check else "n/a - an empty page holds no body ink",
        **extra)
    print(f"  {name}", flush=True)
    return norm


def pair(out, ground, tag, state, setup, check=True, **extra):
    """The state with the bar, and the same state with the Toolbar hidden."""
    for shown in (True, False):
        toolbar("Always Show" if shown else "Hide")
        setup()
        frame(out, f"{PREFIX}-{ground}-stats-{tag}{'' if shown else '-nobar'}.png", ground,
              check=check, state=state,
              toolbar="Always Show, Default" if shown else "Hide", **extra)


def stats_box(on_png, off_png):
    """Where the counts stand, off the difference: the bar's rightmost ink.

    The bottom band of differing rows, not all of them: showing the bar also
    moves a few rows at the window's top edge, and a box drawn round both is the
    whole page — which is how the first run aimed its clicks at the middle of
    the text.
    """
    a, b = fast.arr(on_png).astype(int), fast.arr(off_png).astype(int)
    d = np.abs(a - b).sum(axis=2) > 20
    rows = np.where(d.any(axis=1))[0]
    y1 = int(rows.max())
    y0 = y1
    for r in rows[::-1]:
        if y0 - int(r) > 8:
            break
        y0 = int(r)
    # The counts are the bar's rightmost *ink*, not its rightmost difference:
    # the bar carries its own ground across the whole width, so a difference
    # taken against the page without it is one run from edge to edge.
    band = a[y0 + 6:y1 - 5]
    ground = np.median(band.reshape(-1, 3), axis=0)
    ink = (np.abs(band - ground).sum(axis=2) > 90).sum(axis=0) >= 2
    cols = np.where(ink)[0]
    runs, prev = [], None
    for c in cols:
        if prev is None or c - prev > 30:
            runs.append([int(c), int(c)])
        else:
            runs[-1][1] = int(c)
        prev = c
    # A group narrower than 30 px is the window's own rounded corner, not a label.
    wide = [r for r in runs if r[1] - r[0] + 1 >= 30]
    last = wide[-1]
    return dict(band=[y0, y1], height_px=y1 - y0 + 1, ground=[int(v) for v in ground],
                groups=wide, stats_cols=last,
                x_pt=round((last[0] + last[1]) / 4, 1),
                y_pt=round((y0 + y1) / 4 + REGION[1], 1))


def at_top():
    osa('tell application "System Events" to key code 126 using {command down}')
    time.sleep(0.6)


def at_end():
    osa('tell application "System Events" to key code 125 using {command down}')
    time.sleep(0.8)


def sample(region, n, interval):
    """A strip of the screen read repeatedly through Quartz, with a clock on each.

    `screencapture` is far too slow per frame to time a count that changes while
    the keys move, which is the same reason `blink.py` exists.
    """
    import Quartz
    x, y, w, h = region
    out = []
    for _ in range(n):
        img = Quartz.CGWindowListCreateImage(
            Quartz.CGRectMake(x, y, w, h), Quartz.kCGWindowListOptionOnScreenOnly,
            Quartz.kCGNullWindowID, Quartz.kCGWindowImageDefault)
        prov = Quartz.CGDataProviderCopyData(Quartz.CGImageGetDataProvider(img))
        out.append((time.time(), bytes(prov)))
        time.sleep(interval)
    return out


def observe(out, where):
    """What the bar does while the keys move, and when a count moves.

    Both are behaviour rather than colour, so one ground answers them. They are
    taken twice over: once on the app's own **Fade In/Out**, which is the
    setting a writer meets, and once on **Always Show**, because under the app's
    own setting with the pointer away there is no bar on the screen to watch —
    which is a finding, and is also why the first run of O2 sampled bare paper
    and timed nothing.
    """
    appearance("light")
    toolbar("Fade In/Out")
    S.reset()
    at_end()
    mouse(700, 300)                  # the pointer off the bar, so nothing is hovered
    time.sleep(2.5)
    frame(out, f"{PREFIX}-light-stats-fade-at-rest.png", "light",
          state="Fade In/Out, at rest, the pointer away from the bar",
          toolbar="Fade In/Out, Default", caret="document end", pointer="away")
    osa('tell application "System Events" to keystroke "a"')
    frame(out, f"{PREFIX}-light-stats-fade-typing.png", "light",
          state="Fade In/Out, a frame taken while the keys move",
          toolbar="Fade In/Out, Default", caret="document end", pointer="away")

    toolbar("Always Show")
    S.reset()
    at_end()
    mouse(700, 300)
    time.sleep(1.0)
    osa('tell application "System Events" to keystroke "a"')
    frame(out, f"{PREFIX}-light-stats-o1-typing.png", "light",
          state="O1 Always Show, a frame taken while the keys move",
          toolbar="Always Show, Default", caret="document end", pointer="away")

    # O2: the counts' own strip, sampled through Quartz while a word is typed.
    S.reset()
    at_end()
    strip = (where["stats_cols"][0] / 2 - 10, where["y_pt"] - 12,
             (where["stats_cols"][1] - where["stats_cols"][0]) / 2 + 20, 24)
    out["observations"]["strip_pt"] = [round(v, 1) for v in strip]
    before = sample(strip, 1, 0)[0][1]
    t0 = time.time()
    osa('tell application "System Events" to keystroke "wordcountprobe"')
    # The keystroke call is not instant, so the clock that matters starts when
    # it returns — the moment the last key has landed. Timing from before it
    # measures the typing and the update together and can separate neither.
    t1 = time.time()
    frames = sample(strip, 40, 0.1)
    changed = [(t - t0, t - t1) for t, b in frames if b != before]
    out["observations"]["typing_took_s"] = round(t1 - t0, 3)
    out["observations"]["counts_update_after_keys_s"] = round(changed[0][1], 3) if changed else None
    out["observations"]["counts_update_after_start_s"] = round(changed[0][0], 3) if changed else None
    out["observations"]["counts_settled_after_keys_s"] = round(changed[-1][1], 3) if changed else None
    out["observations"]["counts_samples"] = len(frames)
    out["observations"]["counts_changed_frames"] = len(changed)
    print("  typing took", out["observations"]["typing_took_s"], "s; counts first moved",
          out["observations"]["counts_update_after_keys_s"], "s after the last key", flush=True)
    S.reset()


def main():
    global RAW, NORM
    RAW, NORM = sys.argv[1], sys.argv[2]
    only_observe = "--observe-only" in sys.argv[3:]
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    S.SAMPLE = SAMPLE
    S.REGION = REGION
    out = {"rig": dict(region_pt=list(REGION), passage=SAMPLE, state=27, ticket=381,
                       window_bounds="{0,33,1512,982}",
                       editor="Mono, System - Default, Normal text size, limit 64",
                       toolbar_at_rest=sub_marks("Toolbar")),
           "frames": {}, "observations": {}}
    print("toolbar at rest:", out["rig"]["toolbar_at_rest"], flush=True)

    kept_path = os.path.join("ref", "ia", "mac-native", "stats-381.json")
    if only_observe:
        # The two behaviour questions, re-asked without re-shooting the states in
        # front of them. The counts' place is read back rather than found again.
        kept = json.load(open(kept_path))
        observe(out, kept["rig"]["stats_at"])
        kept["frames"].update(out["frames"])
        kept["observations"].update(out["observations"])
        json.dump(kept, open(kept_path, "w"), indent=1)
        print(f"{len(kept['frames'])} frames", flush=True)
        return

    for ground in ("light", "dark"):
        print(ground, flush=True)
        appearance(ground)
        typewriter(False)
        S.reset()

        # C1: the page at scroll 0, caret at the start, the bar showing.
        pair(out, ground, "c1-top", "C1 at the document top",
             lambda: (S.reset(), at_top()), caret="document start", scroll="top")
        # C2: scrolled to the end, so the air below the last row is read.
        pair(out, ground, "c2-end", "C2 at the document end",
             lambda: (S.reset(), at_end()), caret="document end", scroll="end")

    appearance("light")
    S.reset()

    # C3: an empty document, for the zero.
    pair(out, "light", "c3-empty", "C3 an empty document",
         lambda: S.reset(empty=True), check=False, caret="empty document", scroll="top")

    # C6: a selection over the first sentence, for what the counts say about it.
    first = open(SAMPLE, encoding="utf-8").read().index(".") + 1
    start = open(SAMPLE, encoding="utf-8").read().index("The lamp")

    def select_first():
        S.reset()
        at_top()
        osa(f'tell application "System Events" to repeat {start} times\n'
            f' key code 124\nend repeat')
        time.sleep(0.4)
        osa(f'tell application "System Events" to repeat {first - start} times\n'
            f' key code 124 using {{shift down}}\nend repeat')
        time.sleep(0.6)

    pair(out, "light", "c6-selection", "C6 a selection over the first sentence",
         select_first, caret="held selection", selection="the first sentence")

    # C4 and C5 want the bar's own coordinates, which the C1 pair gives.
    on = os.path.join(NORM, f"{PREFIX}-light-stats-c1-top.png")
    off = os.path.join(NORM, f"{PREFIX}-light-stats-c1-top-nobar.png")
    where = stats_box(on, off)
    out["rig"]["stats_at"] = where
    print("  stats at", where, flush=True)

    toolbar("Always Show")
    S.reset()
    at_top()
    mouse(where["x_pt"], where["y_pt"])
    frame(out, f"{PREFIX}-light-stats-c5-hover.png", "light", state="C5 the pointer on the counts",
          toolbar="Always Show, Default", caret="document start", pointer="on the counts")

    mouse(where["x_pt"], where["y_pt"], "left")
    frame(out, f"{PREFIX}-light-stats-c4-menu.png", "light", state="C4 the counts' menu open",
          toolbar="Always Show, Default", caret="document start", pointer="clicked the counts")
    # The popup is taller than the screen leaves room for and carries a chevron,
    # so a second frame is taken with it scrolled down. Down only moves the
    # highlight; nothing is chosen until Return, and Escape follows.
    for _ in range(6):
        osa('tell application "System Events" to key code 125')
        time.sleep(0.2)
    time.sleep(0.6)
    frame(out, f"{PREFIX}-light-stats-c4-menu-scrolled.png", "light",
          state="C4 the counts' menu, scrolled to its foot",
          toolbar="Always Show, Default", caret="document start",
          pointer="clicked the counts, then six Down presses")
    dismiss()
    mouse(700, 400)

    # C7: Typewriter, dark, the caret at the end — as a pair, because the
    # question is where the last row rests against the bar, and one frame cannot
    # say where that row would have fallen with no bar under it.
    appearance("dark")
    typewriter(True)
    pair(out, "dark", "c7-typewriter", "C7 Typewriter, the caret at the end",
         lambda: (S.reset(), at_end()), caret="document end", typewriter=True)
    typewriter(False)
    appearance("light")

    observe(out, where)
    json.dump(out, open(os.path.join("ref", "ia", "mac-native", "stats-381.json"), "w"), indent=1)
    print(f"{len(out['frames'])} frames", flush=True)


if __name__ == "__main__":
    main()
