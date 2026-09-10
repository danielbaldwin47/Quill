#!/usr/bin/env python3
"""DEBUG-327 round 3: every first-key-after-a-pause, split, with the pause before it.

A `bursts_and_pauses` session types 25 keys at 90 ms, then waits 1.4 s, eleven
times over 300 keys. This finds each key whose handler is more than `--gap` ms
after the previous key's, splits it into wait / draw / comp, and prints every
probe event inside the pause that preceded it.

  wait = handler -> the frame clock's `before` phase (or after-paint, absent one)
  draw = `before` -> after-paint (GTK layout, snapshot, GL render, commit)
  comp = after-paint -> presented (the compositor)

usage: pause.py <stage dir> [--session N] [--gap MS] [--events]
"""
import json
import sys

def load(path):
    return [json.loads(line) for line in open(path) if line.strip()]

def main(argv):
    stage = argv[0]
    session = 0
    gap_ms = 500.0
    events = False
    rest = argv[1:]
    while rest:
        flag = rest.pop(0)
        if flag == '--session':
            session = int(rest.pop(0))
        elif flag == '--gap':
            gap_ms = float(rest.pop(0))
        elif flag == '--events':
            events = True
        else:
            raise SystemExit(f'pause.py: unknown flag {flag}')

    keys = load(f'{stage}/capture-{session}.jsonl')
    probe = load(f'{stage}/capture-{session}.probe.jsonl')
    paint = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'paint'}
    before = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'phase' and e['name'] == 'before'}

    followers = []
    for i, k in enumerate(keys):
        if i == 0 or k['present_us'] is None:
            continue
        gap = (k['handler_us'] - keys[i - 1]['handler_us']) / 1000.0
        if gap >= gap_ms:
            followers.append((i, gap, k))

    print(f'{stage} session {session}: {len(keys)} keys, {len(followers)} after a pause of >= {gap_ms} ms')
    print(f'{"idx":>4} {"gap":>8} {"total":>7} {"wait":>7} {"draw":>7} {"comp":>7} {"frame":>7} '
          f'{"prevpaint":>10} {"paints":>6}')
    for i, gap, k in followers:
        f, h, p = k['frame'], k['handler_us'], k['present_us']
        after = paint.get(f)
        if after is None:
            print(f'{i:>4} {gap:>8.1f} {(p - h) / 1000:>7.2f}   (no paint probe for frame {f})')
            continue
        b = before.get(f, after)
        prev = keys[i - 1]['handler_us']
        inside = sorted(e['at_us'] for e in probe
                        if e['kind'] == 'paint' and prev < e['at_us'] < h)
        last = (h - inside[-1]) / 1000.0 if inside else float('nan')
        print(f'{i:>4} {gap:>8.1f} {(p - h) / 1000:>7.2f} {(b - h) / 1000:>7.2f} '
              f'{(after - b) / 1000:>7.2f} {(p - after) / 1000:>7.2f} {f:>7} {last:>10.1f} {len(inside):>6}')

    if not events:
        return
    for i, gap, k in followers:
        h = k['handler_us']
        prev = keys[i - 1]['handler_us']
        print(f'\n--- key {i}, pause from {prev} to {h} ({gap:.1f} ms) ---')
        for e in probe:
            at = e.get('at_us') or e.get('to_us') or e.get('end_us') or e.get('after_us')
            if at is None or not (prev - 2000 <= at <= h + 40000):
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

if __name__ == '__main__':
    main(sys.argv[1:])
