#!/usr/bin/env python3
"""#343: Classic's first-line indent, and the em of Classic and Manuscript, from ink.

#261 measured the Templates' pitches off `dev/ref/sample.md`, whose first paragraph
follows a heading — the one paragraph an indented Template does not indent — so
no frame showed a first line that follows a body paragraph, and a pitch is a
product of base and line height that a still cannot factor. Two passages settle
both: `dev/ref/short.md`, whose second paragraph follows a body paragraph, and
`passage-template-em.md`, whose paragraphs are runs of one glyph at three
lengths, so the difference between two extents is the advance with both side
bearings cancelled, and the run's own ink height is the cap height.

Preview is outside ADR 0015's reach: these frames inform the Templates ticket,
they do not write a design row. Each passage also gets an Editor frame with
Preview hidden, which is what the run's colour check runs on — Preview paints
its own paper and the § 4.2 rows do not describe it.

    python3 rig/run_templates.py <raw-dir> <normalised-dir>

Run from the repository root, with iA Writer in dark appearance, System —
Default, Library hidden, Focus, Typewriter, Syntax, Style Check and Authors off,
and the window at {0, 33, 1512, 982}.
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

RAW, NORM = sys.argv[1], sys.argv[2]
REGION = (0, 33, 1512, 949)          # the whole window
CHROME_TOP = 30                      # the window's own top edge
CHROME_BOTTOM = 1810                 # Preview's control bar sits at logical y 941
PASSAGES = [("short", "dev/ref/short.md"),
            ("em", "dev/ref/ia/mac-native/passage-template-em.md")]
TEMPLATES = [("modern", "Modern (Sans)"), ("classic", "Classic (Serif)"),
             ("duo", "Manuscript (Duo)"), ("mono", "Manuscript (Mono)")]


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout.strip()


def menu(bar, item):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{item}" of menu 1 of menu bar item "{bar}" of menu bar 1')


def submenu(bar, parent, item):
    osa(f'tell application "System Events" to tell process "iA Writer" to click menu item '
        f'"{item}" of menu 1 of menu item "{parent}" of menu 1 of menu bar item "{bar}" '
        f'of menu bar 1')


def preview(on):
    """Show or hide Preview. The menu item carries the verb, so it is read first."""
    want = "Show Preview" if on else "Hide Preview"
    names = osa('tell application "System Events" to tell process "iA Writer" to return name of '
                'every menu item of menu 1 of menu bar item "View" of menu bar 1')
    if want in names:
        menu("View", want)
        time.sleep(2.2)


def marks(parent):
    """A View submenu's items and which of them carries a check mark.

    Preview's own control bar auto-hides, so it cannot be asked; the menu can.
    """
    names = osa('tell application "System Events" to tell process "iA Writer" to return name of '
                f'every menu item of menu 1 of menu item "{parent}" of menu 1 of menu bar item '
                '"View" of menu bar 1').split(", ")
    ticks = osa('tell application "System Events" to tell process "iA Writer" to return value of '
                f'attribute "AXMenuItemMarkChar" of every menu item of menu 1 of menu item '
                f'"{parent}" of menu 1 of menu bar item "View" of menu bar 1').split(", ")
    return [n for n, t in zip(names, ticks) if t != "missing value"]


def preview_state():
    """Web / PDF / Split / Full and the Template, off the menu, plus the surface itself."""
    roles = osa('tell application "System Events" to tell process "iA Writer" to return role of '
                'every UI element of scroll area 1 of splitter group 1 of window 1')
    return dict(preview=marks("Preview"), template=marks("Template"), surface=roles)


def shoot(name):
    R.shoot(REGION, os.path.join(RAW, name))
    colour.normalise(os.path.join(RAW, name), os.path.join(NORM, name))
    return os.path.join(NORM, name)


def lines(png):
    """Ink rows of the page itself, without the window edge or Preview's control bar."""
    g, ls = fast.ink_lines(png)
    return g, [l for l in ls if CHROME_TOP <= l["y0"] < CHROME_BOTTOM and l["h"] > 4]


def paragraphs(ls):
    """Ink rows grouped where the gap between two rows exceeds the run of pitches."""
    if len(ls) < 2:
        return [ls]
    tops = [l["y0"] for l in ls]
    gaps = sorted(b - a for a, b in zip(tops, tops[1:]))
    pitch = gaps[len(gaps) // 3]                 # the common row-to-row step
    out = [[ls[0]]]
    for prev, cur in zip(ls, ls[1:]):
        (out.append([cur]) if cur["y0"] - prev["y0"] > 1.4 * pitch else out[-1].append(cur))
    return out


def main():
    os.makedirs(RAW, exist_ok=True)
    os.makedirs(NORM, exist_ok=True)
    S.REGION = REGION
    out = {"rig": dict(region_pt=list(REGION), appearance="dark"), "frames": {}}
    for pname, path in PASSAGES:
        preview(False)
        S.SAMPLE = path
        S.reset()
        time.sleep(0.6)
        name = f"mac-native-23-dark-template-editor-{pname}.png"
        colour.check(shoot(name), "dark")
        out["frames"][name] = dict(passage=path, surface="editor", template=None,
                                   colour_check="pass")
        preview(True)
        submenu("View", "Preview", "Web")
        time.sleep(1.2)
        submenu("View", "Preview", "Full")
        time.sleep(2.5)
        st = preview_state()
        assert set(st["preview"]) == {"Web", "Full"}, st
        assert "AXWebArea" in st["surface"], st
        for tname, label in TEMPLATES:
            submenu("View", "Template", label)
            time.sleep(2.8)
            st = preview_state()
            assert st["template"] == [label], st
            name = f"mac-native-23-dark-template-{tname}-{pname}.png"
            shoot(name)
            out["frames"][name] = dict(passage=path, surface="preview-web-full",
                                       template=label, preview=st["preview"],
                                       colour_check="n/a — Preview's own paper")
        preview(False)
    submenu("View", "Template", "Modern (Sans)")
    json.dump(out, open("dev/ref/ia/mac-native/templates-343.json", "w"), indent=1)
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
