#!/usr/bin/env python3
"""DEBUG-327 round 3: the slowest accounted keys of a run, split, with their probes.

usage: worst.py <stage dir> [--session N] [--top N] [--window MS]
"""
import bisect
import json
import sys

def load(path):
    return [json.loads(line) for line in open(path) if line.strip()]

def main(argv):
    session, top, window = 0, 3, 60.0
    stage = argv[0]
    rest = argv[1:]
    while rest:
        flag = rest.pop(0)
        if flag == '--session':
            session = int(rest.pop(0))
        elif flag == '--top':
            top = int(rest.pop(0))
        elif flag == '--window':
            window = float(rest.pop(0))
        else:
            raise SystemExit(f'worst.py: unknown flag {flag}')

    keys = load(f'{stage}/capture-{session}.jsonl')
    probe = load(f'{stage}/capture-{session}.probe.jsonl')
    paint = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'paint'}
    before = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'phase' and e['name'] == 'before'}
    paints = sorted(e['at_us'] for e in probe if e['kind'] == 'paint')

    scored = [(k['present_us'] - k['handler_us'], i, k)
              for i, k in enumerate(keys) if k['present_us'] is not None]
    scored.sort(reverse=True)
    for total, i, k in scored[:top]:
        f, h, p = k['frame'], k['handler_us'], k['present_us']
        after = paint.get(f)
        b = before.get(f, after) if after else None
        j = bisect.bisect_left(paints, h) - 1
        since = (h - paints[j]) / 1000.0 if j >= 0 else float('inf')
        print(f'\n=== key index {i}, {total / 1000:.2f} ms total, frame {f}, '
              f'previous paint {since:.2f} ms before the handler ===')
        if after:
            print(f'    wait {(b - h) / 1000:.2f}   draw {(after - b) / 1000:.2f}   '
                  f'comp {(p - after) / 1000:.2f}')
        for e in probe:
            at = e.get('at_us') or e.get('to_us') or e.get('end_us') or e.get('after_us')
            if at is None or not (h - window * 1000 <= at <= p + 5000):
                continue
            rel = (at - h) / 1000.0
            if e['kind'] == 'phase':
                print(f'{rel:>+9.2f} ms  phase {e["name"]:<12} frame {e["frame"]}')
            elif e['kind'] == 'paint':
                print(f'{rel:>+9.2f} ms  after-paint     frame {e["frame"]}')
            elif e['kind'] == 'idle':
                print(f'{rel:>+9.2f} ms  idle            frame {e["frame"]}')
            elif e['kind'] == 'tail':
                print(f'{rel:>+9.2f} ms  TAIL queue_draw frame {e["frame"]}')
            elif e['kind'] == 'busy':
                print(f'{rel:>+9.2f} ms  busy            {(e["to_us"] - e["from_us"]) / 1000.0:.2f} ms')
            elif e['kind'] == 'stage':
                print(f'{rel:>+9.2f} ms  stage {e["name"]:<12} {(e["end_us"] - e["start_us"]) / 1000.0:.2f} ms')
            elif e['kind'] == 'accept':
                print(f'{rel:>+9.2f} ms  accept {e["consumer"]:<8} frame {e["frame"]} present={e["present_us"]}')
            elif e['kind'] == 'reread':
                print(f'{rel:>+9.2f} ms  reread          frame {e["frame"]} present={e["present_us"]}')

if __name__ == '__main__':
    main(sys.argv[1:])
