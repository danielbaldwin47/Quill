#!/usr/bin/env python3
"""Has anybody touched this machine in the last N seconds?

Owner: latency piece. Used by dev/legacy/bin/quill --panel, which needs the measured window to be VISIBLE on
the physical panel (a Wayland surface on a workspace that is not being displayed gets no frame
callbacks, so its presentation timestamps mean nothing) — and dev/legacy/BRIEF.md says test windows must not
land in front of somebody who is working. So: watch every real keyboard/mouse evdev node, and
exit non-zero the moment one of them says a human is there.

  python3 tools/idle-check.py [seconds]     0 = silent for that long, 1 = somebody is using it
"""
import glob, os, select, sys, time

def main():
    window = float(sys.argv[1]) if len(sys.argv) > 1 else 8.0
    # Only real keyboards and pointers. A game controller's motion sensors stream accelerometer
    # samples forever whether or not anybody is in the room, and would make every check fail.
    wanted = {}
    block = ''
    for line in open('/proc/bus/input/devices'):
        if line.startswith('N: Name='): block = line.split('=', 1)[1].strip().strip('"')
        if line.startswith('H: Handlers='):
            h = line.split('=', 1)[1].split()
            ev = [x for x in h if x.startswith('event')]
            if ev and ({'kbd', 'mouse'} & set(h)) and 'quill-latency-bench' not in block:
                wanted['/dev/input/' + ev[0]] = block
    fds, names = [], {}
    for p, nm in sorted(wanted.items()):
        try:
            fd = os.open(p, os.O_RDONLY | os.O_NONBLOCK)
        except OSError:
            continue
        fds.append(fd); names[fd] = nm
    if not fds:
        print('idle-check: no readable input devices; cannot tell', file=sys.stderr)
        return 1
    end = time.monotonic() + window
    try:
        while True:
            left = end - time.monotonic()
            if left <= 0: break
            r, _, _ = select.select(fds, [], [], left)
            for fd in r:
                data = os.read(fd, 4096)
                if data:
                    print('idle-check: activity on %s' % names[fd], file=sys.stderr)
                    return 1
    finally:
        for fd in fds: os.close(fd)
    return 0

if __name__ == '__main__':
    sys.exit(main())
