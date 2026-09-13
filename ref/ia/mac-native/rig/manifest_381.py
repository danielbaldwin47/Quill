#!/usr/bin/env python3
"""Write #381's capture manifest from the frames `run_stats_381.py` left.

Every frame is named with its original, both hashes, its region and the profile
it was normalised into, and with the Toolbar setting it was shot under, which is
what makes a pair a pair.

    .venv-rig/bin/python3 ref/ia/mac-native/rig/manifest_381.py

Run from the repository root.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image

MAN = Path("ref/ia/mac-native/capture-2026-09-13-stats.json")
NORM = Path("ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-13-stats"
REF = Path("ref/ia/mac-native/rig/display.icc")
CAL = Path("ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")
STATE = json.load(open("ref/ia/mac-native/stats-381.json"))

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
    "modes": "Library and Preview hidden; Focus, Syntax, Style Check and Authors off; "
             "Typewriter only where a state names it",
    "toolbar": "View > Toolbar is the variable: Always Show for the states, Hide for their "
               "controls, Fade In/Out - the app's own setting - for the two fade frames",
    "stats_only": "not shot: four ways of pressing View > Toolbar > Stats Only all report success, "
                  "leave Default checked and change no pixel of the bar",
    "passage": "ref/sample.md",
    "method": "every state shot with the bar and again with the Toolbar hidden and nothing else "
              "changed, so the gutter above the bar is read against a page with no bar under it",
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
                               region_pt=list(region), icc=digest(REF)[:16], ticket=381,
                               notes_state=27, passage=STATE["rig"]["passage"], **meta)
    out = dict(run="2026-09-13, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[381],
               rig=dict(RIG, display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF),
                        stats_at=STATE["rig"].get("stats_at")),
               observations=STATE.get("observations", {}),
               captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
