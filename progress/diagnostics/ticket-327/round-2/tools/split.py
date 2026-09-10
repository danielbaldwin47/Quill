#!/usr/bin/env python3
"""DEBUG-327: every accounted key of a run split into its stages.

  wait   = handler -> the frame clock's `before` phase (or after-paint when no phase probe)
  draw   = `before` -> after-paint (GTK layout, snapshot, GL render, commit)
  comp   = after-paint -> presented (compositor: fence, scheduling, its render)

usage: split.py <stage dir>...
"""
import json, sys, statistics

def load(path):
    return [json.loads(l) for l in open(path) if l.strip()]

for stage in sys.argv[1:]:
    keys = [k for k in load(f'{stage}/capture-0.jsonl') if k['present_us'] is not None]
    probe = load(f'{stage}/capture-0.probe.jsonl')
    paint = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'paint'}
    before = {e['frame']: e['at_us'] for e in probe if e['kind'] == 'phase' and e['name'] == 'before'}
    rows = []
    for k in keys:
        f, h, p = k['frame'], k['handler_us'], k['present_us']
        if f not in paint:
            continue
        b = before.get(f, paint[f])
        rows.append(((b - h) / 1000, (paint[f] - b) / 1000, (p - paint[f]) / 1000, (p - h) / 1000, k))
    if not rows:
        print(stage, 'no rows'); continue
    def col(i):
        v = sorted(r[i] for r in rows)
        return f'max {v[-1]:6.2f} p99 {v[int(len(v)*0.99)-1]:6.2f} p50 {v[len(v)//2]:5.2f}'
    print(f'{stage.split("/")[-1]}: {len(rows)} keys; wait {col(0)} | draw {col(1)} | comp {col(2)} | total {col(3)}')
    for r in sorted(rows, key=lambda r: -r[3])[:3]:
        print(f'    key {r[4]["keycode"]:3d} frame {r[4]["frame"]:4d} total {r[3]:6.2f} = wait {r[0]:6.2f} + draw {r[1]:5.2f} + comp {r[2]:6.2f}')
