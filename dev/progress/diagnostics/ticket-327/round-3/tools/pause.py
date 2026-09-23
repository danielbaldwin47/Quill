#!/usr/bin/env python3
"""DEBUG-327 round 3: every first-key-after-a-pause, split, with the pause before it.

A `bursts_and_pauses` session types 25 keys at 90 ms, then waits 1.4 s, eleven
times over 300 keys. This finds each key whose handler is more than `--gap` ms
after the previous key's, splits it into wait / draw / comp, and — with
`--events` — prints every probe observation inside the pause that preceded it.

usage: pause.py <stage dir> [--session N] [--gap MS] [--events]
"""
import sys

import evidence

def main(argv):
    stages, said = evidence.flags(argv, {'--session': int, '--gap': float}, ('--events',))
    if len(stages) != 1:
        raise SystemExit('usage: pause.py <stage dir> [--session N] [--gap MS] [--events]')
    gap_ms = said.get('--gap', 500.0)
    run = evidence.Session(stages[0], said.get('--session', 0))

    followers = []
    for i, key in enumerate(run.keys):
        if i == 0 or key['present_us'] is None:
            continue
        gap = (key['handler_us'] - run.keys[i - 1]['handler_us']) / 1000.0
        if gap >= gap_ms:
            followers.append((i, gap, key))

    print(f'{run.stage} session {run.session}: {len(run.keys)} keys, '
          f'{len(followers)} after a pause of >= {gap_ms} ms')
    print(f'{"idx":>4} {"gap":>8} {"total":>7} {"wait":>7} {"draw":>7} {"comp":>7} {"frame":>7} '
          f'{"prevpaint":>10} {"paints":>6}')
    for i, gap, key in followers:
        split = run.split(key)
        if split is None:
            print(f'{i:>4} {gap:>8.1f}   (no paint probe for frame {key["frame"]})')
            continue
        wait, draw, comp, total = split
        previous = run.keys[i - 1]['handler_us']
        inside = [at for at in run.paints if previous < at < key['handler_us']]
        last = (key['handler_us'] - inside[-1]) / 1000.0 if inside else float('nan')
        print(f'{i:>4} {gap:>8.1f} {total:>7.2f} {wait:>7.2f} {draw:>7.2f} {comp:>7.2f} '
              f'{key["frame"]:>7} {last:>10.1f} {len(inside):>6}')

    if not said.get('--events'):
        return
    for i, gap, key in followers:
        handler = key['handler_us']
        previous = run.keys[i - 1]['handler_us']
        print(f'\n--- key {i}, pause from {previous} to {handler} ({gap:.1f} ms) ---')
        for event in run.probe:
            at = evidence.when(event)
            if at is not None and previous - 2000 <= at <= handler + 40000:
                print(evidence.render(event, handler))

if __name__ == '__main__':
    main(sys.argv[1:])
