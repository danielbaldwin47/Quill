#!/usr/bin/env python3
"""#379: put the fixture into iA Writer's Library, and take it out again.

The Library must hold `dev/shots/oracle/library/` without its `manifest.json` — six
files, a `Drafts` folder with two, `.archive` with one — so a judged state can
pair against a frame by name. It is **added**, never swapped in: whatever the
Library already holds is left exactly where it is, and the frames carry those
rows too.

`--remove` takes away what `--add` wrote and nothing else. It reads back the
list `--add` left, removes only those paths, and drops the two folders only if
they are empty — so a file a writer put in `Drafts` while the run was going
would stop the folder going with it.

    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/library_fixture_379.py --add
    .venv-rig/bin/python3 dev/ref/ia/mac-native/rig/library_fixture_379.py --remove

Run from the repository root. **`--remove` deletes files from the writer's own
Library**, which is why it removes a named list rather than a pattern. The list
it writes between the two is gitignored: it names what that Library held, which
is the writer's and not the repository's.
"""
import json
import os
import shutil
import sys

LIB = os.path.expanduser("~/Library/Mobile Documents/27N4MQEA55~pro~writer/Documents")
SRC = os.path.join("dev", "shots", "oracle", "library")
ADDED = os.path.join("dev", "ref", "ia", "mac-native", "library-379-added.json")
SKIP = {"manifest.json"}


def fixture():
    """Every file of the fixture, as a path relative to its root."""
    out = []
    for root, _, files in os.walk(SRC):
        for f in sorted(files):
            if f in SKIP:
                continue
            out.append(os.path.relpath(os.path.join(root, f), SRC))
    return sorted(out)


def add():
    before = sorted(os.listdir(LIB))
    written = []
    for rel in fixture():
        dst = os.path.join(LIB, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(os.path.join(SRC, rel), dst)
        written.append(rel)
    json.dump(dict(library=LIB, before=before, added=written), open(ADDED, "w"), indent=1)
    print(f"added {len(written)} files; the Library held {len(before)} before")
    return written


def remove():
    if not os.path.exists(ADDED):
        raise SystemExit(f"{ADDED} is not there: nothing to take away by name")
    state = json.load(open(ADDED))
    gone = []
    for rel in state["added"]:
        p = os.path.join(LIB, rel)
        if os.path.exists(p):
            os.remove(p)
            gone.append(rel)
    for d in sorted({os.path.dirname(r) for r in state["added"] if os.path.dirname(r)},
                    key=len, reverse=True):
        p = os.path.join(LIB, d)
        if os.path.isdir(p) and not os.listdir(p):
            os.rmdir(p)
            gone.append(d + "/")
    now = sorted(os.listdir(LIB))
    print(f"removed {len(gone)}; the Library now holds {len(now)}, and held "
          f"{len(state['before'])} before the run")
    if now != state["before"]:
        print("  ! not what it held before:", sorted(set(now) ^ set(state["before"])))
    return gone


if __name__ == "__main__":
    if "--add" in sys.argv:
        add()
    elif "--remove" in sys.argv:
        remove()
    else:
        raise SystemExit(__doc__)
