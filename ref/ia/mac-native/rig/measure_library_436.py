#!/usr/bin/env python3
"""#436's reader: what State 28's frames still hold, off the frames #379 shot.

`CAPTURE-2026-09-13-LIBRARY.md` § What this capture does not measure lists what
#379 left on the frames — the title bar over the pane, the Sort bar's own
numbers, a status line at the foot, a row's insets and icon, a folder row's
pitch against a file row's — and #436 adds the name, date and excerpt type
sizes and the date's right inset. Every one is an edge, a box or an ink on the
committed frames, so this reads them the way `measure_library_379.py` reads its
own: the ground first, the thing standing on it second, and every edge found by
where a run of pixels leaves the ground rather than by a number written here.

**Type size is the ink height of one named glyph.** A text band's own height
mixes ascenders, descenders and the icon beside it, so every type value names
the glyph it came from and says whether that glyph gives a cap height or an
x-height. The glyph windows are the one place a number is written here: each is
one letter's column range, taken off a ruled crop of the frame itself.

**Ground by ground, not one ground.** The title bar is the Organizer's own
ground over the Organizer, a ground of its own over the File List on dark, and a
third over the page; the Sort pill and the Filter field each have a ground
again, and the ink inside them is read against *that* rather than the pane's.

    python3 ref/ia/mac-native/rig/measure_library_436.py

Run from the repository root. Writes `ref/ia/mac-native/library-436-read.json`
and prints every value in it.
"""
import json
import os

import numpy as np
from PIL import Image

SHOTS = os.path.join("ref", "ia", "shots", "mac-native")
PREFIX = "mac-native-28"
OUT = os.path.join("ref", "ia", "mac-native", "library-436-read.json")

# The pane as `measure_library_379.py` read it: the Organizer's ground ends at
# 258 and the File List's runs to 719. Every inset below is from 259, the File
# List's own left edge.
ORG_R, LIST_L, LIST_R = 258, 259, 719


def arr(name):
    return np.asarray(Image.open(os.path.join(SHOTS, name)).convert("RGB")).astype(np.int16)


def hexof(p):
    return "#%02x%02x%02x" % tuple(int(round(float(v))) for v in p)


def pt(px):
    """Device pixels to points: every frame is backing scale 2."""
    return round(px / 2, 2)


def at(a, y, x):
    return np.array(a[y, x])


def median_of(a, y0, y1, x0, x1):
    return np.median(a[y0:y1, x0:x1].reshape(-1, 3), axis=0)


def box(a, y0, y1, x0, x1, ground, thresh):
    """The bounding box of everything in the window that leaves `ground`."""
    m = np.abs(a[y0:y1 + 1, x0:x1 + 1] - np.array(ground)).sum(axis=2) > thresh
    rows, cols = np.where(m.any(axis=1))[0], np.where(m.any(axis=0))[0]
    if not len(rows):
        return None
    return dict(y=[int(y0 + rows.min()), int(y0 + rows.max())],
                x=[int(x0 + cols.min()), int(x0 + cols.max())],
                h=int(rows.max() - rows.min() + 1), w=int(cols.max() - cols.min() + 1),
                h_pt=pt(int(rows.max() - rows.min() + 1)),
                w_pt=pt(int(cols.max() - cols.min() + 1)))


def ink(a, y0, y1, x0, x1, ground, thresh):
    """The commonest value in the window that is far enough from the ground to be ink."""
    sub = a[y0:y1 + 1, x0:x1 + 1].reshape(-1, 3)
    keep = sub[np.abs(sub - np.array(ground)).sum(axis=1) > thresh]
    if not len(keep):
        return None
    vals, counts = np.unique(keep, axis=0, return_counts=True)
    return hexof(vals[int(np.argmax(counts))])


def glyph(a, y0, y1, x0, x1, ground, thresh, kind):
    """One named glyph's ink height — a cap height or an x-height, as `kind` says."""
    b = box(a, y0, y1, x0, x1, ground, thresh)
    return None if b is None else dict(px=b["h"], pt=pt(b["h"]), kind=kind, y=b["y"], x=b["x"])


