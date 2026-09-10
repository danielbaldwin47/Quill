#!/usr/bin/env python3
"""Write the 2026-09-10 capture manifest from the frames the two runs left.

Every frame is named with its original, both hashes, its region, the profile it
was normalised into and what its colour check could say. Preview paints its own
paper, so a Preview frame carries no § 4.2 check and the Editor control shot
beside it does.

    python3 rig/manifest_2026_09_10.py

Run from the repository root.
"""
import hashlib
import json
import os
from pathlib import Path

from PIL import Image

MAN = Path("ref/ia/mac-native/capture-2026-09-10.json")
NORM = Path("ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-10"
REF = Path("ref/ia/mac-native/rig/display.icc")
CAL = Path("ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")

RIG = {
    "machine": "MacBook Pro 14-inch, MacBookPro18,3, Apple M1 Pro",
    "display": "built-in Color LCD, Liquid Retina XDR, internal",
    "display_px": "3024 x 1964",
    "display_pt": "1512 x 982",
    "backing_scale": 2.0,
    "display_profile": "Display #1 2025-12-15 13-06 2.4 F-S 1xCurve+MTX (DisplayCAL)",
    "display_profile_file": str(CAL),
    "normalised_into": str(REF),
    "macos": "27.0 (26A428)",
    "ia_writer": "8.0.6 (80046)",
    "show_scroll_bars": "Automatic",
    "window_bounds": "{0,33,<width>,982} — the width is the variable of #344",
    "editor": "Mono; System - Default typography; text size and line length limit as each state names",
    "modes": "Library hidden; Focus, Typewriter, Syntax, Style Check and Authors off; "
             "Preview hidden except the #343 Preview states",
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def entry(name, region_pt, ground, check, **extra):
    norm, raw = NORM / name, RAW / name
    im = Image.open(norm)
    assert im.info["icc_profile"] == REF.read_bytes(), name
    assert im.size == tuple(2 * n for n in region_pt[2:]), (name, im.size)
    assert Image.open(raw).size == im.size, name
    return dict(normalised_file=str(norm), raw_file=str(raw), sha256=digest(norm),
                raw_sha256=digest(raw), size_px=list(im.size), region_pt=list(region_pt),
                ground=ground, icc=digest(REF)[:16], colour_check=check, **extra)


def main():
    caps = {}
    for row in json.load(open("ref/ia/mac-native/narrow-344.json")):
        for kind, name in row["frames"].items():
            caps[name[:-4]] = entry(name, row["region_pt"], "light", "colour.check pass",
                                    ticket=344, state=22, tag=row["tag"], kind=kind,
                                    passage="ref/sample.md", width_pt=row["width_pt"],
                                    text_size_step=row["step"], line_length_limit=row["limit"])
    tem = json.load(open("ref/ia/mac-native/templates-343.json"))
    for name, item in tem["frames"].items():
        check = ("colour.check pass" if item["surface"] == "editor"
                 else "n/a - Preview paints its own paper, the Editor control beside it is checked")
        caps[name[:-4]] = entry(name, tem["rig"]["region_pt"], "dark", check,
                                ticket=343, state=23, passage=item["passage"],
                                surface=item["surface"], template=item["template"],
                                text_size_step=5, line_length_limit=64)
    out = dict(run="2026-09-10, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[343, 344],
               rig=dict(RIG, display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF)),
               captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
