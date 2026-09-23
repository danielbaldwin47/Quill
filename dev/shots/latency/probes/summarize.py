#!/usr/bin/env python3
"""Pull the report's numbers out of the round-2 result files."""
import json, glob, sys, os

def load(p):
    try: return json.load(open(p))
    except Exception: return None

def line(t):
    m = t['ms']['to_present']; c = t['ms']['to_commit']
    ci = t['ms'].get('to_present_ci95', {}) or {}
    p99ci = (ci.get('p99') or {})
    return (f"{t['regime']:24s} n{m['n']:4d} p50 {m['p50']:6.2f} p90 {m['p90']:6.2f} p99 {m['p99']:6.2f}"
            f" [{p99ci.get('lo95','?')}-{p99ci.get('hi95','?')}] max {m['max']:6.2f} | commit {c['p50']:5.2f}"
            f" | main {t['main_thread']['busy_ms_per_key']:5.2f} | 1frame {t['presented_within_one_60hz_frame_pct']:5.1f}%"
            f" | drops/s {t['dropped_frames_per_second']} | acct {t['every_keystroke_accounted_for']}")

for f in sys.argv[1:]:
    d = load(f)
    if not d: print('-- missing', f); continue
    print('=' * 100)
    e = d.get('env', {})
    print(f, '|', e.get('app',{}).get('sha256'), '|', e.get('document',{}).get('words'), 'words |',
          'throttle', e.get('cpu_throttle_x'), '| motion', e.get('reduced_motion'), '| load', e.get('load1'), '|', e.get('chromium'))
    for t in d.get('typing', []): print('  ', line(t))
    if d.get('long_session'):
        t = d['long_session']; print('  LONG', line(t))
        print('     quartiles', [q['p50'] for q in t.get('quartiles_to_present',[])], 'p99', [q['p99'] for q in t.get('quartiles_to_present',[])])
        print('     autosave', t['autosave'], 'heap kb', t.get('js_heap_kb_after'), 'gc us/key', t['main_thread']['gc_us_per_key'])
    if d.get('sessions'):
        for k, v in d['sessions'].items():
            print(f"  ACROSS {k:24s} p50/session {v['per_session_p50']} p99/session {v['per_session_p99']} pooled p50 {v['pooled']['p50']} p99 {v['pooled']['p99']} ci99 {v['pooled_p99_ci95']} acct {v['every_keystroke_accounted_for']}")
    for t in d.get('typing', []):
        kk = t.get('by_key_type_to_present', {})
        if kk: print('  KEYS', t['regime'], {k: (v['n'], v['p50'], v['p99'], v['max']) for k, v in kk.items()})
    if d.get('frame_control'): print('  FRAMECTL', json.dumps(d['frame_control'])[:600])
    if d.get('cold_start'): print('  COLD', json.dumps(d['cold_start'])[:900])
    if d.get('startup'):
        for k, v in d['startup'].items():
            print(f"  STARTUP {k}: fcp {v['nav_to_first_contentful_paint']} ready {v['nav_to_editor_ready']}")
