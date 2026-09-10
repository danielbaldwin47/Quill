#!/usr/bin/env python3
"""Verify the 2026-09-10 capture manifest and re-read every number off the frames.

Run from the repository root with the rig's Pillow/numpy Python. Reads only
files; no Mac UI, display or private app state is needed. Every coordinate is
device px at backing scale 2.

    python3 rig/measure_2026_09_10.py
"""
import hashlib
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, sys.path[0] or ".")
import colour
import fast

MAN = Path("ref/ia/mac-native/capture-2026-09-10.json")
NORM = Path("ref/ia/shots/mac-native")
FILL = (204, 237, 248)               # light selection band, § 4.2
CHROME = 120                         # window chrome, skipped when a fill is boxed
PREVIEW_BAR = 1810                   # Preview's control bar sits at logical y 941

# Mono, Duo and IBM Plex all put the cap of H at 0.698 em and the top of n at
# 0.528; Mono and Duo advance 0.600, IBM Plex Serif 0.787 and Sans 0.707, and
# Source Serif 4 — which is Quill's Classic face, not iA's — 0.788 with a cap at
# 0.670. Read out of the faces themselves, `fonts/` and the app bundle.
EM = dict(H_cap=0.698, n_top=0.528, mono_adv=0.600, plex_serif_adv=0.787,
          plex_sans_adv=0.707, ss4_cap=0.670, ss4_H_adv=0.788)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def fill_box(png, top=CHROME):
    m = fast.near_mask(fast.arr(png), FILL, 12)
    m[:top] = False
    rows = np.where(m.sum(axis=1) >= 4)[0]
    cols = np.where(m.sum(axis=0) >= 4)[0]
    return dict(x0=int(cols.min()), x1=int(cols.max()), w=int(cols.max() - cols.min() + 1),
                h=int(rows.max() - rows.min() + 1))


def page_lines(png, bottom=None):
    """Ink rows of the page, without the window's own edge or Preview's bar."""
    g, ls = fast.ink_lines(png)
    wide = 0.9 * fast.arr(png).shape[1]
    return g, [l for l in ls if l["h"] > 4 and not (l["y0"] < 30 and l["x1"] - l["x0"] > wide)
               and (bottom is None or l["y0"] < bottom)]


def advance(fills):
    """The advance, from `width = advance x cells + 2` fitted over the selection fills.

    NOTES' § The grid establishes the edge: a fill runs 1 px proud of the cell on
    each side, at every size and every count.
    """
    ns = sorted(fills)
    return sum(n * (fills[n] - 2) for n in ns) / sum(n * n for n in ns)


def slope(runs):
    """The advance from ink extents, where the edge is a side bearing and unknown.

    Two runs of one glyph differing only in length cancel it, so the advance is
    the slope alone and the intercept is thrown away.
    """
    ns = sorted(runs)
    return float(np.polyfit(ns, [runs[n] for n in ns], 1)[0])


def cap_height(png, line, paper, glyphs):
    """A run of H to a fraction of a pixel, from the ink each of its rows carries.

    The outermost row of the run is part-covered, and how much is the fraction of
    the row just inside it that it carries — the row inside is the same feature,
    a stem row on a sans and a serif row on a serif, so a serif does not saturate
    the reading the way a run-wide maximum would.
    """
    a = fast.arr(png)
    s = a[line["y0"] - 4:line["y1"] + 5, line["x0"]:line["x1"] + 1].mean(axis=2)
    cov = np.clip((s - paper) / (0xCC - paper), 0, 1).sum(axis=1) / glyphs
    on = np.where(cov > 0.02 * np.median(cov[cov > cov.max() * 0.3]))[0]
    top, bot = int(on.min()), int(on.max())
    return float(bot + min(cov[bot] / cov[bot - 1], 1)
                 - top - (1 - min(cov[top] / cov[top + 1], 1)))


def manifest():
    man = json.loads(MAN.read_text())
    profile = Path(man["rig"]["normalised_into"]).read_bytes()
    for name, item in man["captures"].items():
        assert digest(item["normalised_file"]) == item["sha256"], name
        assert digest(item["raw_file"]) == item["raw_sha256"], name
        im = Image.open(item["normalised_file"])
        assert list(im.size) == item["size_px"], name
        assert im.size == tuple(2 * n for n in item["region_pt"][2:]), name
        assert Image.open(item["raw_file"]).size == im.size, name
        assert im.info["icc_profile"] == profile, name
        if item["colour_check"] == "colour.check pass":
            colour.check(item["normalised_file"], item["ground"])
    assert digest(man["rig"]["display_profile_file"]) == man["rig"]["display_profile_sha256"]
    assert digest(man["rig"]["normalised_into"]) == man["rig"]["normalised_into_sha256"]
    print(f"capture manifest: pass ({len(man['captures'])} originals and normalised frames)")
    return man