def rules(a, x0, x1, y0, y1, ground, tol=6, min_span=200):
    """Drawn rules: rows whose ink runs unbroken across most of a column."""
    out = []
    for y in range(y0, y1):
        d = np.abs(a[y, x0:x1 + 1] - np.array(ground)).sum(axis=1)
        cols = np.where(d > tol)[0]
        if not len(cols):
            continue
        span = cols.max() - cols.min() + 1
        if span >= min_span and len(cols) >= 0.95 * span:
            out.append(dict(row=int(y), ink=hexof(np.median(a[y, x0 + cols.min():
                                                              x0 + cols.max() + 1], axis=0)),
                            x=[int(x0 + cols.min()), int(x0 + cols.max())]))
    # One rule drawn over two device rows is one rule.
    bands, prev = [], None
    for r in out:
        if prev is not None and r["row"] - prev <= 1:
            bands[-1].append(r)
        else:
            bands.append([r])
        prev = r["row"]
    return [dict(row=b[0]["row"], rows=[b[0]["row"], b[-1]["row"]], thickness=len(b),
                 ink=b[0]["ink"], x=b[0]["x"]) for b in bands]


def capsule(a, y0, y1, x0, x1, ground, tol=12):
    """A capsule or pill: the rows its border reaches, and its width at mid-height."""
    rows = [y for y in range(y0, y1)
            if np.abs(np.median(a[y, x0:x1], axis=0) - np.array(ground)).sum() > tol]
    if not rows:
        return None
    top, bot = rows[0], rows[-1]
    mid = (top + bot) // 2
    cols = np.where(np.abs(a[mid, x0:x1] - np.array(ground)).sum(axis=1) > tol)[0]
    firsts = []
    for y in range(top, bot + 1):
        c = np.where(np.abs(a[y, x0:x1] - np.array(ground)).sum(axis=1) > tol)[0]
        if len(c):
            firsts.append(int(c.min()))
    return dict(y=[top, bot], h=bot - top + 1, h_pt=pt(bot - top + 1),
                x=[int(x0 + cols.min()), int(x0 + cols.max())] if len(cols) else None,
                w=int(cols.max() - cols.min() + 1) if len(cols) else None,
                w_pt=pt(int(cols.max() - cols.min() + 1)) if len(cols) else None,
                corner_inset=max(firsts) - min(firsts) if firsts else None)


def frame(theme, tag):
    return f"{PREFIX}-{theme}-library-{tag}.png"


