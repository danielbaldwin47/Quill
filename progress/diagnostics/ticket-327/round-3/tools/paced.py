#!/usr/bin/env python3
"""DEBUG-327 round 3: every accounted key against the paint that came before it.

GDK's frame clock will not start a frame until one refresh interval after the
last one was presented, so a key whose handler lands inside that interval waits
for the slot. This buckets every key by how long before its handler the previous
frame was painted, and prints what the keys in each bucket cost.

usage: paced.py <stage dir>... [--session N]
"""
import bisect
import json
import sys

def load(path):
    return [json.loads(line) for line in open(path) if line.strip()]

BUCKETS = [(0, 2), (2, 5), (5, 10), (10, 16.7), (16.7, 50), (50, 1e9)]

def main(argv):
    sessions = [0]
    if '--session' in argv:
        at = argv.index('--session')
        said = argv[at + 1]
        sessions = [0, 1] if said == 'all' else [int(said)]
        argv = argv[:at] + argv[at + 2:]
    stages = [a for a in argv if not a.startswith('--')]

    rows = []
    for stage, session in ((s, n) for s in stages for n in sessions):
        keys = load(f'{stage}/capture-{session}.jsonl')
        probe = load(f'{stage}/capture-{session}.probe.jsonl')
        paint = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'paint'}
        before = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'phase' and e['name'] == 'before'}
        paints = sorted(e['at_us'] for e in probe if e['kind'] == 'paint')
        for k in keys:
            f, h, p = k['frame'], k['handler_us'], k['present_us']
            if p is None or f not in paint:
                continue
            i = bisect.bisect_left(paints, h) - 1
            since = (h - paints[i]) / 1000.0 if i >= 0 else float('inf')
            b = before.get(f, paint[f])
            rows.append((since, (b - h) / 1000.0, (paint[f] - b) / 1000.0,
                         (p - paint[f]) / 1000.0, (p - h) / 1000.0))

    print(f'{len(rows)} accounted keys with a paint probe')
    print(f'{"paint was ago":>16} {"keys":>6} {"wait p50":>9} {"wait max":>9} '
          f'{"total p50":>10} {"total p99":>10} {"total max":>10}')
    for lo, hi in BUCKETS:
        got = [r for r in rows if lo <= r[0] < hi]
        if not got:
            continue
        waits = sorted(r[1] for r in got)
        totals = sorted(r[4] for r in got)
        label = f'{lo:g}-{hi:g} ms' if hi < 1e9 else f'>{lo:g} ms'
        print(f'{label:>16} {len(got):>6} {waits[len(waits) // 2]:>9.2f} {waits[-1]:>9.2f} '
              f'{totals[len(totals) // 2]:>10.2f} {totals[int(len(totals) * 0.99)]:>10.2f} '
              f'{totals[-1]:>10.2f}')

if __name__ == '__main__':
    main(sys.argv[1:])
