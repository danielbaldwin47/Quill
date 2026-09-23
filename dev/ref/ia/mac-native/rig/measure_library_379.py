#!/usr/bin/env python3
"""#379's reader: the Library pane, off the frames the three passes shot.

The pane is two columns, not one — an **Organizer** and a **File List** — and
almost every number #379 asks for is an edge between two grounds or an ink
against the ground behind it. So this reads grounds first and inks second, and
finds every edge by where a long run of rows changes value rather than by a
number written here.

The pane's own right edge is **not** read as the last column the L1 pair differs
on: showing the pane reflows the page, so the two frames differ nearly
everywhere. It is read as the last column of the pane's own ground.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/measure_library_379.py

Run from the repository root.
"""
import json
import os
import sys

import numpy as np
from PIL import Image

SHOTS = os.path.join("dev", "ref", "ia", "shots", "mac-native")
PREFIX = "mac-native-28"
STATE = os.path.join("dev", "ref", "ia", "mac-native", "library-379.json")
MID = slice(400, 1600)               # rows clear of the title bar and the foot


def arr(name):
    return np.asarray(Image.open(os.path.join(SHOTS, name)).convert("RGB")).astype(np.int16)


def hexof(p):
    return "#%02x%02x%02x" % tuple(int(v) for v in p)


def columns(a):
    """Each column's own value down the middle of the window — the pane's grounds."""
    return np.median(a[MID], axis=0)


def edges(col, tol=6):
    """Where one column's value gives way to the next: the pane's vertical edges."""
    d = np.abs(np.diff(col, axis=0)).sum(axis=1)
    return [int(i) for i in np.where(d > tol)[0]]