def measure(theme):
    a = arr(frame(theme, "l1-rest"))
    nolib = arr(frame(theme, "l1-rest-nolib"))
    h = a.shape[0]
    o = {"frame": frame(theme, "l1-rest")}

    g_list, g_org, g_page = at(a, 1000, 700), at(a, 900, 120), at(a, 1000, 2600)
    bar_org, bar_list, bar_page = at(a, 52, 165), at(a, 160, 700), at(a, 20, 2600)
    o["grounds"] = dict(
        list=hexof(g_list), organizer=hexof(g_org), page=hexof(g_page),
        title_bar_over_organizer=hexof(bar_org), title_bar_over_list=hexof(bar_list),
        title_bar_over_page=hexof(bar_page))

    # ---- the title bar ---------------------------------------------------
    page_rule = rules(a, 900, 2900, 40, 110, bar_page, min_span=1500)
    list_rule = rules(a, LIST_L, LIST_R, 150, 175, bar_list, min_span=400)
    # The toggle: the run its own fill makes across the Organizer's ground.
    tog = box(a, 10, 95, 160, ORG_R - 2, g_org, 10)
    tog_n = box(nolib, 10, 95, 160, 268, at(nolib, 52, 165), 10)
    t = dict(height_px=page_rule[0]["row"] + 1 if page_rule else None,
             bottom_rule_over_page=page_rule[0] if page_rule else None,
             rule_under_pane_toolbar=list_rule[0] if list_rule else None,
             toggle=tog, toggle_nolib=tog_n,
             back_button=box(a, 10, 95, 266, 360, bar_list, 12),
             new_file_pill=box(a, 10, 95, 520, LIST_R - 4, bar_list, 12),
             history_pill=box(a, 10, 95, 725, 920, bar_page, 12),
             history_pill_nolib=box(nolib, 10, 95, 266, 460, at(nolib, 20, 700), 12),
             location_icon=box(a, 25, 80, 375, 419, bar_list, 40),
             title_text=box(a, 25, 80, 420, 515, bar_list, 60),
             title_cap_C=glyph(a, 25, 80, 434, 452, bar_list, 60, "cap height, the C of iCloud"),
             title_ink=ink(a, 25, 80, 420, 515, bar_list, 60),
             document_title=box(a, 25, 85, 1600, 2100, bar_page, 60),
             document_title_nolib=box(nolib, 25, 85, 1300, 1800, at(nolib, 20, 2600), 60),
             page_centre=(LIST_R + 1 + a.shape[1] - 1) // 2,
             window_centre=(a.shape[1] - 1) // 2)
    for k in ("document_title", "document_title_nolib"):
        t[k + "_centre"] = sum(t[k]["x"]) // 2 if t[k] else None
    o["title_bar"] = t

    # ---- the Sort bar ----------------------------------------------------
    pill = capsule(a, 95, 168, 265, 700, bar_list)
    pill_g = median_of(a, 120, 135, 340, 520)
    s = dict(pill=pill, pill_ground=hexof(pill_g),
             pill_border=ink(a, pill["y"][0], pill["y"][0], 300, 540, bar_list, 12)
             if pill else None,
             text_ink=ink(a, 118, 140, 292, 540, pill_g, 100),
             cap_S=glyph(a, 112, 145, 292, 308, pill_g, 100, "cap height, the S of Sort"),
             x_height_o=glyph(a, 112, 145, 309, 322, pill_g, 100, "x-height, the o of Sort"),
             anything_right_of_pill=box(a, 95, 168, 580, LIST_R - 4, bar_list, 10))
    if page_rule and list_rule:
        s["band_rows"] = [page_rule[0]["row"] + 1, list_rule[0]["row"]]
        s["band_height_px"] = list_rule[0]["row"] - page_rule[0]["row"]
        s["pane_toolbar_height_px"] = list_rule[0]["row"] + 1
    o["sort_bar"] = s

    # ---- the foot --------------------------------------------------------
    field = capsule(a, h - 76, h - 6, 265, 715, g_list)
    field_g = median_of(a, h - 53, h - 28, 450, 650)
    f = dict(rule_above=rules(a, LIST_L, LIST_R, h - 100, h - 60, g_list, min_span=300),
             field=field, field_ground=hexof(field_g),
             field_border=ink(a, field["y"][0], field["y"][0], 300, 660, g_list, 10)
             if field else None,
             icon=box(a, h - 62, h - 22, 288, 322, field_g, 60),
             icon_ink=ink(a, h - 62, h - 22, 288, 322, field_g, 60),
             prompt=box(a, h - 62, h - 22, 323, 500, field_g, 60),
             prompt_ink=ink(a, h - 62, h - 22, 323, 500, field_g, 60),
             prompt_cap_F=glyph(a, h - 62, h - 22, 326, 340, field_g, 60,
                                "cap height, the F of Filter"),
             pane_bottom_row=h - 1)
    if field:
        f["field_left_inset_px"] = field["x"][0] - LIST_L
        f["field_right_inset_px"] = LIST_R - field["x"][1]
        f["gap_below_field_px"] = (h - 1) - field["y"][1]
        f["anything_below_field"] = box(a, field["y"][1] + 3, h - 3, LIST_L + 4, LIST_R - 4,
                                        g_list, 10)
    o["foot"] = f

    # ---- the rows --------------------------------------------------------
    sep = rules(a, LIST_L + 4, LIST_R - 4, 175, 1500, g_list, min_span=200)
    seps = [r["row"] for r in sep]
    top = list_rule[0]["rows"][1] if list_rule else 170
    r = dict(separators=seps[:10], separator_thickness=sep[0]["thickness"] if sep else None,
             separator_ink=sep[0]["ink"] if sep else None, separator_x=sep[0]["x"] if sep else None,
             list_top_rule=top,
             file_row_pitch_px=seps[1] - seps[0] if len(seps) > 1 else None)
    r["separator_left_inset_px"] = r["separator_x"][0] - LIST_L
    r["separator_right_inset_px"] = LIST_R - r["separator_x"][1]
    y0 = seps[0] + 1                                   # winter-list.md, the first file row
    r["icon"] = box(a, y0, y0 + 60, 290, 338, g_list, 25)
    r["icon_body_ink"] = ink(a, y0, y0 + 60, 298, 330, g_list, 25)
    r["name"] = box(a, y0, y0 + 52, 336, 580, g_list, 100)
    r["name_ink"] = ink(a, y0, y0 + 52, 336, 580, g_list, 100)
    r["date"] = box(a, y0, y0 + 52, 580, LIST_R - 4, g_list, 60)
    r["date_ink"] = ink(a, y0, y0 + 52, 580, LIST_R - 4, g_list, 60)
    r["icon_left_inset_px"] = r["icon"]["x"][0] - LIST_L if r["icon"] else None
    r["name_left_inset_px"] = r["name"]["x"][0] - LIST_L if r["name"] else None
    r["date_right_inset_px"] = LIST_R - r["date"]["x"][1] if r["date"] else None
    r["name_x_height_n"] = glyph(a, y0 + 15, y0 + 50, 368, 382, g_list, 100,
                                 "x-height, the n of winter-list.md")
    r["name_ascender_l"] = glyph(a, y0 + 15, y0 + 50, 426, 432, g_list, 100,
                                 "ascender, the l of winter-list.md")
    r["name_cap_T"] = glyph(a, 1068, 1120, 340, 363, g_list, 100,
                            "cap height, the T of The Lighthouse.txt")
    r["date_digit_3"] = glyph(a, y0 + 15, y0 + 50, 592, 610, g_list, 60,
                              "digit height, the 3 of 3:16 AM")
    r["date_cap_A"] = glyph(a, y0 + 15, y0 + 50, 648, 668, g_list, 60, "cap height, the A of AM")
    r["excerpt_cap_W"] = glyph(a, y0 + 55, y0 + 85, 340, 378, g_list, 40,
                               "cap height, the W of Winter")
    r["excerpt_x_height_n"] = glyph(a, y0 + 55, y0 + 85, 397, 411, g_list, 40,
                                    "x-height, the n of Winter")
    r["excerpt_ink"] = ink(a, y0 + 55, y0 + 85, 341, 684, g_list, 40)
    # The folder row, above the first file row.
    r["folder_icon"] = box(a, top + 1, seps[0], 290, 338, g_list, 25)
    r["folder_icon_ink"] = ink(a, top + 1, seps[0], 299, 329, g_list, 60)
    r["folder_name"] = box(a, top + 1, seps[0], 336, 580, g_list, 100)
    r["folder_cap_D"] = glyph(a, top + 1, seps[0], 341, 364, g_list, 100,
                              "cap height, the D of Drafts")
    r["folder_chevron"] = box(a, top + 1, seps[0], 640, LIST_R - 4, g_list, 60)
    r["folder_date"] = box(a, top + 1, seps[0], 500, 660, g_list, 60)
    # The folder row's own height: its text's centre against a file row's, which
    # is what says whether the excerpt switch reaches it.
    r["folder_text_centre"] = (r["folder_name"]["y"][0] + r["folder_name"]["y"][1]) // 2 \
        if r["folder_name"] else None
    r["list_top_inset_px"] = (2 * r["folder_text_centre"] - top - seps[0] - 1) \
        if r["folder_text_centre"] else None
    o["rows"] = r

    # ---- the Organizer ---------------------------------------------------
    pillbox = capsule(a, 140, 230, 20, ORG_R - 8, g_org, tol=10)
    pill_g2 = median_of(a, 152, 165, 140, 220)
    z = dict(location_pill=pillbox, location_pill_ground=hexof(pill_g2),
             pill_left_inset_px=pillbox["x"][0] if pillbox else None,
             pill_right_inset_px=ORG_R - pillbox["x"][1] if pillbox else None,
             head_Locations=box(a, 105, 140, 20, 240, g_org, 80),
             head_cap_L=glyph(a, 105, 140, 33, 47, g_org, 80, "cap height, the L of Locations"),
             head_ink=ink(a, 113, 133, 32, 140, g_org, 60),
             head_Favorites=box(a, 240, 275, 20, 240, g_org, 80),
             head_SmartFolders=box(a, 448, 482, 20, 240, g_org, 80),
             head_Hashtags=box(a, 582, 620, 20, 240, g_org, 80),
             pill_icon=box(a, 160, 205, 30, 82, pill_g2, 40),
             pill_label=box(a, 160, 205, 84, 235, pill_g2, 80),
             pill_label_cap_C=glyph(a, 160, 205, 96, 114, pill_g2, 80,
                                    "cap height, the C of iCloud"),
             pill_label_ink=ink(a, 167, 193, 84, 165, pill_g2, 80),
             row_icon=box(a, 500, 545, 28, 78, g_org, 40),
             row_text=box(a, 500, 545, 80, 240, g_org, 80),
             row_cap_R=glyph(a, 500, 545, 82, 104, g_org, 80, "cap height, the R of Recents"),
             row_ink=ink(a, 511, 534, 82, 180, g_org, 60),
             prose=box(a, 282, 410, 20, 240, g_org, 80),
             prose_ink=ink(a, 288, 340, 30, 200, g_org, 60),
             prose_x_height_r=glyph(a, 285, 315, 62, 76, g_org, 80, "x-height, the r of Drag"))
    o["organizer"] = z

    # ---- the light-only frames -------------------------------------------
    if theme == "light":
        bare = arr(frame("light", "l7-bare"))
        bsep = [x["row"] for x in rules(bare, LIST_L + 4, LIST_R - 4, 110, 900, g_list,
                                        min_span=200)]
        bname = box(bare, bsep[0] + 1, bsep[1], 336, 580, g_list, 100)
        bfold = box(bare, 105, bsep[0], 336, 580, g_list, 100)
        o["bare"] = dict(
            frame=frame("light", "l7-bare"), separators=bsep[:10],
            toolbar_rules=rules(bare, LIST_L, LIST_R, 105, 180, g_list, min_span=400),
            filter_field=capsule(bare, h - 76, h - 6, 265, 715, g_list),
            file_row_pitch_px=bsep[1] - bsep[0] if len(bsep) > 1 else None,
            folder_text_centre=(bfold["y"][0] + bfold["y"][1]) // 2 if bfold else None,
            first_file_text_centre=(bname["y"][0] + bname["y"][1]) // 2 if bname else None,
            folder_row=[105, bsep[0]], icon=box(bare, bsep[0] + 1, bsep[1], 290, 338, g_list, 25))
        fold = arr(frame("light", "l3-folder"))
        fsep = rules(fold, LIST_L + 4, LIST_R - 4, 175, 700, g_list, min_span=200)
        o["folder_open"] = dict(
            frame=frame("light", "l3-folder"),
            separators=[x["row"] for x in fsep][:6],
            separator_x_parent=fsep[0]["x"], separator_x_child=fsep[1]["x"],
            parent_chevron=box(fold, 190, fsep[0]["row"], 640, LIST_R - 4, g_list, 60),
            child_icon=box(fold, fsep[0]["row"] + 1, fsep[1]["row"], 300, 364, g_list, 25),
            child_name=box(fold, fsep[0]["row"] + 1, fsep[0]["row"] + 52, 360, 600, g_list, 100),
            child_date=box(fold, fsep[0]["row"] + 1, fsep[0]["row"] + 52, 600, LIST_R - 4,
                           g_list, 60))
        drag = arr(frame("light", "o1-dragged"))
        col = np.median(drag[400:1600], axis=0)
        edges = [int(i) for i in np.where(np.abs(np.diff(col, axis=0)).sum(axis=1) > 6)[0]]
        right = edges[2] if len(edges) > 2 else None
        o["dragged"] = dict(
            frame=frame("light", "o1-dragged"), column_edges=edges[:6],
            pane_width_pt=pt(right + 1) if right else None,
            icon=box(drag, 253, 320, 290, 338, g_list, 25),
            name=box(drag, 253, 300, 336, 700, g_list, 100),
            date=box(drag, 253, 300, 800, right - 4, g_list, 60) if right else None)
        if right and o["dragged"]["date"]:
            o["dragged"]["date_right_inset_px"] = right - o["dragged"]["date"]["x"][1]
            o["dragged"]["name_left_inset_px"] = o["dragged"]["name"]["x"][0] - LIST_L
    return o


def main():
    out = {t: measure(t) for t in ("light", "dark")}
    json.dump(out, open(OUT, "w"), indent=1, default=str)
    print(json.dumps(out, indent=1, default=str))


if __name__ == "__main__":
    main()
