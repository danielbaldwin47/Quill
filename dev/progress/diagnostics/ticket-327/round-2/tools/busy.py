#!/usr/bin/env python3
"""DEBUG-327: the longest main-loop busy spans of a run and what the probes saw inside them."""
import json, sys
stage = sys.argv[1]
top = int(sys.argv[2]) if len(sys.argv) > 2 else 8
probe = [json.loads(l) for l in open(f'{stage}/capture-0.probe.jsonl') if l.strip()]
keys = [json.loads(l) for l in open(f'{stage}/capture-0.jsonl') if l.strip()]
busy = sorted((e for e in probe if e['kind'] == 'busy'), key=lambda e: e['from_us'] - e['to_us'])
for b in busy[:top]:
    lo, hi = b['from_us'], b['to_us']
    inside = []
    for e in probe:
        if e['kind'] == 'stage' and lo <= e['start_us'] <= hi:
            inside.append(f'{e["name"]} {(e["end_us"]-e["start_us"])/1000:.2f}ms@{(e["start_us"]-lo)/1000:+.2f}')
        elif e['kind'] == 'phase' and lo <= e['at_us'] <= hi:
            inside.append(f'{e["name"]}@{(e["at_us"]-lo)/1000:+.2f}')
        elif e['kind'] in ('paint', 'idle') and lo <= e['at_us'] <= hi:
            inside.append(f'{e["kind"]}@{(e["at_us"]-lo)/1000:+.2f}')
    ks = [f'key {k["keycode"]}@{(k["handler_us"]-lo)/1000:+.2f} lat {(k["present_us"]-k["handler_us"])/1000 if k["present_us"] else None}' for k in keys if lo - 2000 <= k['handler_us'] <= hi]
    print(f'busy {(hi-lo)/1000:7.3f} ms at {lo}: {" ".join(ks)}\n    {" ".join(inside)}')
