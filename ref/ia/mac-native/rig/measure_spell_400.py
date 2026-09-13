#!/usr/bin/env python3
"""#400's reader: the misspelling mark, read off the pairs `run_spell_400.py` shot.

Every state was shot twice, once with Check Spelling While Typing on and once
with it off, so the mark is exactly where the two frames differ and nothing has
to be assumed about what the paper under it would otherwise hold. That is
[#354](../CAPTURE-2026-09-10-STYLE.md)'s method, and the same reason stands: the
mark is drawn under the glyphs, so a reading taken off the marked frame alone
cannot tell the mark's ink from the descender crossing it.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/measure_spell_400.py

Run from the repository root.
"""
import json
import os
import struct
import sys

import numpy as np
from PIL import Image

SHOTS = os.path.join("ref", "ia", "shots", "mac-native")
PREFIX = "mac-native-26"
FONT = os.path.join("ref", "ia", "fonts", "Mono", "iAWriterMonoS-Regular.ttf")
CELL = 25.6                          # § The grid, at the default text size
X0 = 691.0                           # the body column's own left edge
PAPER = {"light": (0xf7, 0xf7, 0xf7), "dark": (0x1a, 0x1a, 0x1a)}
GAP = 6                              # blank rows that separate two marked lines


def arr(name):
    return np.asarray(Image.open(os.path.join(SHOTS, name)).convert("RGB")).astype(np.int16)


def hexof(p):
    return "#%02x%02x%02x" % tuple(int(v) for v in p)


def diff(on, off, tol=20):
    """Where the two frames differ: the mark, and nothing else."""
    return np.abs(arr(on) - arr(off)).sum(axis=2) > tol


def bands(mask, gap=GAP):
    """The mark's own rows, grouped into the text lines they belong to."""
    rows = np.where(mask.any(axis=1))[0]
    if not len(rows):
        return []
    out, start, prev = [], rows[0], rows[0]
    for r in rows[1:]:
        if r - prev > gap:
            out.append((int(start), int(prev)))
            start = r
        prev = r
    out.append((int(start), int(prev)))
    return out


def runs(cols):
    """Column runs, so one band's separate marks come apart from each other."""
    out, start, prev = [], cols[0], cols[0]
    for c in cols[1:]:
        if c - prev > 1:
            out.append((int(start), int(prev)))
            start = c
        prev = c
    out.append((int(start), int(prev)))
    return out


JOIN = 12                            # device px: wider than a dot's gap, narrower than a space


def merge(rs, join=JOIN):
    """Column runs closer than `join` joined into one.

    The mark is **dotted**, so every dot is a run of its own and a word is a
    train of them. A space is a whole cell, 25.6 px, and the gaps inside the
    mark are a few pixels, so the two do not meet in the middle.
    """
    out = [list(rs[0])]
    for a, b in rs[1:]:
        if a - out[-1][1] - 1 <= join:
            out[-1][1] = b
        else:
            out.append([a, b])
    return [tuple(r) for r in out]


def words(mask, band, min_w=8):
    """The marked words of one line, as trains of dots wider than a stray pixel."""
    y0, y1 = band
    cols = np.where(mask[y0:y1 + 1].any(axis=0))[0]
    if not len(cols):
        return []
    return [r for r in merge(runs(cols)) if r[1] - r[0] + 1 >= min_w]


def cells_of(run):
    """A column run as the cells it covers, so a mark can be named by its word."""
    return round((run[0] - X0) / CELL, 2), round((run[1] + 1 - X0) / CELL, 2)


PASSAGE = os.path.join("ref", "spell.md")
LIMIT = 64


def wrapped_rows(path=PASSAGE, limit=LIMIT):
    """The passage as the Editor lays it out: greedy word wrap at the limit."""
    rows = []
    for line in open(path, encoding="utf-8").read().split("\n"):
        if not line:
            rows.append("")
            continue
        cur = ""
        for w in line.split(" "):
            if cur and len(cur) + 1 + len(w) > limit:
                rows.append(cur)
                cur = w
            else:
                cur = f"{cur} {w}" if cur else w
        rows.append(cur)
    return rows


def word_index(rows=None):
    """Every word of the laid-out passage, keyed by the cells it stands on.

    A mark is named by where it is rather than by a list written here, so a
    change to the passage cannot leave this file quietly wrong.
    """
    idx = {}
    for row in (rows or wrapped_rows()):
        col = 0
        for w in row.split(" "):
            if w:
                idx.setdefault((col, len(w)), set()).add(w)
            col += len(w) + 1
    return idx


INDEX = word_index()


def word_at(run):
    """The word a mark covers, or None where no single word stands on those cells."""
    key = (int(round((run[0] - X0) / CELL)), int(round((run[1] + 1 - run[0]) / CELL)))
    got = INDEX.get(key)
    return sorted(got)[0] if got and len(got) == 1 else None


