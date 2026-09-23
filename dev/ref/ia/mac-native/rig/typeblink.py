#!/usr/bin/env python3
"""Watch the caret while a hand types, and after it stops.

The sampler runs on its own thread so the keystrokes and the frames share one
clock: what comes back is the accent series with the typing window marked, which
is what says whether the blink is suppressed while a writer types and how long
the suppression outlives the last key.
"""
import subprocess
import sys
import threading
import time

sys.path.insert(0, sys.path[0] or ".")
import blink
import caretat

RECT = (335, 185, 230, 45)
SECS = 14.0

rows = []
marks = []


def sample():
    t0 = time.monotonic()
    while time.monotonic() - t0 < SECS:
        f = blink.grab(RECT)
        if f is not None:
            rows.append((time.monotonic() - t0, blink.accent(f)))
    return t0


def main():
    caretat.place(2, 0)
    time.sleep(0.5)
    t0 = time.monotonic()
    th = threading.Thread(target=sample)
    th.start()
    time.sleep(2.5)
    marks.append(("type-start", time.monotonic() - t0))
    for ch in "lighthouse":
        subprocess.run(["osascript", "-e",
                        f'tell application "System Events" to keystroke "{ch}"'],
                       capture_output=True)
        time.sleep(0.18)
    marks.append(("type-end", time.monotonic() - t0))
    th.join()
    with open(sys.argv[1], "w") as fh:
        for t, n in rows:
            fh.write(f"{t:.4f}\t{n}\n")
    with open(sys.argv[1] + ".marks", "w") as fh:
        for k, t in marks:
            fh.write(f"{k}\t{t:.4f}\n")
    print("marks:", marks, "samples:", len(rows))
    # undo the typing so the document is back to the sample passage
    for _ in range(14):
        subprocess.run(["osascript", "-e",
                        'tell application "System Events" to keystroke "z" using {command down}'],
                       capture_output=True)


if __name__ == "__main__":
    main()
