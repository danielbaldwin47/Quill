#!/usr/bin/env python3
"""Write #354's capture manifest from the frames `run_style.py` left.

Every frame is named with its original, both hashes, its region, the profile it
was normalised into and the state it holds. The run shares the 2026-09-10 raw
directory with #343 and #344 — it is the same machine on the same day — and
carries a manifest of its own so the Style states can be verified alone.

    python3 rig/manifest_style_354.py

Run from the repository root.
"""
import hashlib
import json
from pathlib import Path

from PIL import Image

MAN = Path("ref/ia/mac-native/capture-2026-09-10-style.json")
NORM = Path("ref/ia/shots/mac-native")
RAW = NORM / "raw-2026-09-10"
REF = Path("ref/ia/mac-native/rig/display.icc")
CAL = Path("ref/ia/mac-native/rig/display-calibrated-2025-12-15.icc")
DRIVE = Path("ref/ia/mac-native/style-354.json")

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
    "ia_writer": "8.0.6",
    "show_scroll_bars": "Automatic",
    "window_bounds": "{0,33,1512,982}",
    "editor": "Mono; System - Default typography; Normal text size; 64-cell line length limit",
    "modes": "Library and Preview hidden; Typewriter and Authors off; Focus, Syntax and "
             "Style Check as each state names",
    "passage": "ref/style.md",
    "document": "a scratch library document — the app's own sample files were not written to",
}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def entry(name, region_pt, meta):
    norm, raw = NORM / name, RAW / name
    im = Image.open(norm)
    assert im.info["icc_profile"] == REF.read_bytes(), name
    assert im.size == tuple(2 * n for n in region_pt[2:]), (name, im.size)
    assert Image.open(raw).size == im.size, name
    return dict(normalised_file=str(norm), raw_file=str(raw), sha256=digest(norm),
                raw_sha256=digest(raw), size_px=list(im.size), region_pt=list(region_pt),
                icc=digest(REF)[:16], ticket=354, state_number=24, **meta)


def main():
    drv = json.load(open(DRIVE))
    region = tuple(drv["rig"]["region_pt"])
    caps = {name[:-4]: entry(name, region, meta) for name, meta in drv["frames"].items()}
    out = dict(run="2026-09-10, original 14-inch M1 MacBook Pro, built-in display",
               tickets=[354],
               rig=dict(RIG, region_pt=list(region), display_profile_sha256=digest(CAL),
                        normalised_into_sha256=digest(REF)),
               lists=drv["lists"], captures=dict(sorted(caps.items())))
    MAN.write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n")
    print(f"{MAN}: {len(caps)} frames")


if __name__ == "__main__":
    main()
