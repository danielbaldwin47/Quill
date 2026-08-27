#!/usr/bin/env python3
"""Pull round 3's numbers out of the raw runs. `python3 shots/latency/probes/r3-summary.py`"""
import json, os, sys, glob
R = 'shots/latency'
def load(n):
    p = os.path.join(R, n)
    return json.load(open(p)) if os.path.exists(p) else None
def f(st, keys=('mean','sd','p50','p99','max','n')):
    if not st: return '-'
    return ' '.join(f'{k}={st.get(k)}' for k in keys if st.get(k) is not None)

for name in sys.argv[1:] or ['r3-appcost.json','r3-headless.json']:
    d = load(name)
    if not d: print(f'-- {name}: missing'); continue
    print(f'\n===== {name}  ({d["env"]["chromium"]}, app {d["env"]["app"]["sha256"]}, load1 {d["env"]["load1"]})')
    for r in d.get('typing', []):
        b = r.get('bar_app_internal') or {}
        print(f'{r["regime"]:<24} acct={r["every_keystroke_accounted_for"]!s:<5} '
              f'commit[{f(r["ms"]["to_commit"])}] ')
        print(f'{"":<24} js[{f(r["ms"].get("to_js_done"))}] wait[{f(r["ms"].get("frame_wait"))}] paint[{f(r["ms"].get("to_paint"))}] present[{f(r["ms"]["to_present"])}]')
    s = d.get('sessions') or {}
    print('--- pooled across sessions')
    for k, v in s.items():
        print(f'{k:<24} present[{f(v["pooled"])}] commit[{f(v.get("pooled_to_commit"))}] paint[{f(v.get("pooled_to_paint"))}] js[{f(v.get("pooled_to_js_done"))}]')
        print(f'{"":<24} mean_ci={v.get("pooled_mean_ci95")} p99_ci={v.get("pooled_p99_ci95")} commit_mean_ci={v.get("pooled_to_commit_mean_ci95")} acct={v["every_keystroke_accounted_for"]}')
    if d.get('typing'):
        mt = d['typing'][0]['main_thread']
        print('--- main thread (first regime)', {k: mt[k] for k in mt if k != 'top'})
        print('   top:', ', '.join(f'{t["what"]}={t["us_per_key"]}' for t in mt['top'][:8]))