def narrow():
    """#344 — State 22. What the type and the measure each do as the window narrows."""
    rows = sorted(json.load(open("ref/ia/mac-native/narrow-344.json")),
                  key=lambda r: (r["limit"], r["step"], r["width_pt"]))
    print("\n#344 — the Editor as the window narrows (Mono, light, ref/sample.md)")
    print(f"  {'window':>6} {'step':>4} {'limit':>5} {'advance':>7} {'em pt':>6} {'pitch':>5} "
          f"{'container':>9} {'wanted':>7} {'win-20':>7} {'gutter':>6} {'measure':>7}")
    for r in rows:
        f = {int(n): b["w"] for n, b in r["fills"].items()}
        a = advance(f)
        c = fill_box(NORM / r["frames"]["all"])
        left = fill_box(NORM / r["frames"][f"sel{min(f):02d}"])["x0"] + 1
        gut = left - c["x0"]
        want = (r["limit"] + 14) * a
        print(f"  {r['width_pt']:6} {r['step']:4} {r['limit']:5} {a:7.3f} {a / 1.2:6.3f} "
              f"{r['pitch']:5} {c['w']:9} {want:7.0f} {2 * r['width_pt'] - 20:7} "
              f"{gut / a:6.2f} {(c['w'] - 2 * gut) / a:7.2f}")
        if r["width_pt"] > 440:
            assert abs(c["w"] - min(want, 2 * r["width_pt"] - 20)) <= 4, (r["tag"], c["w"], want)
    print("  container = min(limit + 14 cells, window - 20 px) at every width above 440 pt;"
          " the gutter is 7 cells wherever the first term wins.")
    print("  440 pt is the exception: its container is 828, 32 px inside the window's 860,"
          " and its gutter is one cell.")
    edge = json.load(open("ref/ia/mac-native/narrow-344-threshold.json"))
    for run, got in edge.items():
        print(f"  threshold, {run}: " + "; ".join(
            f"pitch {t['narrow_pitch']} up to {t['narrow_at']} pt, "
            f"{t['wide_pitch']} from {t['wide_at']} pt" for t in got["thresholds"]))


def templates():
    """#343 — State 23. The Templates' indent and em, read off ink."""
    tem = json.load(open("ref/ia/mac-native/templates-343.json"))
    print("\n#343 — the Templates in Preview Web Full, dark")
    editor = NORM / "mac-native-23-dark-template-editor-em.png"
    _, ls = page_lines(editor)
    runs = {n: ls[i] for n, i in ((10, 6), (20, 7), (30, 8))}
    ed_adv = slope({n: l["x1"] - l["x0"] + 1 for n, l in runs.items()})
    ed_em = ed_adv / EM["mono_adv"]
    ed_H = cap_height(editor, runs[30], 0x1A, 30)
    lift = EM["H_cap"] * ed_em - ed_H
    print(f"  Editor control: advance {ed_adv:.3f}, em {ed_em:.3f} px, H ink {ed_H:.2f} px "
          f"against an outline of {EM['H_cap'] * ed_em:.2f} — the ink sits {lift:.2f} px inside it")
    print(f"  {'template':10} {'H adv':>6} {'n adv':>6} {'n/H':>6} {'H ink':>6} {'H out':>6} "
          f"{'pitch':>5} {'para step':>9} {'indent':>6}")
    out = {}
    for t in ("modern", "classic", "duo", "mono"):
        em_png = NORM / f"mac-native-23-dark-template-{t}-em.png"
        _, ls = page_lines(em_png, PREVIEW_BAR)
        runs = ls[-5:]
        w = [l["x1"] - l["x0"] + 1 for l in runs]
        aH = slope({10: w[0], 20: w[1], 30: w[2]})
        an = slope({10: w[3], 20: w[4]})
        body = ls[1:5]
        pitch = float(np.mean(np.diff([l["y0"] for l in body])))
        step = float(np.mean(np.diff([runs[i]["y0"] for i in (0, 1, 2)])))
        ink = cap_height(em_png, runs[2], 0x10, 30)
        short = NORM / f"mac-native-23-dark-template-{t}-short.png"
        _, sl = page_lines(short, PREVIEW_BAR)
        para2 = [l for l in sl if l["y0"] > sl[0]["y0"] + 1.5 * pitch]
        second = [l for l in para2 if l["y0"] - para2[0]["y0"] > 1.5 * pitch]
        indent = second[0]["x0"] - min(l["x0"] for l in second[1:])
        out[t] = dict(H=aH, n=an, ink=ink, outline=ink + lift, pitch=pitch, step=step)
        print(f"  {t:10} {aH:6.3f} {an:6.3f} {an / aH:6.4f} {ink:6.2f} {ink + lift:6.2f} "
              f"{pitch:5.1f} {step:9.1f} {indent:6}")
        assert abs(indent) <= 3, (t, indent)
    print("  indent is the first line of a paragraph that follows a body paragraph, against"
          " the lines under it: no Template indents one")
    m = out["duo"]
    assert abs(m["H"] - ed_adv) < 0.05 and abs(m["pitch"] - 73) < 0.5, m
    print(f"  Manuscript is the Editor's own grid: advance {m['H']:.3f} against the Editor's "
          f"{ed_adv:.3f}, pitch {m['pitch']:.1f}, em {m['H'] / EM['mono_adv']:.3f} px "
          f"= {m['H'] / EM['mono_adv'] / 2:.3f} pt")
    c = out["classic"]
    print(f"  Classic on Source Serif 4: matching its cap height needs em "
          f"{c['outline'] / EM['ss4_cap']:.2f} px = {c['outline'] / EM['ss4_cap'] / 2:.2f} pt; "
          f"matching its H advance needs {c['H'] / EM['ss4_H_adv']:.2f} px = "
          f"{c['H'] / EM['ss4_H_adv'] / 2:.2f} pt — the two are "
          f"{100 * (c['outline'] / EM['ss4_cap']) / (c['H'] / EM['ss4_H_adv']) - 100:.0f}% apart")
    print(f"  n/H advance: Classic {c['n'] / c['H']:.4f} against IBM Plex Serif's "
          f"{0.639 / 0.787:.4f} and Source Serif 4's {0.606 / 0.788:.4f}")
    assert len(tem["frames"]) == 10


if __name__ == "__main__":
    manifest()
    narrow()
    templates()
