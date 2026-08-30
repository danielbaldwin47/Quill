# THROWAWAY (#147): writes the shot list `tools/shoot147.mjs` reads.
#
# Four shapes of the caret's column x four shots: the two judged states the
# Piece is won on, and the "jump" pair the ticket measured its 9 device px on.
# Shape 1 is shot twice — once from the unpatched binary saved before the
# geometry patch, once from the patched one with no shape asked for — so the
# sheets can show that the switch itself moves no pixels.
#
#   python3 shots/caret/147/jobs147.py <jobs.json> [<unpatched binary>]
#
# The unpatched binary is the one built before `shapes.patch` was applied; it
# is what shape 1 is shot from, so that the shots the sheets call "as-is" came
# from code with none of the patch in it. Without one, shape 1 is shot from the
# patched build with no shape asked for, which the verify script shows is the
# same pixels either way.
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.dirname(os.path.abspath(__file__)))))
PATCHED = f"{ROOT}/target/release/quill"
BASELINE = sys.argv[2] if len(sys.argv) > 2 else PATCHED

DEFAULTS = {
    "text": "ref/sample.md", "w": 1440, "h": 900, "theme": "light", "font": "duo",
    "size": 20, "focus": "off", "typewriter": False, "chrome": "on", "caret": 403,
    "select": None, "scroll": 0, "nocaret": False, "active": True,
}


def state(**over):
    s = dict(DEFAULTS)
    s.update(over)
    return s


# The three views. `caret` and `selection` are the judged states verbatim
# (`shots/oracle/states.json`, `pieces.caret`), so they pair with the frozen
# oracle. The jump pair is the ticket's own condition: mono at 20 px, where one
# cell is 24 device px, and offset 10 is the `t` of `test` with the space
# before it in cell 9.
JUMP = {"text": "shots/caret/147/jump.md", "font": "mono", "chrome": "off", "scroll": 0}
VIEWS = {
    "caret": state(chrome="off"),
    "selection": state(chrome="off", select=[153, 171], caret=171),
    "unfocused": state(chrome="off", active=False),
    "jump-caret": state(caret=10, **JUMP),
    "jump-select": state(caret=14, select=[10, 14], **JUMP),
}

jobs = []
for view, flags in VIEWS.items():
    jobs.append({"bin": BASELINE, "out": f"shots/caret/147/s1base-{view}.png",
                 "flags": flags, "active": flags["active"]})
for shape in (1, 2, 3, 4, 5):
    for view, flags in VIEWS.items():
        jobs.append({"bin": PATCHED, "out": f"shots/caret/147/s{shape}-{view}.png",
                     "flags": flags, "active": flags["active"],
                     "env": {"QUILL_CARET_SHAPE": str(shape)}})

json.dump(jobs, open(sys.argv[1], "w"), indent=1)
print(f"{len(jobs)} jobs")
