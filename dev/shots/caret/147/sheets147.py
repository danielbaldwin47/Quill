#!/usr/bin/env python3
"""THROWAWAY (#147): builds the comparison sheets the owner decides from.

One sheet per view. Each is a row of enlarged crops at the insertion point,
zoomed `Z` times with nearest-neighbour so a bar and a glyph's stem stay
separate objects on the page, with the Parity oracle first and every shape
after it. The reference point the ticket asks for is a column of its own:
round 4's inset bar, the one its critic read as a smear.

Run from the repo root, after the shots are taken:

    python3 dev/shots/caret/147/sheets147.py

The crop windows are written out per view rather than read from
`measure147.py`: a crop has to hold still across shapes so the sheets can be
compared column to column, and a measured one would move with the bar.
"""
import subprocess

DIR = "dev/shots/caret/147"
Z = 3  # nearest-neighbour zoom; a bar is 6 device px and has to stay crisp

# The sheets are read on the issue, where GitHub fits them to the column width,
# so the labels are set large enough to survive that rather than sized to the
# crop they sit under.
LABEL_PT = 40
TITLE_PT = 46


def crop(src, box, out, label):
    x, y, w, h = box
    subprocess.run(
        ["magick", src, "-crop", f"{w}x{h}+{x}+{y}", "+repage",
         "-filter", "point", "-resize", f"{Z * 100}%",
         "-bordercolor", "#c8c8c8", "-border", "1", out], check=True)
    return ["-label", label, out]


def sheet(name, box, columns, title, tile):
    args = []
    for i, (src, label) in enumerate(columns):
        args += crop(src, box, f"/tmp/s147-{name}-{i}.png", label)
    subprocess.run(
        ["magick", "montage", *args, "-tile", tile, "-geometry", "+10+10",
         "-background", "white", "-fill", "#222", "-pointsize", str(LABEL_PT),
         f"{DIR}/sheet-{name}.png"], check=True)
    subprocess.run(
        ["magick", f"{DIR}/sheet-{name}.png", "-background", "white",
         "-fill", "#111", "-pointsize", str(TITLE_PT), "label:" + title,
         "+swap", "-gravity", "west", "-append",
         f"{DIR}/sheet-{name}.png"], check=True)
    print(f"wrote {DIR}/sheet-{name}.png")


def pair(name, ours, oracle, title):
    """Ours beside the oracle, whole frame, at half size for the page."""
    args = ["-label", "ours (shape 1, as-is)", ours, "-label", "Parity oracle", oracle]
    subprocess.run(
        ["magick", "montage", *args, "-tile", "2x1", "-geometry", "50%x50%+10+10",
         "-background", "white", "-fill", "#222", "-pointsize", "30",
         f"{DIR}/pair-{name}.png"], check=True)
    subprocess.run(
        ["magick", f"{DIR}/pair-{name}.png", "-background", "white", "-fill", "#111",
         "-pointsize", "34", "label:" + title, "+swap", "-gravity", "west", "-append",
         f"{DIR}/pair-{name}.png"], check=True)
    print(f"wrote {DIR}/pair-{name}.png")


SHAPES = [
    ("s1", "1 as-is"), ("s2", "2 no nudge"), ("s3", "3 nudged bars"),
    ("s4", "4 both"), ("s5", "5 oracle order"),
]

# The judged `caret` state: duo at 20 px, caret 403, which falls in a word
# space — which is why no critic has ever seen the thing #147 reports.
sheet("caret", (1856, 786, 160, 88),
      [("dev/shots/oracle/caret/caret.png", "oracle")]
      + [(f"{DIR}/{s}-caret.png", l) for s, l in SHAPES],
      "judged `caret` state, insertion point (duo 20px, offset 403, a word space)", "6x1")

# The judged `selection` state, both ends. Round 4's inset bar is the column
# the ticket asks to be shown beside them.
sheet("selection-open", (900, 426, 160, 88),
      [("dev/shots/oracle/caret/selection.png", "oracle"),
       ("dev/shots/caret/r4-selection-ours.png", "round 4 (lost)")]
      + [(f"{DIR}/{s}-selection.png", l) for s, l in SHAPES],
      "judged `selection` state, opening bar (duo 20px, select 153-171)", "7x1")

sheet("selection-close", (1350, 426, 160, 88),
      [("dev/shots/oracle/caret/selection.png", "oracle"),
       ("dev/shots/caret/r4-selection-ours.png", "round 4 (lost)")]
      + [(f"{DIR}/{s}-selection.png", l) for s, l in SHAPES],
      "judged `selection` state, closing bar (duo 20px, select 153-171)", "7x1")

# The jump pair: the ticket's own condition, where the next cell carries ink.
# Two rows, the free caret above the selection that opens at the same offset,
# so the jump is the horizontal distance between one row and the next.
jump = [("oracle-jump-caret", "oracle")] + [(f"{s}-jump-caret", l) for s, l in SHAPES] \
    + [("oracle-jump-select", "oracle")] + [(f"{s}-jump-select", l) for s, l in SHAPES]
sheet("jump", (866, 138, 160, 88),
      [(f"{DIR}/{n}.png", l) for n, l in jump],
      "the jump: free caret (top) then a selection opening at the same offset "
      "(bottom) — mono 20px, offset 10, the `t` of `test`", "6x2")

# Whole frames, so the crops above have somewhere to sit. Shape 1 only: it is
# the one that pairs with a frozen oracle shot, and the shapes differ from it
# by pixels that are invisible at this size.
pair("caret", f"{DIR}/s1-caret.png", "dev/shots/oracle/caret/caret.png",
     "judged `caret` state, whole frame")
pair("selection", f"{DIR}/s1-selection.png", "dev/shots/oracle/caret/selection.png",
     "judged `selection` state, whole frame")
pair("jump", f"{DIR}/s1-jump-caret.png", f"{DIR}/oracle-jump-caret.png",
     "the jump passage, free caret, whole frame")
