#!/usr/bin/env python3
"""Writes the job file `shoot.mjs` reads.

Five states of one binary: the three the `caret` Piece is judged on (`caret`,
`selection`, `unfocused`, verbatim from `dev/shots/oracle/states.json`, so they pair
with the frozen oracle) and the "jump" pair, which is ADR 0013's own condition —
mono at 20 px on `jump.md`, where one cell is 24 device pixels and offset 10 is
the `t` of `test`, with the space before it in cell 9.

    python3 dev/shots/caret/ia/jobs.py <jobs.json> [<binary>]
"""
import json
import sys

BIN = sys.argv[2] if len(sys.argv) > 2 else "target/release/quill"

# `dev/shots/oracle/states.json`'s `defaults`, which a job's flags have to carry
# resolved: `quillArgv` reads a whole state, not a patch on one.
DEFAULTS = {
    "text": "dev/ref/sample.md", "w": 1440, "h": 900, "scale": 2, "theme": "light",
    "font": "duo", "size": 20, "focus": "off", "typewriter": False,
    "chrome": "on", "caret": 403, "select": None, "scroll": 0, "nocaret": False,
    "active": True, "typing": False, "menu": None,
}

JUMP = {"text": "dev/shots/caret/ia/jump.md", "font": "mono", "chrome": "off", "scroll": 0}


def state(**over):
    return {**DEFAULTS, **over}


VIEWS = {
    "caret": state(chrome="off"),
    "selection": state(chrome="off", select=[153, 171], caret=171),
    "unfocused": state(chrome="off", active=False),
    "jump-caret": state(caret=10, **JUMP),
    "jump-select": state(caret=14, select=[10, 14], **JUMP),
}

jobs = [{"bin": BIN, "out": f"dev/shots/caret/ia/ours-{view}.png",
         "flags": flags, "active": flags["active"]}
        for view, flags in VIEWS.items()]

with open(sys.argv[1], "w", encoding="utf-8") as f:
    json.dump(jobs, f, indent=1)
print(f"{len(jobs)} jobs")
