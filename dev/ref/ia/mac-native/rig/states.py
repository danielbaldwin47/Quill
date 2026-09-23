#!/usr/bin/env python3
"""Drive iA Writer through the ticket's states and keep one frame of each.

Every state starts from `reset()` — the sample passage laid down fresh and the
caret at Home — so no state depends on the one before it, which is what the
ticket's Method asks for.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, sys.path[0] or ".")
import iarig as R

REGION = (0, 48, 1470, 380)          # logical points, below the menu bar, inside the scrollbar
SAMPLE = os.environ.get("IA_SAMPLE", "dev/ref/sample.md")


def osa(script):
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout


def act():
    osa('tell application "iA Writer" to activate')
    time.sleep(0.45)


def keyc(code, mods=None):
    m = f" using {{{mods}}}" if mods else ""
    osa(f'tell application "System Events" to key code {code}{m}')


def keys(ch, mods=None):
    m = f" using {{{mods}}}" if mods else ""
    osa(f'tell application "System Events" to keystroke "{ch}"{m}')


def repeat(code, n, mods=None):
    if n <= 0:
        return
    m = f" using {{{mods}}}" if mods else ""
    osa(f'tell application "System Events" to repeat {n} times\n key code {code}{m}\nend repeat')


def reset(empty=False):
    """Sample passage laid down fresh, caret at the top of the document."""
    act()
    keys("a", "command down")
    time.sleep(0.25)
    if empty:
        keyc(51)                      # delete
        time.sleep(0.4)
        return
    subprocess.run(["bash", "-c", f"pbcopy < {SAMPLE}"], check=True)
    time.sleep(0.25)
    keys("v", "command down")
    time.sleep(0.9)
    keyc(126, "command down")         # top of document
    time.sleep(0.5)


def goto(downs=0, rights=0, shift_rights=0, shift_downs=0):
    repeat(125, downs)
    time.sleep(0.2)
    keyc(123, "command down")         # start of line
    time.sleep(0.3)
    repeat(124, rights)
    time.sleep(0.2)
    repeat(125, shift_downs, "shift down")
    repeat(124, shift_rights, "shift down")
    time.sleep(0.6)


def shoot_state(out, region=None, burst=True):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if burst:
        best, scores = R.burst(region or REGION, out)
        return best, scores
    R.shoot(region or REGION, out)
    return None, None
