#!/usr/bin/env python3
"""DEBUG-327 round 3: the slowest accounted keys of a run, split, with their probes.

usage: worst.py <stage dir> [--session N] [--top N] [--window MS]
"""
import bisect
import sys

import evidence

def main(argv):
    stages, said = evidence.flags(argv, {'--session': int, '--top': int, '--window': float})
    if len(stages) != 1:
        raise SystemExit('usage: worst.py <stage dir> [--session N] [--top N] [--window MS]')
    top = said.get('--top', 3)
    window = said.get('--window', 60.0)
    run = evidence.Session(stages[0], said.get('--session', 0))

    scored = sorted(((key['present_us'] - key['handler_us'], i, key)
                     for i, key in enumerate(run.keys) if key['present_us'] is not None),
                    reverse=True)
    for total_us, i, key in scored[:top]:
        handler, presented = key['handler_us'], key['present_us']
        at = bisect.bisect_left(run.paints, handler) - 1
        since = (handler - run.paints[at]) / 1000.0 if at >= 0 else float('inf')
        print(f'\n=== key index {i}, {total_us / 1000:.2f} ms total, frame {key["frame"]}, '
              f'previous paint {since:.2f} ms before the handler ===')
        split = run.split(key)
        if split is not None:
            wait, draw, comp, _ = split
            print(f'    wait {wait:.2f}   draw {draw:.2f}   comp {comp:.2f}')
        for event in run.probe:
            when = evidence.when(event)
            if when is not None and handler - window * 1000 <= when <= presented + 5000:
                print(evidence.render(event, handler))

if __name__ == '__main__':
    main(sys.argv[1:])
