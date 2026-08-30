#!/usr/bin/env python3
"""Put the caret at a known offset on a known line and keep the brightest frame."""
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import iarig as R

REGION = (0, 48, 1512, 380)


def osa(script):
    subprocess.run(["osascript", "-e", script], capture_output=True)


def place(downs, rights, shift_rights=0):
    osa('tell application "iA Writer" to activate')
    time.sleep(0.4)
    osa('tell application "System Events" to key code 126 using {command down}')
    time.sleep(0.4)
    for _ in range(downs):
        osa('tell application "System Events" to key code 125')
    time.sleep(0.25)
    osa('tell application "System Events" to key code 123 using {command down}')
    time.sleep(0.35)
    if rights:
        osa(f'tell application "System Events" to repeat {rights} times\n key code 124\nend repeat')
    if shift_rights:
        osa(f'tell application "System Events" to repeat {shift_rights} times\n'
            f' key code 124 using {{shift down}}\nend repeat')
    time.sleep(0.6)


if __name__ == "__main__":
    downs, rights, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
    place(downs, rights)
    best, scores = R.burst(REGION, out)
    print(f"{out.split('/')[-1]}: best={best} accent px  scores={scores}")