def ink_of(a, y0, y1, x0, x1, ground, thresh=60):
    """The ink a run of rows carries against the ground behind it."""
    sub = a[y0:y1 + 1, x0:x1 + 1].reshape(-1, 3)
    far = np.abs(sub - ground).sum(axis=1)
    vals, counts = np.unique(sub[far > thresh], axis=0, return_counts=True)
    if not len(vals):
        return None
    keep = counts >= max(4, counts.max() // 10)
    i = int(np.argmax(np.where(keep, np.abs(vals - ground).sum(axis=1), -1)))
    return hexof(vals[i])


def rows_in(a, x0, x1, ground, thresh=120, gap=30, top=180):
    """Ink bands in one column of the pane — its rows, or its section heads.

    `gap` is what holds a row together: a name and the first line of its own
    excerpt are 16 px apart and one row and the next are 40, so a gap under
    30 reads three bands where there is one row and calls 64 px the pitch.
    """
    ink = (np.abs(a[:, x0:x1] - ground).sum(axis=2) > thresh).sum(axis=1) >= 3
    ink[:top] = False
    ys = np.where(ink)[0]
    out, prev = [], None
    for y in ys:
        if prev is None or y - prev > gap:
            out.append([int(y), int(y)])
        else:
            out[-1][1] = int(y)
        prev = y
    return [r for r in out if r[1] - r[0] >= 4]


def accent_bar(a, x0, x1, ground):
    """The selected row's mark: a bar at the File List's left edge, not a fill."""
    band = a[:, x0:x1 + 40]
    blue = (band[:, :, 2] > 150) & (band[:, :, 2] - band[:, :, 0] > 60)
    ys = np.where(blue.any(axis=1))[0]
    if not len(ys):
        return None
    xs = np.where(blue[ys.min():ys.max() + 1].any(axis=0))[0]
    return dict(rows=[int(ys.min()), int(ys.max())], height_px=int(ys.max() - ys.min() + 1),
                cols=[int(x0 + xs.min()), int(x0 + xs.max())],
                width_px=int(xs.max() - xs.min() + 1),
                ink=hexof(np.median(band[ys.min():ys.max() + 1][blue[ys.min():ys.max() + 1]],
                                    axis=0)))


def separator(a, y0, y1, x0, x1, ground):
    """The rule between two rows: its row, its value and the inset it runs in.

    Read between the last row of one and the first of the next, as a row whose
    median across the list differs from the list's own ground — so a descender
    reaching into it cannot be taken for a rule.
    """
    for y in range(y0 + 1, y1):
        med = np.median(a[y, x0:x1], axis=0)
        if np.abs(med - ground).sum() > 4:
            cols = np.where(np.abs(a[y, x0:x1] - ground).sum(axis=1) > 4)[0]
            return dict(row=int(y), ink=hexof(med),
                        cols=[int(x0 + cols.min()), int(x0 + cols.max())],
                        inset_left_px=int(cols.min()), inset_right_px=int(x1 - x0 - cols.max() - 1))
    return None


def search_field(a, x0, x1, ground):
    """The field at the foot of the File List: its own ground, and its prompt.

    The prompt is the one number #379 was filed over, so it is read here rather
    than off a crop: the field is the bottom of the list's column, and the prompt
    the ink standing on the field's ground.
    """
    h = a.shape[0]
    band = a[h - 80:h - 8, x0:x1]
    fg = np.median(band.reshape(-1, 3), axis=0)
    return dict(rows=[h - 80, h - 9], ground=hexof(fg),
                prompt_ink=ink_of(a, h - 70, h - 18, x0 + 30, x1, fg, thresh=25),
                is_list_ground=bool(np.abs(fg - ground).sum() <= 4))


def pane_of(name):
    """One frame's pane: its two columns, their grounds, and where each ends.

    The pane is three grounds side by side — the Organizer, the File List and
    the page — so its edges are the columns where the profile steps, and every
    width below is the distance between two of them. Nothing is assumed about
    which ground is which: they are named by the order they stand in.
    """
    a = arr(name)
    col = columns(a)
    e = [x for x in edges(col) if x > 4]
    # Consecutive columns of one step are one edge.
    steps = []
    for x in e:
        if steps and x - steps[-1][-1] <= 2:
            steps[-1].append(x)
        else:
            steps.append([x])
    at = [s[0] for s in steps]
    org_right = at[0] if at else None
    pane_right = at[1] if len(at) > 1 else None
    return dict(frame=name, steps=at[:8],
                organiser_ground=hexof(col[40]),
                organiser_right_px=org_right,
                organiser_width_pt=round((org_right + 1) / 2, 1) if org_right else None,
                list_ground=hexof(col[org_right + 40]) if org_right else None,
                list_width_pt=round((pane_right - org_right) / 2, 1)
                if org_right and pane_right else None,
                pane_right_px=pane_right,
                pane_width_pt=round((pane_right + 1) / 2, 1) if pane_right else None,
                divider_px=[at[1], at[1] + len(steps[1]) - 1] if len(steps) > 1 else None,
                page_ground=hexof(col[-40]))


def main():
    state = json.load(open(STATE)) if os.path.exists(STATE) else {}
    out = {"observations": state.get("observations", {}), "panes": {}, "rows": {}}

    for tag in ("l1-rest", "l7-bare", "l3-folder", "o1-dragged", "o1-restored"):
        name = f"{PREFIX}-light-library-{tag}.png"
        if os.path.exists(os.path.join(SHOTS, name)):
            out["panes"][tag] = pane_of(name)
    for tag in ("l1-rest",):
        name = f"{PREFIX}-dark-library-{tag}.png"
        if os.path.exists(os.path.join(SHOTS, name)):
            out["panes"]["dark/" + tag] = pane_of(name)

    # The list's rows, its inks and the selected row's bar, on the state at rest
    # and on the one with its excerpts and bars off.
    for theme, tag in (("light", "l1-rest"), ("light", "l7-bare"),
                       ("dark", "l1-rest")):
        name = f"{PREFIX}-{theme}-library-{tag}.png"
        if not os.path.exists(os.path.join(SHOTS, name)):
            continue
        a = arr(name)
        p = out["panes"][tag if theme == "light" else "dark/" + tag]
        lx = p["organiser_right_px"] + 8
        rx = p["pane_right_px"] - 8
        ground = np.array([int(p["list_ground"][i:i + 2], 16) for i in (1, 3, 5)])
        names = rows_in(a, lx + 60, rx, ground)
        pitches = [names[i + 1][0] - names[i][0] for i in range(len(names) - 1)]
        org = np.array([int(p["organiser_ground"][i:i + 2], 16) for i in (1, 3, 5)])
        # Below the window's own traffic lights, which are ink on no ground.
        heads = rows_in(a, 20, p["organiser_right_px"] - 10, org, thresh=90, top=120)
        key = tag if theme == "light" else "dark/" + tag
        out["rows"][key] = dict(
            name_rows=names[:12], pitches=pitches[:10],
            median_pitch=int(np.median(pitches)) if pitches else None,
            name_ink=ink_of(a, names[1][0], names[1][1], lx + 60, rx, ground) if len(names) > 1
            else None,
            date_ink=ink_of(a, names[1][0], names[1][1], rx - 200, rx, ground) if len(names) > 1
            else None,
            organiser_bands=heads[:10],
            organiser_head_ink=ink_of(a, heads[0][0], heads[0][1], 20,
                                      p["organiser_right_px"] - 10, org) if heads else None,
            accent=accent_bar(a, p["organiser_right_px"], p["organiser_right_px"] + 1, ground))
        out["rows"][key]["search_field"] = search_field(a, lx, rx, ground_px := np.array([int(p["list_ground"][i:i + 2], 16) for i in (1, 3, 5)]))
        if len(names) > 2:
            # A joined row is the name and its excerpt together, so the excerpt
            # is the lower two thirds of the band — not the rows under it, which
            # are the separator, and which is what an earlier reading caught.
            y0, y1 = names[1]
            out["rows"][key]["excerpt_ink"] = ink_of(a, y0 + (y1 - y0) // 3, y1,
                                                     lx + 60, rx, ground, thresh=30)
            out["rows"][key]["separator"] = separator(a, names[1][1], names[2][0], lx, rx, ground)

    before = out["panes"].get("l1-rest", {}).get("pane_width_pt")
    after = out["panes"].get("o1-dragged", {}).get("pane_width_pt")
    back = out["panes"].get("o1-restored", {}).get("pane_width_pt")
    out["observations"]["pane_width_pt"] = before
    out["observations"]["pane_width_after_drag_pt"] = after
    out["observations"]["pane_width_restored_pt"] = back
    out["observations"]["divider_drags"] = (
        None if before is None or after is None else abs(after - before) > 2)
    json.dump(out, open(os.path.join("dev", "ref", "ia", "mac-native", "library-379-read.json"), "w"),
              indent=1)
    for k, p in out["panes"].items():
        print(f"{k}: pane {p['pane_width_pt']} pt (right {p['pane_right_px']}), organiser "
              f"{p['organiser_width_pt']} pt {p['organiser_ground']}, list {p['list_ground']}, "
              f"page {p['page_ground']}")
    for k, r in out["rows"].items():
        print(f"{k}: rows {len(r['name_rows'])} pitch {r['median_pitch']} name {r['name_ink']} "
              f"date {r['date_ink']} excerpt {r.get('excerpt_ink')} head {r['organiser_head_ink']}")
        print(f"   accent {r['accent']}")
    print("observations:", {k: v for k, v in out["observations"].items()
                            if not isinstance(v, list)})


if __name__ == "__main__":
    main()
