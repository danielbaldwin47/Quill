#!/usr/bin/env python3
"""Numpy readings of the same frames iarig describes, for the sweeps.

The pure-Python scans in iarig are fine for one frame and far too slow for a
size sweep, which shoots three frames a step. These give the same numbers off a
numpy array: the accent count, the caret's own box, the fill's box, and the
lines of glyph ink.
"""
import numpy as np
from PIL import Image


def arr(png):
    return np.asarray(Image.open(png).convert("RGB")).astype(np.int16)


def bar_mask(a):
    r, g, b = a[..., 0], a[..., 1], a[..., 2]
    return (b > 170) & ((b - r) > 80) & (g > 90)


def near_mask(a, c, tol=16):
    return (np.abs(a[..., 0] - c[0]) <= tol) & (np.abs(a[..., 1] - c[1]) <= tol) & \
           (np.abs(a[..., 2] - c[2]) <= tol)


def accent_count(png):
    return int(bar_mask(arr(png)).sum())


def ground(a):
    flat = a.reshape(-1, 3)[::7]
    cols, counts = np.unique(flat, axis=0, return_counts=True)
    return tuple(int(v) for v in cols[counts.argmax()])


def box(mask, min_cols=1):
    ys = np.where(mask.any(axis=1))[0]
    xs = np.where(mask.sum(axis=0) >= min_cols)[0]
    if not len(ys) or not len(xs):
        return None
    return int(xs.min()), int(xs.max()), int(ys.min()), int(ys.max())


def ink_lines(png, gap=2):
    a = arr(png)
    g = ground(a)
    dark = sum(g) < 384
    s = a.sum(axis=2)
    thr = sum(g) + 150 if dark else sum(g) - 150
    ink = (s > thr) if dark else (s < thr)
    ink &= ~bar_mask(a)
    rows = np.where(ink.sum(axis=1) > 3)[0]
    groups = []
    for y in rows:
        if groups and y <= groups[-1][1] + gap:
            groups[-1][1] = int(y)
        else:
            groups.append([int(y), int(y)])
    out = []
    for y0, y1 in groups:
        cols = np.where(ink[y0:y1 + 1].any(axis=0))[0]
        out.append(dict(y0=y0, y1=y1, h=y1 - y0 + 1,
                        x0=int(cols.min()), x1=int(cols.max())))
    return g, out


def caret_box(png):
    a = arr(png)
    m = bar_mask(a)
    b = box(m, min_cols=4)
    if b is None:
        return None
    x0, x1, y0, y1 = b
    cols = np.where(m.sum(axis=0) >= 4)[0]
    return dict(x0=x0, x1=x1, y0=y0, y1=y1, w=x1 - x0 + 1, h=y1 - y0 + 1,
                centre=(x0 + x1 + 1) / 2, npx=int(m.sum()))


def fill_box(png, colour, tol=16):
    a = arr(png)
    m = near_mask(a, colour, tol)
    b = box(m, min_cols=4)
    if b is None:
        return None
    x0, x1, y0, y1 = b
    return dict(x0=x0, x1=x1, y0=y0, y1=y1, w=x1 - x0 + 1, h=y1 - y0 + 1, npx=int(m.sum()))
