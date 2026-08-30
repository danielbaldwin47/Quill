# THROWAWAY (#147): measures the caret's column off a judged shot.
#
# The numbers the ticket asks for are all in one row band: where each
# full-opacity bar starts and how wide it is, where the selection's fill
# begins and ends, and how far the nearest glyph ink is either side. So this
# asks ImageMagick for the bounding box of the caret blue, crops that band,
# and classifies the band's pixels in Python — the images themselves are never
# looked at, only the numbers that come out.
#
#   python3 tools/measure147.py <shot.png> [<shot.png> ...]
#
# Prints one block per shot, and a JSON summary on the last line for the sheet
# builder to read.
import json
import subprocess
import sys

# iA's caret blue, full opacity. The selection's fill is the same hue at .22
# over paper (about 195,235,251), which is far enough away in red that a bar
# and the fill it stands in never classify as each other.
CARET_BLUE = (0, 181, 255)


def is_bar(r, g, b):
    return r <= 90 and 120 <= g <= 220 and b >= 200


def is_fill(r, g, b):
    return b >= 235 and 215 <= g < 250 and 150 <= r < 235


def is_ink(r, g, b):
    return r + g + b < 330


def runs(cols):
    """Contiguous runs in a sorted column list, as (start, end_inclusive)."""
    out = []
    for c in cols:
        if out and c == out[-1][1] + 1:
            out[-1][1] = c
        else:
            out.append([c, c])
    return [(a, b) for a, b in out]


def band_of(png):
    """The bounding box of every caret-blue pixel: WxH+X+Y, or None."""
    out = subprocess.run(
        ["magick", png, "-fuzz", "10%", "+transparent", f"rgb{CARET_BLUE}",
         "-format", "%@", "info:"],
        capture_output=True, text=True, check=True).stdout.strip()
    if not out or out.startswith("0x0"):
        return None
    wh, x, y = out.replace("+", " +").split()
    w, h = wh.split("x")
    return int(w), int(h), int(x), int(y)


def measure(png):
    box = band_of(png)
    if box is None:
        return {"shot": png, "bars": [], "note": "no caret blue in this shot"}
    _, bh, _, by = box
    size = subprocess.run(["identify", "-format", "%w %h", png],
                          capture_output=True, text=True, check=True).stdout.split()
    iw = int(size[0])
    raw = subprocess.run(
        ["magick", png, "-crop", f"{iw}x{bh}+0+{by}", "+repage", "-depth", "8", "RGB:-"],
        capture_output=True, check=True).stdout

    bar_cols, fill_cols, ink_cols = [], [], []
    blue_rows, ink_rows = [0] * iw, [0] * iw
    for x in range(iw):
        bar = fill = ink = 0
        for y in range(bh):
            i = (y * iw + x) * 3
            r, g, b = raw[i], raw[i + 1], raw[i + 2]
            if is_bar(r, g, b):
                bar += 1
            elif is_fill(r, g, b):
                fill += 1
            if is_ink(r, g, b):
                ink += 1
        blue_rows[x], ink_rows[x] = bar, ink
        # A bar is the full height of the band, so a column that is mostly bar
        # is the bar's own and a column with a few blue pixels is a glyph's
        # antialiasing against it.
        if bar >= bh * 0.6:
            bar_cols.append(x)
        if fill >= bh * 0.3:
            fill_cols.append(x)
        if ink >= 2:
            ink_cols.append(x)

    bars = [{"x": a, "w": b - a + 1} for a, b in runs(bar_cols)]
    fills = [{"x": a, "w": b - a + 1} for a, b in runs(fill_cols)]
    # The fill can be broken by a bar standing inside it (shape 3), so the
    # selection's span is the outermost fill columns rather than one run.
    span = ({"x": fill_cols[0], "w": fill_cols[-1] - fill_cols[0] + 1}
            if fill_cols else None)

    for bar in bars:
        left = [c for c in ink_cols if c < bar["x"]]
        right = [c for c in ink_cols if c >= bar["x"] + bar["w"]]
        bar["inkLeft"] = bar["x"] - left[-1] - 1 if left else None
        bar["inkRight"] = right[0] - (bar["x"] + bar["w"]) if right else None
        # Clearance either side says nothing about a bar standing *on* a
        # glyph: those columns are inside the bar and fall out of both counts.
        # Counting columns overstates it — a `t`'s crossbar is three rows of a
        # 72-row band and crosses five columns — so what is reported is how
        # many of the bar's columns are the caret's blue all the way down
        # (`solid`), and the deepest bite any one column takes (`cut`). A bar
        # the oracle draws over the ink is solid on every column; ours is
        # drawn under it, so a stem shows through.
        under = range(bar["x"], bar["x"] + bar["w"])
        bar["solid"] = sum(1 for c in under if blue_rows[c] == bh)
        bar["cut"] = max(ink_rows[c] for c in under)
    return {"shot": png, "band": {"y": by, "h": bh}, "bars": bars,
            "fills": fills, "span": span}


results = [measure(p) for p in sys.argv[1:]]
for r in results:
    name = r["shot"].split("/")[-1]
    if not r["bars"]:
        print(f"{name:28} {r.get('note', 'nothing measured')}")
        continue
    band = f"y={r['band']['y']} h={r['band']['h']}"
    bars = "  ".join(
        f"x={b['x']} w={b['w']} gap<{b['inkLeft']} gap>{b['inkRight']} "
        f"solid={b['solid']}/{b['w']} cut={b['cut']}"
        for b in r["bars"])
    span = f"  fill x={r['span']['x']} w={r['span']['w']}" if r["span"] else ""
    print(f"{name:28} {band}  {bars}{span}")
print("JSON " + json.dumps(results))
