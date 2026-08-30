#!/usr/bin/env python3
"""Shoot and measure iA Writer for Mac.

The Mac port of `shots/caret/ia/wine/{pickbright,measure}.py`: the same reading,
the same vocabulary (band, bar, fill, ink, gap<, gap>, solid, cut), taken with
Pillow because this machine has no ImageMagick. Regions are given in logical
points, the way `screencapture -R` takes them, and every number that comes back
out is in device pixels at the display's backing scale.
"""
import os
import shutil
import subprocess
import sys
import time

from PIL import Image

SCALE = 2  # backing scale of the built-in Liquid Retina XDR, verified 200pt -> 400px


def shoot(region, out):
    """One capture of a logical-point region (x, y, w, h)."""
    x, y, w, h = region
    subprocess.run(["screencapture", "-x", "-R", f"{x},{y},{w},{h}", out], check=True)
    return out


def accent_count(png, pred=None):
    im = Image.open(png).convert("RGB")
    pred = pred or is_bar
    return sum(1 for p in im.getdata() if pred(p))


def burst(region, out, n=14, interval=0.11, pred=None):
    """Keep the frame of a burst with the most caret in it.

    The caret blinks, so one shot is a coin toss. A state that never draws a bar
    scores the same everywhere and keeps the first frame.
    """
    tmp = out + ".burst"
    os.makedirs(tmp, exist_ok=True)
    frames = []
    for i in range(n):
        f = os.path.join(tmp, f"{i:02d}.png")
        shoot(region, f)
        frames.append(f)
        time.sleep(interval)
    scores = [(accent_count(f, pred), f) for f in frames]
    best = max(scores, key=lambda s: s[0])
    shutil.copyfile(best[1], out)
    shutil.rmtree(tmp, ignore_errors=True)
    return best[0], [s[0] for s in scores]


def is_bar(p):
    """The caret's own accent: strongly blue and bright with it."""
    r, g, b = p
    return b > 170 and b - r > 80 and g > 90


def load(png):
    im = Image.open(png).convert("RGB")
    return im.size[0], im.size[1], im.load()


