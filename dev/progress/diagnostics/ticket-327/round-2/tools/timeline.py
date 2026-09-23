#!/usr/bin/env python3
"""DEBUG-327: one key's timeline across the app's probes and its Wayland wire log.

usage: timeline.py <stage dir> <wayland log> <clocks line> [min_ms] [before_ms] [after_ms]
  clocks line: "realtime_ns monotonic_ns" as clocks.log records it before the run.
"""
import json, re, sys, datetime

stage, wlog, clocks = sys.argv[1], sys.argv[2], sys.argv[3]
min_ms = float(sys.argv[4]) if len(sys.argv) > 4 else 8.0
before_ms = float(sys.argv[5]) if len(sys.argv) > 5 else 40.0
after_ms = float(sys.argv[6]) if len(sys.argv) > 6 else 10.0
rt_ns, mono_ns = (int(x) for x in clocks.split()[-2:])
offset_us = (rt_ns - mono_ns) / 1000.0  # realtime_us - monotonic_us

ANSI = re.compile(r'\x1b\[[0-9;]*m')
LINE = re.compile(r'^\[(\d\d):(\d\d):(\d\d)\.(\d{6})\] \{([^}]*)\} (discarded )?(-> )?([A-Za-z0-9_]+)#(\d+)\.(\w+)\((.*)\)$')
day = datetime.datetime.fromtimestamp(rt_ns / 1e9, datetime.timezone.utc).replace(hour=0, minute=0, second=0, microsecond=0)
day_us = int(day.timestamp() * 1_000_000)

wire = []
frame_cbs = set()
feedbacks = {}
gdk = {}
GDK = re.compile(r'^Gdk-Message: \S+\s+(\d+): (.*)$')
for raw in open(wlog, errors='replace'):
    g = GDK.match(raw.rstrip('\n'))
    if g:
        gdk[int(g.group(1))] = g.group(2).strip()
        continue
    m = LINE.match(ANSI.sub('', raw.rstrip('\n')))
    if not m:
        continue
    h, mi, s, us, queue, disc, out, iface, oid, ev, args = m.groups()
    wall_us = day_us + ((int(h) * 60 + int(mi)) * 60 + int(s)) * 1_000_000 + int(us)
    mono = wall_us - offset_us
    if out and iface == 'wl_surface' and ev == 'frame':
        frame_cbs.add(re.search(r'wl_callback#(\d+)', args).group(1))
    if out and iface == 'wp_presentation' and ev == 'feedback':
        feedbacks[re.search(r'wp_presentation_feedback#(\d+)', args).group(1)] = mono
    said = None
    if iface == 'wl_callback' and ev == 'done' and oid in frame_cbs:
        t = int(args)
        said = ('frame-callback sent', t * 1000.0)
    if iface == 'wp_presentation_feedback' and ev == 'presented':
        a = [int(x) for x in args.split(',')]
        said = ('presented at', ((a[0] << 32) | a[1]) * 1e6 + a[2] / 1000.0)
    wire.append((mono, bool(out), bool(disc), iface, oid, ev, args, said))

# Sanity: wire receipt minus what the compositor said, for frame callbacks and presented events.
lags = [w[0] - w[7][1] for w in wire if w[7]]
if lags:
    lags.sort()
    print(f'offset check: {len(lags)} compositor stamps; receipt - stamp ms: min {lags[0]/1000:.3f} '
          f'p50 {lags[len(lags)//2]/1000:.3f} max {lags[-1]/1000:.3f}')
kinds = {}
for w in wire:
    kinds[(w[3], w[5])] = kinds.get((w[3], w[5]), 0) + 1
print('presented:', kinds.get(('wp_presentation_feedback', 'presented'), 0),
      'discarded:', kinds.get(('wp_presentation_feedback', 'discarded'), 0),
      'frame-callbacks:', len(frame_cbs), 'commits:', kinds.get(('wl_surface', 'commit'), 0))

keys = [json.loads(l) for l in open(f'{stage}/capture-0.jsonl') if l.strip()]
probe = [json.loads(l) for l in open(f'{stage}/capture-0.probe.jsonl') if l.strip()]

def at(e):
    return e.get('at_us') or e.get('start_us') or e.get('before_us') or e.get('from_us')

for k in keys:
    if k['present_us'] is None:
        continue
    lat = (k['present_us'] - k['handler_us']) / 1000
    if lat < min_ms:
        continue
    h = k['handler_us']
    lo, hi = h - before_ms * 1000, k['present_us'] + after_ms * 1000
    print(f'\n=== key {k["keycode"]} handler {h} frame {k["frame"]} present {k["present_us"]} latency {lat:.3f} ms; '
          f'times below are ms from the handler')
    rows = []
    for e in probe:
        t = at(e)
        if t is None or not (lo <= t <= hi):
            continue
        if e['kind'] == 'stage':
            rows.append((t, f'probe stage {e["name"]} {(e["end_us"]-e["start_us"])/1000:.3f} ms'))
        elif e['kind'] in ('paint', 'idle'):
            rows.append((t, f'probe {e["kind"]} frame {e["frame"]}'))
        elif e['kind'] == 'busy':
            rows.append((e['from_us'], f'main loop busy {(e["to_us"]-e["from_us"])/1000:.3f} ms, until {(e["to_us"]-h)/1000:+.3f}'))
        elif e['kind'] == 'phase':
            rows.append((t, f'probe phase {e["name"]} frame {e["frame"]} frame_time {(e["frame_time_us"]-h)/1000:+.3f} predicted {(e["predicted_us"]-h)/1000 if e["predicted_us"] else None}'))
        elif e['kind'] == 'accept':
            rows.append((t, f'probe accept {e["consumer"]} frame {e["frame"]} complete={e["complete"]} present={e["present_us"]} ({(e["present_us"]-h)/1000:+.3f})'))
        elif e['kind'] == 'reread':
            rows.append((t, f'probe reread frame {e["frame"]} present={e["present_us"]} ({(e["present_us"]-h)/1000:+.3f})'))
    for kk in keys:
        if lo <= kk['handler_us'] <= hi:
            rows.append((kk['handler_us'], f'KEY {kk["keycode"]} handler (frame {kk["frame"]}, presented {(kk["present_us"]-kk["handler_us"])/1000 if kk["present_us"] else None} ms later)'))
    for w in wire:
        if lo <= w[0] <= hi:
            arrow = '->' if w[1] else '<-'
            extra = ''
            if w[7]:
                extra = f'   [{w[7][0]} {(w[7][1]-h)/1000:+.3f}]'
            args = w[6] if len(w[6]) < 70 else w[6][:67] + '...'
            rows.append((w[0], f'wire {arrow} {"DISCARDED " if w[2] else ""}{w[3]}#{w[4]}.{w[5]}({args}){extra}'))
    rows.sort()
    for t, text in rows:
        print(f'{(t-h)/1000:+9.3f}  {text}')
    for fr in (k['frame'] - 1, k['frame'], k['frame'] + 1):
        if fr in gdk:
            print(f'   gdk frame {fr}: {gdk[fr]}')
