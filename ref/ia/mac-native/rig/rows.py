#!/usr/bin/env python3
"""Row bands taken from an unselected frame, then read back out of a selected one.

A multi-row selection is continuous, so its own rows cannot be split on a gap.
The bands come from the frame without the selection — where the glyph ink still
separates one line from the next — and the fill is measured inside them.
"""
import sys
sys.path.insert(0, sys.path[0] or ".")
import iarig as R


def bands_from(png, pitch_guess=73):
    ground, ls = R.lines(png)
    return [(l["y0"], l["y1"]) for l in ls]


def fill_rows(sel_png, bands, pred):
    w, h, px = R.load(sel_png)
    out = []
    for a, b in bands:
        xs = [x for x in range(w) if sum(1 for y in range(a, b + 1) if pred(px[x, y])) >= 3]
        if not xs:
            out.append((a, b, None))
            continue
        runs = []
        for x in xs:
            if runs and x == runs[-1][1]:
                runs[-1][1] = x + 1
            else:
                runs.append([x, x + 1])
        out.append((a, b, runs))
    return out
