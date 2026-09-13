#!/usr/bin/env python3
"""Write #379's capture manifest from the frames the three passes left.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/manifest_379.py

Run from the repository root.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image

MAN = Path("ref/ia/mac-native/capture-2026-09-13-library.json")
NORM = Path("ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-13-library"
REF = Path("ref/ia/mac-native/rig/display.icc")
CAL = Path("ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")
STATE = json.load(open("ref/ia/mac-native/library-379.json"))

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
    "window_bounds": "{0,33,1512,982}",
    "editor": "Mono; System - Default; Normal text size; line length limit 64",
    "modes": "Preview hidden; Focus, Typewriter, Syntax, Style Check and Authors off; Library "
             "shown for every state but the L1 control",
    "library_settings": "Organizer: Favorites, Smart Folders and Hashtags all on; Files: Sort bar, "
                        "Filter bar and Text excerpts on, Pin folders to top off; Sort by Date "
                        "Modified, Newest on Top; Navigation: Tree - the app's own, as found",
    "fixture": "shots/oracle/library without manifest.json, ADDED to whatever the Library already "
               "held rather than swapped in: the four documents already there are in every frame, "
               "and the readings name the fixture's rows rather than counting all of them",
    "open_document": "sea-storm.md",
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    region = STATE["rig"]["region_pt"]
    caps = {}
    for name, meta in STATE["frames"].items():
        norm, raw = NORM / name, RAW / name
        im = Image.open(norm)
        assert im.info["icc_profile"] == REF.read_bytes(), name
        assert im.size == tuple(2 * n for n in region[2:]), (name, im.size)
        assert Image.open(raw).size == im.size, name
        caps[name[:-4]] = dict(normalised_file=str(norm), raw_file=str(raw), sha256=digest(norm),
                               raw_sha256=digest(raw), size_px=list(im.size),
                               region_pt=list(region), icc=digest(REF)[:16], ticket=379,
                               notes_state=28, **meta)
    out = dict(run="2026-09-13, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[379],
               rig=dict(RIG, display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF)),
               observations=STATE.get("observations", {}),
               captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