def classify(png, ink_thresh=None, fill_pred=None, bar_pred=None):
    w, h, px = load(png)
    bar_pred = bar_pred or is_bar

    counts = {}
    for y in range(0, h, 2):
        for x in range(0, w, 3):
            p = px[x, y]
            counts[p] = counts.get(p, 0) + 1
    ground = max(counts, key=counts.get)
    gsum = sum(ground)
    dark_ground = gsum < 384

    if ink_thresh is None:
        ink_thresh = gsum + 150 if dark_ground else gsum - 150

    def is_ink(p):
        if bar_pred(p):
            return False
        return sum(p) > ink_thresh if dark_ground else sum(p) < ink_thresh

    def default_fill(p):
        # The selection's tint: pulled off the ground toward the accent, but
        # nowhere near the bar's strength.
        r, g, b = p
        if bar_pred(p):
            return False
        return b - r > 12 and abs(b - ground[2]) > 12

    fp = fill_pred or default_fill

    rows = [y for y in range(h) if sum(1 for x in range(w) if is_ink(px[x, y])) > 3]
    if not rows:
        return dict(png=png, ground=ground, band=None, bars=[], fills=[], ink=[], size=(w, h))
    band = (min(rows), max(rows))
    bh = band[1] - band[0] + 1

    def runs(pred, minrows):
        cols = [x for x in range(w)
                if sum(1 for y in range(band[0], band[1] + 1) if pred(px[x, y])) >= minrows]
        out = []
        for x in cols:
            if out and x == out[-1][1]:
                out[-1][1] = x + 1
            else:
                out.append([x, x + 1])
        return out

    bars = runs(bar_pred, max(4, bh // 2))
    fills = runs(fp, max(4, bh // 2))
    ink = runs(is_ink, 1)
    return dict(png=png, ground=ground, band=band, bars=bars, fills=fills, ink=ink, size=(w, h))


def vspan(png, pred):
    """Full vertical extent of anything matching pred, in device pixels."""
    w, h, px = load(png)
    ys = [y for y in range(h) if any(pred(px[x, y]) for x in range(w))]
    return (min(ys), max(ys)) if ys else None


def hexof(p):
    return "#%02x%02x%02x" % p


def report(png, **kw):
    c = classify(png, **kw)
    name = os.path.basename(png)
    if c["band"] is None:
        print(f"{name}: {c['size'][0]}x{c['size'][1]} ground={hexof(c['ground'])} no text band")
        return c
    b0, b1 = c["band"]
    print(f"\n{name}  {c['size'][0]}x{c['size'][1]} ground={hexof(c['ground'])} "
          f"band={b0}..{b1} ({b1 - b0 + 1}px)")
    for a, b in c["fills"]:
        print(f"   fill  x={a}..{b - 1} w={b - a}")
    for a, b in c["bars"]:
        left = [r for r in c["ink"] if r[1] <= a]
        right = [r for r in c["ink"] if r[0] >= b]
        gl = a - left[-1][1] if left else None
        gr = right[0][0] - b if right else None
        on = any(r[0] < b and r[1] > a for r in c["ink"])
        sp = vspan(png, is_bar)
        w, h, px = load(png)
        col = [y for y in range(h) if is_bar(px[a, y])]
        solid = len(col) if col else 0
        print(f"   bar   x={a}..{b - 1} w={b - a} vspan={sp} h={sp[1] - sp[0] + 1 if sp else 0} "
              f"solid={solid} gap<{gl} gap>{gr} ink-under={on}")
    return c


if __name__ == "__main__":
    for p in sys.argv[1:]:
        report(p)


def ink_rows(png, bar_pred=None):
    """Rows carrying glyph ink, and the ground, for a multi-line frame."""
    w, h, px = load(png)
    bar_pred = bar_pred or is_bar
    counts = {}
    for y in range(0, h, 2):
        for x in range(0, w, 3):
            p = px[x, y]
            counts[p] = counts.get(p, 0) + 1
    ground = max(counts, key=counts.get)
    gsum = sum(ground)
    dark = gsum < 384
    thr = gsum + 150 if dark else gsum - 150

    def is_ink(p):
        if bar_pred(p):
            return False
        return sum(p) > thr if dark else sum(p) < thr

    rows = [y for y in range(h) if sum(1 for x in range(w) if is_ink(px[x, y])) > 3]
    return ground, rows, is_ink, (w, h, px)


def lines(png, bar_pred=None):
    """Contiguous ink-row groups: one per rendered text line."""
    ground, rows, is_ink, (w, h, px) = ink_rows(png, bar_pred)
    groups = []
    for y in rows:
        if groups and y <= groups[-1][1] + 2:
            groups[-1][1] = y
        else:
            groups.append([y, y])
    out = []
    for a, b in groups:
        xs = [x for x in range(w)
              if any(is_ink(px[x, y]) for y in range(a, b + 1))]
        out.append(dict(y0=a, y1=b, h=b - a + 1,
                        x0=min(xs) if xs else None, x1=max(xs) if xs else None))
    return ground, out


def pitch_report(png):
    ground, ls = lines(png)
    print(f"{os.path.basename(png)} ground={hexof(ground)} lines={len(ls)}")
    tops = [l["y0"] for l in ls]
    for i, l in enumerate(ls):
        step = tops[i] - tops[i - 1] if i else None
        print(f"  line{i:2d} y={l['y0']}..{l['y1']} h={l['h']} x={l['x0']}..{l['x1']} step={step}")
    steps = [b - a for a, b in zip(tops, tops[1:])]
    if steps:
        uniq = sorted(set(steps))
        print(f"  steps={steps}")
        print(f"  distinct steps={uniq}")
    return ls


def ground_of(png):
    w, h, px = load(png)
    counts = {}
    for y in range(0, h, 2):
        for x in range(0, w, 3):
            p = px[x, y]
            counts[p] = counts.get(p, 0) + 1
    return max(counts, key=counts.get), (w, h, px)


def feature(png, pred, min_run=2):
    """Every rectangle-ish region matching pred: its rows, its columns, its colours.

    Rows are grouped into bands the way a selection's rows group; within a band
    the column runs are reported separately, so a fill that spans rows and a bar
    that stands in one column both come back described rather than summed.
    """
    ground, (w, h, px) = ground_of(png)
    hits = {}
    for y in range(h):
        xs = [x for x in range(w) if pred(px[x, y])]
        if len(xs) >= min_run:
            hits[y] = xs
    bands = []
    for y in sorted(hits):
        if bands and y <= bands[-1][1] + 1:
            bands[-1][1] = y
        else:
            bands.append([y, y])
    out = []
    for a, b in bands:
        cols = sorted({x for y in range(a, b + 1) for x in hits.get(y, [])})
        runs = []
        for x in cols:
            if runs and x == runs[-1][1]:
                runs[-1][1] = x + 1
            else:
                runs.append([x, x + 1])
        cnt = {}
        for y in range(a, b + 1):
            for x in hits.get(y, []):
                p = px[x, y]
                cnt[p] = cnt.get(p, 0) + 1
        dom = max(cnt, key=cnt.get) if cnt else None
        out.append(dict(y0=a, y1=b, h=b - a + 1, runs=runs, colour=dom,
                        npx=sum(cnt.values())))
    return ground, out


def is_fill_dark(p):
    """A selection's tint on a dark ground: blue-biased, well short of the bar."""
    r, g, b = p
    return (not is_bar(p)) and b > 45 and b - r > 20 and b < 170


def is_fill_light(p):
    r, g, b = p
    return (not is_bar(p)) and b - r > 18 and b > 170 and r < 235


def show(png, pred, label, min_run=2):
    ground, fs = feature(png, pred, min_run)
    print(f"{os.path.basename(png)} ground={hexof(ground)} {label}: {len(fs)} band(s)")
    for f in fs:
        rs = " ".join(f"x{a}..{b-1}(w{b-a})" for a, b in f["runs"][:8])
        more = "" if len(f["runs"]) <= 8 else f" +{len(f['runs'])-8} more"
        print(f"   y={f['y0']}..{f['y1']} h={f['h']} colour={hexof(f['colour'])} "
              f"npx={f['npx']} :: {rs}{more}")
    return fs


def rowbands(png, pred, min_run=2, gap=6):
    """Like feature(), but bands split on a vertical gap so two text rows stay two."""
    ground, (w, h, px) = ground_of(png)
    hits = {}
    for y in range(h):
        xs = [x for x in range(w) if pred(px[x, y])]
        if len(xs) >= min_run:
            hits[y] = xs
    bands = []
    for y in sorted(hits):
        if bands and y <= bands[-1][1] + gap:
            bands[-1][1] = y
        else:
            bands.append([y, y])
    out = []
    for a, b in bands:
        cols = sorted({x for y in range(a, b + 1) for x in hits.get(y, [])})
        runs = []
        for x in cols:
            if runs and x == runs[-1][1]:
                runs[-1][1] = x + 1
            else:
                runs.append([x, x + 1])
        cnt = {}
        for y in range(a, b + 1):
            for x in hits.get(y, []):
                p = px[x, y]
                cnt[p] = cnt.get(p, 0) + 1
        out.append(dict(y0=a, y1=b, h=b - a + 1, runs=runs,
                        colour=max(cnt, key=cnt.get) if cnt else None,
                        npx=sum(cnt.values())))
    return ground, out


def ink_extent(png, y0, y1, bar_pred=None):
    """Leftmost and rightmost glyph ink between two rows, ignoring the bar."""
    ground, (w, h, px) = ground_of(png)
    bar_pred = bar_pred or is_bar
    gsum = sum(ground)
    dark = gsum < 384
    thr = gsum + 150 if dark else gsum - 150

    def is_ink(p):
        if bar_pred(p):
            return False
        return sum(p) > thr if dark else sum(p) < thr

    xs = [x for x in range(w)
          if any(is_ink(px[x, y]) for y in range(max(0, y0), min(h, y1 + 1)))]
    return (min(xs), max(xs)) if xs else None


def window_id(owner="iA Writer"):
    import Quartz
    wl = Quartz.CGWindowListCopyWindowInfo(
        Quartz.kCGWindowListOptionOnScreenOnly | Quartz.kCGWindowListExcludeDesktopElements,
        Quartz.kCGNullWindowID)
    best = None
    for w in wl:
        if owner in str(w.get("kCGWindowOwnerName", "")) and w.get("kCGWindowLayer") == 0:
            b = w.get("kCGWindowBounds")
            area = b["Width"] * b["Height"]
            if best is None or area > best[1]:
                best = (w.get("kCGWindowNumber"), area, b)
    return best


def shoot_window(out, crop_pts=None, owner="iA Writer"):
    """Capture the app's own window, so a deactivated window need not be uncovered.

    `screencapture -l` takes the window's image whatever is in front of it, which
    is the only way to shoot states 6 and 7 — the window has to lose key status,
    and anything brought forward to take it would otherwise sit over the frame.
    """
    wid, _, b = window_id(owner)
    raw = out + ".win.png"
    subprocess.run(["screencapture", "-x", "-o", "-l", str(wid), raw], check=True)
    if crop_pts is None:
        os.replace(raw, out)
        return out, b
    im = Image.open(raw).convert("RGB")
    sx = im.size[0] / b["Width"]           # device px per point, as captured
    x, y, w, h = crop_pts                  # screen points
    left = int(round((x - b["X"]) * sx))
    top = int(round((y - b["Y"]) * sx))
    im.crop((left, top, left + int(round(w * sx)), top + int(round(h * sx)))).save(out)
    os.remove(raw)
    return out, b
