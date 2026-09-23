#!/usr/bin/env python3
"""Verify the original-MBP capture manifest and re-read its measurements.

Run from the repository root with the rig's Pillow/numpy Python. Reads only files;
no Mac UI, display, or private app state is needed. Output coordinates are device px.
"""
import collections
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image

import colour
import fast

MANIFEST = Path("dev/ref/ia/mac-native/capture-original-mbp.json")
SHOTS = Path("dev/ref/ia/shots/mac-native")

# The caret is a saturated cyan bar whose hue falls in the blue band, so a scan
# that names a Category by hue counts it as a Verb. No Category colour comes near
# its blue, so it is dropped by colour rather than by geometry.
CARET = lambda a: (a[..., 2] >= 0xE0) & ((a[..., 2] - a[..., 0]) >= 0x80)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def family(rgb):
    r, g, b = (int(v) for v in rgb)
    rgb = (r, g, b)
    mx, mn = max(rgb), min(rgb)
    if mx == mn:
        return "grey"
    h = {r: (g - b) / (mx - mn), g: 2 + (b - r) / (mx - mn), b: 4 + (r - g) / (mx - mn)}[mx]
    h = (h * 60) % 360
    if h < 20 or h >= 330:
        return "noun"
    if h < 50:
        return "adjective"
    if h < 160:
        return "conjunction"
    if h < 260:
        return "verb"
    return "adverb"


def categories(png, chroma, rows=None):
    """The Category colours a frame carries: the commonest solid glyph colour of each."""
    a = np.asarray(Image.open(png).convert("RGB"))
    if rows:
        a = a[rows[0]:rows[1] + 1]
    b = a.astype(np.int16)
    m = ((b.max(axis=2) - b.min(axis=2)) >= chroma) & ~CARET(b)
    counts = collections.Counter(map(tuple, a[m]))
    seen = {}
    for rgb, n in counts.most_common(40):
        if n < 120:
            break
        f = family(rgb)
        if f not in seen:
            seen[f] = ("#%02x%02x%02x" % rgb, n)
    return seen


def body_rows(png):
    """Ink rows, without the window's own full-width top edge."""
    _, lines = fast.ink_lines(png)
    return [l for l in lines if not (l["y0"] < 30 and l["x1"] - l["x0"] > 2900)]


def main():
    man = json.loads(MANIFEST.read_text())
    profile = Path("dev/ref/ia/mac-native/rig/display.icc").read_bytes()
    for name, item in man["captures"].items():
        norm, raw = item["normalised_file"], item["raw_file"]
        assert digest(norm) == item["sha256"], name
        assert digest(raw) == item["raw_sha256"], name
        im = Image.open(norm)
        assert list(im.size) == item["size_px"], name
        assert im.size == tuple(2 * n for n in item["region_pt"][2:]), name
        assert Image.open(raw).size == im.size, name
        assert im.info["icc_profile"] == profile, name
        if item["colour_check"] == "pass":
            colour.check(norm, item["ground"])
    print(f"capture manifest: pass ({len(man['captures'])} originals and normalised frames)")

    rig = man["rig"]
    assert digest(rig["display_profile_file"]) == rig["display_profile_sha256"]
    assert digest("dev/ref/ia/mac-native/rig/display.icc") == rig["normalised_into_sha256"]
    print("display profiles: pass (both hashes match the manifest)")

    # #231 — the page top is a constant, at three pitches
    for step in ["00", "05", "13", "05-after"]:
        empty = SHOTS / f"mac-native-231-original-mbp-dark-page-top-empty-step-{step}.png"
        box = fast.caret_box(empty)
        rows = body_rows(SHOTS / f"mac-native-231-original-mbp-dark-page-top-step-{step}.png")
        gaps = [rows[i + 1]["y0"] - rows[i]["y0"] for i in range(1, 4)]
        assert box["y0"] == 164, (step, box)
        print(f"page top step {step:8} box {box['y0']} … {box['y1']}  body pitch {gaps}  ink {rows[0]['y0']}")

    # #231 — the title bar is an opaque band whose bottom is where paper begins
    a = np.asarray(Image.open(SHOTS / "mac-native-231-original-mbp-dark-titlebar-always.png").convert("RGB"))
    band = [y for y in range(200) if len(np.unique(a[y, 100:2900], axis=0)) == 1]
    edge = max(y for y in band if tuple(a[y, 1000]) != (0x1A,) * 3)
    print(f"title bar: opaque to y={edge}, paper from y={edge + 1}")

    # #308 — the ten Category colours, and dim over colour
    for ground, chroma in (("light", 40), ("dark", 24)):
        allf = SHOTS / f"mac-native-308-original-mbp-{ground}-syntax-all.png"
        cats = categories(allf, chroma)
        assert len(cats) == 5, (ground, cats)
        for fam, (hx, n) in sorted(cats.items()):
            line = f"  {ground:5} {fam:12} {hx} {n:6d} px"
            if ground == "light":
                only = SHOTS / f"mac-native-308-original-mbp-light-syntax-{fam}-only.png"
                iso = categories(only, chroma)
                assert set(iso) == {fam}, (fam, iso)
                assert iso[fam] == (hx, n), (fam, iso[fam], (hx, n))
                line += "   isolated frame agrees"
            print(line)
        focus = SHOTS / f"mac-native-308-original-mbp-{ground}-syntax-focus.png"
        rows = body_rows(focus)
        top = np.asarray(Image.open(focus).convert("RGB"))[rows[0]["y0"]:rows[0]["y1"] + 1]
        b = top.astype(np.int16)
        paper = 0xF7 if ground == "light" else 0x1A
        m = ((b.max(axis=2) - b.min(axis=2)) < 10) & (np.abs(b[..., 0] - paper) > 12)
        dim, npx = collections.Counter(map(tuple, top[m])).most_common(1)[0]
        held = categories(focus, chroma, rows=(rows[0]["y0"], rows[0]["y1"]))
        assert not held, (ground, held)
        print(f"  {ground:5} dimmed row  #{dim[0]:02x}{dim[1]:02x}{dim[2]:02x} {npx:6d} px, no Category colour")

    print("syntax: pass")


if __name__ == "__main__":
    main()
