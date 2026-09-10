#!/usr/bin/env python3
"""DEBUG-327: for every slow key, the last pointer event and frame request before it.

usage: pointer.py <stage dir> <wayland log> <clocks line> [min_ms]
"""
import json, re, sys, datetime
stage, wlog, clocks = sys.argv[1], sys.argv[2], sys.argv[3]
min_ms = float(sys.argv[4]) if len(sys.argv) > 4 else 8.0
rt_ns, mono_ns = (int(x) for x in clocks.split()[-2:])
offset_us = (rt_ns - mono_ns) / 1000.0
ANSI = re.compile(r'\x1b\[[0-9;]*m')
LINE = re.compile(r'^\[(\d\d):(\d\d):(\d\d)\.(\d{6})\]\s*(\{[^}]*\})?\s*(discarded )?(-> )?([A-Za-z0-9_]+)#(\d+)\.(\w+)\((.*)\)$')
day = datetime.datetime.fromtimestamp(rt_ns / 1e9, datetime.timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
day_us = int(day.timestamp() * 1_000_000)
pointer, frames = [], []
for raw in open(wlog, errors='replace'):
    m = LINE.match(ANSI.sub('', raw.rstrip('\n')))
    if not m:
        continue
    h, mi, s, us, _q, _d, out, iface, oid, ev, args = m.groups()
    mono = day_us + ((int(h) * 60 + int(mi)) * 60 + int(s)) * 1_000_000 + int(us) - offset_us
    if iface == 'wl_pointer' and ev in ('enter', 'leave', 'motion', 'button'):
        pointer.append((mono, ev))
    if out and iface == 'wl_surface' and ev == 'frame':
        frames.append(mono)
print(f'pointer events: {len(pointer)} ({", ".join(sorted(set(e for _, e in pointer)))}); frame requests: {len(frames)}')
keys = [json.loads(l) for l in open(f'{stage}/capture-0.jsonl') if l.strip()]
for k in keys:
    if k['present_us'] is None:
        continue
    h = k['handler_us']
    lat = (k['present_us'] - h) / 1000
    if lat < min_ms:
        continue
    before = [p for p in pointer if p[0] <= h]
    last = f'{before[-1][1]} {(before[-1][0]-h)/1000:+.1f} ms' if before else 'none'
    recent = [f'{(f-h)/1000:+.1f}' for f in frames if h - 120_000 <= f <= h + lat * 1000]
    print(f'key {k["keycode"]} frame {k["frame"]} latency {lat:6.3f} ms: last pointer event {last}; frame requests in the 120 ms before and until presented: {" ".join(recent)}')
