#!/usr/bin/env python3
"""Read #354's numbers off the frames `run_style.py` left.

Everything here is a difference between a Style Check frame and the control
frame shot beside it, so nothing depends on knowing the face. The unit is the
cell: the body column starts at x = 691.0 and the advance is 25.6 px (NOTES
§ The grid), so a span of columns is read back as a span of characters in the
row's own text and named as the phrase it covers.

The mark itself is read in the **gap columns** — the columns a row's control
frame leaves blank between two glyphs. Whatever is drawn there in the Style
Check frame is the mark and nothing else, so its colour is sampled off paper,
its thickness is a row count and its position is a row band the glyph rows of
the same line are measured against.

    python3 rig/measure_style_354.py            # prints the report
    python3 rig/measure_style_354.py --json     # and writes style-354-read.json

Run from the repository root.
"""
import json
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import marks

SHOTS = "dev/ref/ia/shots/mac-native/"
BODY_X = 691.0     # NOTES § The text container
ADVANCE = 25.6     # NOTES § The grid at the default text size
TOL = 24           # a pixel this far off the paper in any channel is something drawn
TOP = 60           # below the window's own top edge

# `dev/ref/style.md` as the 64-cell measure wraps it. The heading is row 0 and the
# blank line draws nothing, so the body rows are the ink rows after it.
ROWS = ["# Style check",
        "Basically, the plan was pretty much finished, and we were sort",
        "of ready to get down to brass tacks. Against all odds the team",
        "combined together the basic fundamentals into one very short",
        "draft. The long and short of it: the draft was a little rough,",
        "and it fell down only where the past history ran too long."]


def arr(png):
    return np.asarray(Image.open(SHOTS + png).convert("RGB")).astype(np.int16)


def paper_of(a):
    flat = a.reshape(-1, 3)[::7]
    cols, counts = np.unique(flat, axis=0, return_counts=True)
    return tuple(int(v) for v in cols[counts.argmax()])


def drawn(a, paper):
    """Everything the frame draws that is not the paper and not the caret.

    The caret blinks and is taller than the glyph band, so a row that holds it
    merges with its neighbour; dropping its own accent keeps the rows apart.
    """
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    caret = (b > 170) & ((b - r) > 80) & (g > 90)
    return (np.abs(a - np.array(paper)).max(axis=2) > TOL) & ~caret


def bands(a, paper, gap=6):
    """The ink rows of a frame, as (y0, y1) bands below the window edge."""
    rows = np.where(drawn(a, paper)[:, :].sum(axis=1) > 3)[0]
    rows = rows[rows >= TOP]
    out = []
    for y in rows:
        if out and y <= out[-1][1] + gap:
            out[-1][1] = int(y)
        else:
            out.append([int(y), int(y)])
    # a text row is at least the x-height tall, and the window's own bottom edge
    # is not a text row
    return [tuple(b) for b in out if b[1] - b[0] >= 20 and b[1] < 1800]


def x_of(cell):
    return BODY_X + ADVANCE * cell


def cell_of(x):
    return (x - BODY_X) / ADVANCE


def span_text(row, c0, c1):
    """The characters a column span covers, by the cell each column falls in."""
    lo, hi = int(round(cell_of(c0))), int(round(cell_of(c1 + 1)))
    return ROWS[row][max(lo, 0):hi]


def core(a, ys, xs, paper, min_count=6):
    """The colour furthest from the paper that a region holds at least `min_count` times."""
    if not len(ys) or not len(xs):
        return None, 0
    sub = a[ys[0]:ys[1] + 1, xs].reshape(-1, 3)
    sub = sub[np.abs(sub - np.array(paper)).max(axis=1) > TOL]
    if not len(sub):
        return None, 0
    cols, counts = np.unique(sub, axis=0, return_counts=True)
    keep = counts >= min_count
    cols, counts = (cols[keep], counts[keep]) if keep.any() else (cols, counts)
    d = np.abs(cols - np.array(paper)).sum(axis=1)
    i = int(d.argmax())
    return tuple(int(v) for v in cols[i]), int(counts[i])


def hexof(p):
    return "#%02x%02x%02x" % p if p else "-"


# ---------------------------------------------------------------- the strike

