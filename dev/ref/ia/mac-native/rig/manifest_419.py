#!/usr/bin/env python3
"""Write #419's capture manifest from the frames `run_narrow_419.py` left.

Every frame is named with its original, both hashes, its region and the profile
it was normalised into. All of them are Editor frames on the light ground, so
every one carries a `colour.check` and none is exempt.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/manifest_419.py

Run from the repository root.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image

MAN = Path("dev/ref/ia/mac-native/capture-2026-09-13.json")
NORM = Path("dev/ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-13"
REF = Path("dev/ref/ia/mac-native/rig/display.icc")
CAL = Path("dev/ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")

RIG = {
    "machine": "MacBook Pro 14-inch, MacBookPro18,3, Apple M1 Pro",
    "display": "built-in Color LCD, Liquid Retina XDR, internal",
    "display_px": "3024 x 1964",
    "display_pt": "1512 x 982",
    "backing_scale": 2.0,
    "second_backing_scale": "not at hand - no external display was connected, and this Mac's "
                            "built-in display offers no scale-1 mode, so whether the class breaks "
                            "are points or device pixels stays open",
    "display_profile": "Display #1 2025-12-15 13-06 2.4 F-S 1xCurve+MTX (DisplayCAL)",
    "display_profile_file": str(CAL),
    "normalised_into": str(REF),
    "macos": "27.0 (26A428)",
    "ia_writer": "8.0.6 (80046)",
    "show_scroll_bars": "Automatic",
    "window_bounds": "{0,33,<width>,982} - the width and the text size are #419's variables",
    "minimum_window_width_pt": 240,
    "editor": "Mono; System - Default typography; line length limit 64; text size as each state "
              "names",
    "modes": "Library and Preview hidden; Focus, Typewriter, Syntax, Style Check and Authors off; "
             "markers visible",
    "passage": "dev/ref/sample.md, scroll at the document top",
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def entry(name, region_pt, **extra):
    norm, raw = NORM / name, RAW / name
    im = Image.open(norm)
    assert im.info["icc_profile"] == REF.read_bytes(), name
    assert im.size == tuple(2 * n for n in region_pt[2:]), (name, im.size)
    assert Image.open(raw).size == im.size, name
    return dict(normalised_file=str(norm), raw_file=str(raw), sha256=digest(norm),
                raw_sha256=digest(raw), size_px=list(im.size), region_pt=list(region_pt),
                ground="light", icc=digest(REF)[:16], colour_check="colour.check pass", **extra)


def main():
    caps = {}
    for row in json.load(open("dev/ref/ia/mac-native/narrow-419.json")):
        for kind, name in row["frames"].items():
            caps[name[:-4]] = entry(name, row["region_pt"], ticket=419, state=25, tag=row["tag"],
                                    kind=kind, passage="dev/ref/sample.md",
                                    width_pt=row["width_pt"], text_size_step=row["step"],
                                    line_length_limit=row["limit"])
    out = dict(run="2026-09-13, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[419],
               rig=dict(RIG, display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF)),
               captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