def ink_of(on, off, mask, band, run):
    """The mark's own colour, read only where its ground is clean.

    A dot that lands on a descender is the dot over ink, not the dot, and over a
    held selection the two are far enough apart to move the answer. So the
    ground is taken from the control frame first, and only the mark pixels
    standing on that one ground are counted.
    """
    a, b = arr(on), arr(off)
    y0, y1 = band
    m = mask[y0:y1 + 1, run[0]:run[1] + 1]
    sub, ctrl = a[y0:y1 + 1, run[0]:run[1] + 1], b[y0:y1 + 1, run[0]:run[1] + 1]
    bg = ground_of(b, y0, y1, run)
    clean = m & (np.abs(ctrl - bg).sum(axis=2) == 0)
    px = sub[clean if clean.sum() >= 20 else m]
    if not len(px):
        return None, 0, hexof(bg)
    vals, counts = np.unique(px, axis=0, return_counts=True)
    far = np.abs(vals - bg).sum(axis=1)
    keep = counts >= max(6, counts.max() // 8)
    i = int(np.argmax(np.where(keep, far, -1)))
    return hexof(vals[i]), int(counts[i]), hexof(bg)


def glyph_ink(off, band, run):
    """The word's own ink in the control frame, so a dimmed word can be told from a lit one."""
    a = arr(off)
    lo, hi = max(0, band[0] - 60), band[0] - 2
    bg = ground_of(a, lo, hi, run)
    sub = a[lo:hi + 1, run[0]:run[1] + 1].reshape(-1, 3)
    far = np.abs(sub - bg).sum(axis=1)
    vals, counts = np.unique(sub[far > 60], axis=0, return_counts=True)
    if not len(vals):
        return None
    keep = counts >= max(4, counts.max() // 8)
    i = int(np.argmax(np.where(keep, np.abs(vals - bg).sum(axis=1), -1)))
    return hexof(vals[i])


def profile(mask, band, run):
    """The mark's rows, and how wide it is on each of them."""
    y0, y1 = band
    return [(int(y), int(mask[y, run[0]:run[1] + 1].sum())) for y in range(y0, y1 + 1)]


def shape(mask, band, run):
    """Dots or a line: the lit columns of the mark's own widest row.

    A dotted underline leaves gaps that repeat; a solid one leaves none. Both
    the period and the lit width come off the same row, so neither is inferred
    from the other.
    """
    y0, y1 = band
    widths = [int(mask[y, run[0]:run[1] + 1].sum()) for y in range(y0, y1 + 1)]
    y = y0 + int(np.argmax(widths))
    lit = np.where(mask[y, run[0]:run[1] + 1])[0]
    rs = runs(lit) if len(lit) else []
    on_lens = [b - a + 1 for a, b in rs]
    gaps = [rs[i + 1][0] - rs[i][1] - 1 for i in range(len(rs) - 1)]
    periods = [rs[i + 1][0] - rs[i][0] for i in range(len(rs) - 1)]
    return dict(row=int(y), pieces=len(rs), lit_run_px=on_lens[:12], gap_px=gaps[:12],
                period_px=periods[:12],
                solid=len(rs) <= 1,
                period=round(float(np.median(periods)), 2) if periods else None,
                lit=round(float(np.median(on_lens)), 2) if on_lens else None)


def ground_of(a, lo, hi, run):
    """The commonest value behind a word — the paper, or the selection's fill.

    Sampled rather than assumed: over a held selection the background under a
    mark is the fill, and a reading that took the paper for it would call the
    whole band ink.
    """
    sub = a[lo:hi + 1, run[0]:run[1] + 1].reshape(-1, 3)
    vals, counts = np.unique(sub, axis=0, return_counts=True)
    return vals[int(np.argmax(counts))]


def baseline(off, band, run):
    """The text baseline of the line a mark sits under, off the control frame.

    Taken as the row where the line's ink falls away: every glyph but a
    descender ends there, so the count drops by most from one row to the next.
    """
    a = arr(off)
    lo = max(0, band[0] - 80)
    bg = ground_of(a, lo, band[1], run)
    ink = (np.abs(a[lo:band[1] + 1, run[0]:run[1] + 1] - bg).sum(axis=2) > 30).sum(axis=1)
    drops = [(int(ink[i] - ink[i + 1]), lo + i) for i in range(len(ink) - 1)]
    return max(drops)[1] if drops else None


def under(off, mask, band, run):
    """What the control frame holds where the mark is drawn — the mark's own ground."""
    a = arr(off)
    y0, y1 = band
    px = a[y0:y1 + 1, run[0]:run[1] + 1][mask[y0:y1 + 1, run[0]:run[1] + 1]]
    if not len(px):
        return None
    vals, counts = np.unique(px, axis=0, return_counts=True)
    return hexof(vals[int(np.argmax(counts))])


def read_pair(ground, tag, min_w=8):
    """Every mark on one state, named by the cells it covers."""
    on = f"{PREFIX}-{ground}-spell-{tag}.png"
    off = f"{PREFIX}-{ground}-spell-{tag}-nospell.png"
    if not os.path.exists(os.path.join(SHOTS, on)):
        return None
    m = diff(on, off)
    out = []
    for band in bands(m):
        for run in words(m, band, min_w):
            ink, n, bg = ink_of(on, off, m, band, run)
            base = baseline(off, band, run)
            out.append(dict(band=list(band), cols=list(run), cells=list(cells_of(run)),
                            word=word_at(run), width_cells=round((run[1] - run[0] + 1) / CELL, 2),
                            ink=ink, ink_px=n, under=bg, glyph_ink=glyph_ink(off, band, run),
                            rows=profile(m, band, run),
                            shape=shape(m, band, run), baseline=base,
                            above_baseline=[base - band[1], base - band[0]] if base else None))
    return out


def font_metrics(path=FONT):
    """`post` and `head` read straight out of the file, so no font library is needed.

    A Cocoa underline is drawn at the face's own `underlinePosition` and
    `underlineThickness` when it asks for one, so the two numbers the frames give
    can be checked against what the face asks for.
    """
    b = open(path, "rb").read()
    n = struct.unpack(">H", b[4:6])[0]
    tabs = {}
    for i in range(n):
        o = 12 + 16 * i
        tabs[b[o:o + 4].decode("latin-1")] = struct.unpack(">II", b[o + 8:o + 16])
    head, post = tabs["head"][0], tabs["post"][0]
    upem = struct.unpack(">H", b[head + 18:head + 20])[0]
    pos, thick = struct.unpack(">hh", b[post + 8:post + 12])
    em_px = 2 * 21.333                                 # the default em, in device px
    return dict(upem=upem, underline_position=pos, underline_thickness=thick,
                position_px=round(pos / upem * em_px, 2),
                thickness_px=round(thick / upem * em_px, 2))


RAW = os.path.join(SHOTS, "raw-2026-09-13-spell")


def over_fill(ground):
    """How much of what is under the mark shows through, off the selection state.

    Solved on the **raw** frames rather than the normalised ones: normalising is
    a colour conversion and it is not linear, so an alpha solved after it is not
    the alpha the app drew with. The selection state gives two grounds under one
    mark — the paper and the fill — which is what makes the solve possible at
    all.
    """
    def raw(name):
        return np.asarray(Image.open(os.path.join(RAW, name)).convert("RGB")).astype(np.int16)

    on = raw(f"{PREFIX}-{ground}-spell-s4-selection.png")
    off = raw(f"{PREFIX}-{ground}-spell-s4-selection-nospell.png")
    m = np.abs(on - off).sum(axis=2) > 20
    got = {}
    for band in bands(m):
        for run in words(m, band):
            y0, y1 = band
            mm = m[y0:y1 + 1, run[0]:run[1] + 1]
            sub = on[y0:y1 + 1, run[0]:run[1] + 1][mm]
            ctrl = off[y0:y1 + 1, run[0]:run[1] + 1][mm]
            v, c = np.unique(sub, axis=0, return_counts=True)
            g, gc = np.unique(ctrl, axis=0, return_counts=True)
            got.setdefault(tuple(g[int(np.argmax(gc))]), []).append(v[int(np.argmax(c))])
    if len(got) < 2:
        return None
    (g1, r1), (g2, r2) = [(np.array(k), v[0]) for k, v in got.items()][:2]
    dg, dr = g1 - g2, r1 - r2
    keep = np.abs(dg) > 3
    alpha = 1 - float(np.mean(dr[keep] / dg[keep]))
    src = (r1 - (1 - alpha) * g1) / alpha
    pred = alpha * src + (1 - alpha) * g2
    return dict(ground=ground, grounds=[hexof(g1), hexof(g2)], rendered=[hexof(r1), hexof(r2)],
                alpha=round(alpha, 3), source=hexof(np.clip(np.round(src), 0, 255)),
                predicts=hexof(np.clip(np.round(pred), 0, 255)),
                fits=bool(np.all(np.abs(pred - r2) <= 1)))


def main():
    out = {"font": font_metrics(), "states": {}, "over_fill": {}}
    for ground in ("light", "dark"):
        got = over_fill(ground)
        if got:
            out["over_fill"][ground] = got
    for ground in ("light", "dark"):
        for tag in ("s1-rest", "s2-focus", "s3-syntax", "s4-selection"):
            got = read_pair(ground, tag)
            if got is not None:
                out["states"][f"{ground}/{tag}"] = got
    json.dump(out, open(os.path.join("ref", "ia", "mac-native", "spell-400-marks.json"), "w"),
              indent=1)

    for key, marks in out["states"].items():
        print(f"\n=== {key}: {len(marks)} marks")
        for m in marks:
            s = m["shape"]
            form = ("solid" if s["solid"] else
                    f"{s['pieces']} dots, period {s['period']}, lit {s['lit']}")
            print(f"  {str(m['word']):12s} cells {m['cells'][0]:6.2f} w {m['width_cells']:5.2f}"
                  f"  mark {m['ink']} on {m['under']}  word {m['glyph_ink']}"
                  f"  rows {m['band'][0]}…{m['band'][1]}"
                  f"  below baseline {-m['above_baseline'][1]}…{-m['above_baseline'][0]}  {form}")
    print("\nface underline:", out["font"])
    for ground, got in out["over_fill"].items():
        print(f"over the selection fill, {ground}: {got}")


if __name__ == "__main__":
    main()
