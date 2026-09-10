#!/usr/bin/env python3
"""DEBUG-327 round 3: every accounted key against the paint that came before it.

GDK's frame clock will not start a frame until one refresh interval after the
last one was presented, so a key whose handler lands inside that interval waits
for the slot. This buckets every key by how long before its handler the previous
frame was painted, and prints what the keys in each bucket cost.

usage: paced.py <stage dir>... [--session N|all]
"""
import bisect
import sys

import evidence

BUCKETS = [(0, 2), (2, 5), (5, 10), (10, 16.7), (16.7, 50), (50, None)]

def main(argv):
    stages, said = evidence.flags(argv, {'--session': str})
    if not stages:
        raise SystemExit('usage: paced.py <stage dir>... [--session N|all]')
    asked = said.get('--session', '0')
    sessions = [0, 1] if asked == 'all' else [int(asked)]

    rows = []
    for stage in stages:
        for session in sessions:
            run = evidence.Session(stage, session)
            for key in run.keys:
                split = run.split(key)
                if split is None:
                    continue
                at = bisect.bisect_left(run.paints, key['handler_us']) - 1
                since = ((key['handler_us'] - run.paints[at]) / 1000.0
                         if at >= 0 else float('inf'))
                rows.append((since,) + split)

    print(f'{len(rows)} accounted keys with a paint probe')
    print(f'{"paint was ago":>16} {"keys":>6} {"wait p50":>9} {"wait max":>9} '
          f'{"total p50":>10} {"total p99":>10} {"total max":>10}')
    for low, high in BUCKETS:
        got = [r for r in rows if low <= r[0] and (high is None or r[0] < high)]
        if not got:
            continue
        waits = sorted(r[1] for r in got)
        totals = sorted(r[4] for r in got)
        label = f'{low:g}-{high:g} ms' if high is not None else f'>{low:g} ms'
        print(f'{label:>16} {len(got):>6} {waits[len(waits) // 2]:>9.2f} {waits[-1]:>9.2f} '
              f'{totals[len(totals) // 2]:>10.2f} {totals[int(len(totals) * 0.99)]:>10.2f} '
              f'{totals[-1]:>10.2f}')

if __name__ == '__main__':
    main(sys.argv[1:])