def struck_runs(style, ctrl, band, paper):
    """The mark's own columns on one row, grouped into the phrases they cross.

    A column blank in the control and drawn in the Style Check frame carries the
    mark and nothing else. Two such columns belong to one phrase when the gap
    between them is no wider than the widest glyph, which at this size is one cell.
    """
    y0, y1 = band
    cd = drawn(ctrl, paper)[y0:y1 + 1].any(axis=0)
    sd = drawn(style, paper)[y0:y1 + 1].any(axis=0)
    cols = np.where(sd & ~cd)[0]
    runs = []
    for x in cols:
        if runs and x - runs[-1][-1] <= ADVANCE:
            runs[-1].append(int(x))
        else:
            runs.append([int(x)])
    return runs


def mark_profile(style, run, band, paper):
    """The mark's rows, thickness and colour, read in one phrase's gap columns."""
    y0, y1 = band
    xs = np.array(run)
    d = drawn(style, paper)[y0:y1 + 1][:, xs]
    cover = d.sum(axis=1)
    peak = cover.max()
    rows = np.where(cover >= 0.75 * peak)[0]
    full = np.where(cover >= 0.98 * peak)[0]
    c, n = core(style, (y0 + full[0], y0 + full[-1]), xs, paper) if len(full) else (None, 0)
    return dict(y0=int(y0 + rows[0]), y1=int(y0 + rows[-1]),
                thickness_px=int(rows[-1] - rows[0] + 1),
                solid_y0=int(y0 + full[0]) if len(full) else None,
                solid_y1=int(y0 + full[-1]) if len(full) else None,
                solid_px=int(len(full)), colour=c, npx=n,
                cover=[int(v) for v in cover])


def glyph_band(ctrl, band, c0, c1, paper, skip=None):
    """The ink rows a span of cells covers in the control frame."""
    y0, y1 = band
    xs = np.arange(int(round(x_of(c0))), int(round(x_of(c1))))
    d = drawn(ctrl, paper)[y0:y1 + 1][:, xs]
    if skip:
        d = d.copy()
        d[skip[0] - y0:skip[1] - y0 + 1] = False
    rows = np.where(d.sum(axis=1) > 0)[0]
    return int(y0 + rows[0]), int(y0 + rows[-1])


# ------------------------------------------------------------------ the read

def read_pair(style_png, ctrl_png, label):
    style, ctrl = arr(style_png), arr(ctrl_png)
    paper = paper_of(ctrl)
    bs = bands(ctrl, paper)
    out = dict(frame=style_png, control=ctrl_png, label=label, paper=paper, rows=[])
    for i, band in enumerate(bs):
        runs = struck_runs(style, ctrl, band, paper)
        row = dict(band=list(band), phrases=[])
        for run in runs:
            if len(run) < 4:
                continue
            p = mark_profile(style, run, band, paper)
            c0, c1 = run[0], run[-1]
            # the phrase the mark crosses: the drawn columns it runs through
            sd = drawn(style, paper)[band[0]:band[1] + 1].any(axis=0)
            lo = c0
            while lo > 0 and sd[lo - 1]:
                lo -= 1
            hi = c1
            while hi + 1 < len(sd) and sd[hi + 1]:
                hi += 1
            p.update(x0=lo, x1=hi, cells=[round(cell_of(lo), 2), round(cell_of(hi), 2)],
                     text=span_text(i, lo, hi))
            struck = np.arange(lo, hi + 1)
            p["glyph_ink"] = core(style, band, struck, paper,
                                  min_count=6)[0] if len(struck) else None
            row["phrases"].append(p)
        out["rows"].append(row)
    return out, style, ctrl, paper, bs


def unstruck_ink(style, band, struck_cols, paper):
    """The ink of the row's glyphs that no mark crosses."""
    keep = np.ones(style.shape[1], bool)
    for a, b in struck_cols:
        keep[a:b + 1] = False
    xs = np.where(keep)[0]
    xs = xs[(xs >= int(BODY_X) - 4) & (xs <= int(x_of(64)))]
    return core(style, band, xs, paper)[0]


# ------------------------------------------------- what the face itself asks for

FONT = "dev/ref/ia/fonts/Mono/iAWriterMonoS-Regular.ttf"


