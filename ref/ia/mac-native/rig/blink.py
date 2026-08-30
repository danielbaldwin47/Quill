#!/usr/bin/env python3
"""Sample the caret's own pixels fast enough to read its blink.

`screencapture` costs too much per frame to time a blink with, so the frames come
straight from the window server through Quartz. Each sample is the count of
accent-strength pixels in a small rect around the caret, stamped with a
monotonic clock; the cadence and the duty cycle are read off that series.
"""
import sys
import time

import Quartz


def grab(rect):
    x, y, w, h = rect
    img = Quartz.CGWindowListCreateImage(
        Quartz.CGRectMake(x, y, w, h),
        Quartz.kCGWindowListOptionOnScreenOnly,
        Quartz.kCGNullWindowID,
        Quartz.kCGWindowImageDefault)
    if img is None:
        return None
    W = Quartz.CGImageGetWidth(img)
    H = Quartz.CGImageGetHeight(img)
    bpr = Quartz.CGImageGetBytesPerRow(img)
    data = Quartz.CGDataProviderCopyData(Quartz.CGImageGetDataProvider(img))
    return W, H, bpr, bytes(data)


def accent(frame):
    W, H, bpr, buf = frame
    n = 0
    for yy in range(H):
        row = yy * bpr
        for xx in range(W):
            i = row + xx * 4
            b, g, r = buf[i], buf[i + 1], buf[i + 2]   # BGRA
            if b > 170 and b - r > 80 and g > 90:
                n += 1
    return n


def series(rect, seconds, out=None):
    t0 = time.monotonic()
    rows = []
    while time.monotonic() - t0 < seconds:
        f = grab(rect)
        if f is None:
            continue
        rows.append((time.monotonic() - t0, accent(f)))
    if out:
        with open(out, "w") as fh:
            for t, n in rows:
                fh.write(f"{t:.4f}\t{n}\n")
    return rows


def edges(rows, hi=None):
    """On/off transitions, using half of the observed maximum as the threshold."""
    peak = max(n for _, n in rows) if rows else 0
    thr = hi if hi is not None else peak * 0.5
    out, state = [], None
    for t, n in rows:
        s = n > thr
        if state is None:
            state = s
            continue
        if s != state:
            out.append((t, "on" if s else "off"))
            state = s
    return peak, thr, out


if __name__ == "__main__":
    x, y, w, h = (float(v) for v in sys.argv[1].split(","))
    secs = float(sys.argv[2])
    out = sys.argv[3] if len(sys.argv) > 3 else None
    rows = series((x, y, w, h), secs, out)
    peak, thr, es = edges(rows)
    dt = [b[0] - a[0] for a, b in zip(es, es[1:])]
    print(f"samples={len(rows)} rate={len(rows)/secs:.1f}/s peak={peak} thr={thr:.0f}")
    print("edges: " + " ".join(f"{t:.3f}{k}" for t, k in es[:24]))
    print("intervals: " + " ".join(f"{d:.3f}" for d in dt[:24]))
