#!/usr/bin/env python3
"""Write #400's capture manifest from the frames `run_spell_400.py` left.

Every frame is named with its original, both hashes, its region and the profile
it was normalised into, and with the state and the switches it was shot under,
which is what makes a pair a pair.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/manifest_400.py

Run from the repository root.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image

MAN = Path("dev/ref/ia/mac-native/capture-2026-09-13-spell.json")
NORM = Path("dev/ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-13-spell"
REF = Path("dev/ref/ia/mac-native/rig/display.icc")
CAL = Path("dev/ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")
STATE = json.load(open("dev/ref/ia/mac-native/spell-400.json"))

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
    "modes": "Library and Preview hidden; Authors off; Focus, Syntax and Style Check as each "
             "state names",
    "spelling": "Edit > Spelling and Grammar: Check Spelling While Typing is the variable; "
                "Check Grammar With Spelling off; Correct Spelling Automatically on",
    "passage": "dev/ref/spell.md, the spell Piece's own state and the engine test's fixture",
    "method": "every state shot twice, once with the spelling switch on and once with it off and "
              "nothing else changed, so every number is a difference between two frames (#354's "
              "method)",
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
                               region_pt=list(region), icc=digest(REF)[:16], ticket=400,
                               notes_state=26, passage=STATE["rig"]["passage"], **meta)
    out = dict(run="2026-09-13, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[400],
               rig=dict(RIG, display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF)),
               captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