def font_metrics(path=FONT):
    """`head` and `OS/2` read straight out of the file, so no font library is needed.

    A Cocoa strikethrough is drawn at the face's own `yStrikeoutPosition` and
    `yStrikeoutSize`, so the two numbers the frames give can be checked against
    what the face asks for rather than left as measurements of one app.
    """
    import struct
    b = open(path, "rb").read()
    n = struct.unpack(">H", b[4:6])[0]
    tabs = {}
    for i in range(n):
        o = 12 + 16 * i
        tabs[b[o:o + 4].decode("latin-1")] = struct.unpack(">II", b[o + 8:o + 16])
    head, os2 = tabs["head"][0], tabs["OS/2"][0]
    upem = struct.unpack(">H", b[head + 18:head + 20])[0]
    sz, pos = struct.unpack(">hh", b[os2 + 26:os2 + 30])
    xh, cap = struct.unpack(">hh", b[os2 + 86:os2 + 90])
    return dict(file=path, units_per_em=upem, strikeout_size=sz, strikeout_position=pos,
                x_height=xh, cap_height=cap)


# ------------------------------------------------------------------ the report

MIN_CELLS = 1.5    # a glyph's own crossbar crosses the mark's rows; a mark is wider


def baseline_of(ctrl, band, row, glyph, paper):
    """One row's baseline, off a flat-bottomed glyph — the first row below its ink."""
    c = ROWS[row].index(glyph)
    return glyph_band(ctrl, band, c, c + 1, paper)[1] + 1


FLAT = {1: "l", 2: "d", 3: "h", 4: "T", 5: "w"}     # a flat-bottomed glyph per body row


def marks_of(style_png, ctrl_png):
    """Every rule the frame draws: its cells, its phrase, its colour and its rows."""
    style, ctrl = arr(style_png), arr(ctrl_png)
    paper = paper_of(ctrl)
    body = bands(ctrl, paper)[1:6]
    out = []
    for i, band in enumerate(body, start=1):
        base = baseline_of(ctrl, band, i, FLAT[i], paper)
        for run in struck_runs(style, ctrl, band, paper):
            p = mark_profile(style, run, band, paper)
            # The rule's own span is the unbroken group of columns its rows draw
            # that the gap columns fall in; a glyph's crossbar reaches into the
            # same rows but is a group of its own, no wider than a cell.
            d = drawn(style, paper)[p["y0"]:p["y1"] + 1].all(axis=0)
            groups = []
            for x in np.where(d)[0]:
                if groups and x - groups[-1][1] <= 2:
                    groups[-1][1] = int(x)
                else:
                    groups.append([int(x), int(x)])
            hit = [gp for gp in groups if gp[0] <= run[0] and gp[1] >= run[-1]]
            if not hit:
                continue
            lo, hi = hit[0]
            c0, c1 = cell_of(lo), cell_of(hi + 1)
            if c1 - c0 < MIN_CELLS:
                continue
            out.append(dict(row=i, band=list(band), baseline=base,
                            cells=[round(c0, 2), round(c1, 2)], n_cells=round(c1 - c0, 2),
                            text=ROWS[i][int(round(c0)):int(round(c1))],
                            colour=p["colour"], hex=hexof(p["colour"]),
                            y0=p["y0"], y1=p["y1"], thickness_px=p["thickness_px"],
                            above_baseline_px=[base - p["y1"] - 1, base - p["y0"]]))
    return out, paper


def word_ink(png, ctrl_png, row, word, occurrence=0):
    a, c = arr(png), arr(ctrl_png)
    paper = paper_of(c)
    band = bands(c, paper)[1:6][row - 1]
    i = ROWS[row].index(word, occurrence)
    xs = np.arange(int(round(x_of(i))), int(round(x_of(i + len(word)))))
    return core(a, band, xs, paper)[0]


def fill_of(png, row, c0, c1):
    a = arr(png)
    paper = paper_of(a)
    band = bands(a, paper)[1:6][row - 1]
    xs = np.arange(int(round(x_of(c0))), int(round(x_of(c1))))
    sub = a[band[0]:band[1] + 1, xs].reshape(-1, 3)
    cols, cnt = np.unique(sub, axis=0, return_counts=True)
    return tuple(int(v) for v in cols[cnt.argmax()])


LISTS = [("fillers", "Fillers"), ("cliches", "Clichés"), ("redundancies", "Redundancies")]
SYNTAX_WORDS = [(1, "Basically"), (1, "plan"), (1, "pretty"), (1, "much"), (1, "finished"),
                (3, "together"), (3, "basic"), (3, "fundamentals"), (3, "very"), (3, "short"),
                (5, "down"), (5, "only"), (5, "past"), (5, "history"), (5, "long")]
FOCUS_WORDS = [("outside, unstruck", 1, "the plan was"), ("outside, struck", 1, "Basically,"),
               ("outside, struck", 1, "pretty much"), ("inside, unstruck", 3, "fundamentals"),
               ("inside, struck", 3, "together"), ("inside, struck", 3, "basic"),
               ("inside, struck", 3, "very")]


def main():
    rep = dict(font=font_metrics(), grid=dict(body_x=BODY_X, advance=ADVANCE, em_px=42.667))
    for g in ("dark", "light"):
        ctrl = f"mac-native-24-{g}-style-off.png"
        ms, paper = marks_of(f"mac-native-24-{g}-style-all.png", ctrl)
        thick = sorted({m["thickness_px"] for m in ms})
        cols = sorted({m["hex"] for m in ms})
        above = sorted({tuple(m["above_baseline_px"]) for m in ms})
        rep[g] = dict(paper=hexof(paper), ink=hexof(word_ink(ctrl, ctrl, 3, "fundamentals")),
                      mark_colour=cols, thickness_px=thick, above_baseline_px=above,
                      struck=[(m["row"], m["text"], m["n_cells"]) for m in ms],
                      x_height_px=None)
        b = bands(arr(ctrl), paper)[1:6][2]
        base = baseline_of(arr(ctrl), b, 3, "h", paper)
        xh = glyph_band(arr(ctrl), b, ROWS[3].index("short") + 3,
                        ROWS[3].index("short") + 4, paper)          # the 'r' of "short"
        rep[g]["x_height_px"] = base - xh[0]
        rep[g]["focus"] = {}
        for label, row, word in FOCUS_WORDS:
            k = f"{label}: {word}"
            rep[g]["focus"][k] = dict(
                style=hexof(word_ink(f"mac-native-24-{g}-style-focus-sentence.png",
                                     f"mac-native-24-{g}-focus-sentence-nostyle.png", row, word)),
                control=hexof(word_ink(f"mac-native-24-{g}-focus-sentence-nostyle.png",
                                       f"mac-native-24-{g}-focus-sentence-nostyle.png", row, word)))
        rep[g]["syntax"] = {}
        for row, word in SYNTAX_WORDS:
            rep[g]["syntax"][word] = dict(
                with_style=hexof(word_ink(f"mac-native-24-{g}-style-syntax.png",
                                          f"mac-native-24-{g}-syntax-nostyle.png", row, word)),
                syntax_only=hexof(word_ink(f"mac-native-24-{g}-syntax-nostyle.png",
                                           f"mac-native-24-{g}-syntax-nostyle.png", row, word)))
        c0 = ROWS[2].index("Against")
        rep[g]["selection"] = dict(
            fill=hexof(fill_of(f"mac-native-24-{g}-selection-nostyle.png", 2, c0, c0 + 16)),
            fill_with_style=hexof(fill_of(f"mac-native-24-{g}-style-selection.png", 2, c0, c0 + 16)),
            ink_selected=hexof(word_ink(f"mac-native-24-{g}-selection-nostyle.png",
                                        f"mac-native-24-{g}-selection-nostyle.png", 2,
                                        "Against all odds")),
            struck_selected=hexof(word_ink(f"mac-native-24-{g}-style-selection.png",
                                           f"mac-native-24-{g}-selection-nostyle.png", 2,
                                           "Against all odds")))
    rep["lists"] = {}
    for slug, name in LISTS:
        ms, _ = marks_of(f"mac-native-24-light-style-list-{slug}.png",
                         "mac-native-24-light-style-off.png")
        rep["lists"][name] = [(m["row"], m["text"], m["n_cells"]) for m in ms]
    return rep


if __name__ == "__main__":
    rep = main()
    print(json.dumps(rep, indent=1, ensure_ascii=False))
    if "--json" in sys.argv:
        with open("dev/ref/ia/mac-native/style-354-read.json", "w") as f:
            json.dump(rep, f, indent=1, ensure_ascii=False)
            f.write("\n")
