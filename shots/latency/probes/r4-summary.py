#!/usr/bin/env python3
"""Pull round 4's numbers out of the raw runs. `python3 shots/latency/probes/r4-summary.py [file...]`"""
import json, os, sys
R = 'shots/latency'
def load(n):
    p = n if os.path.sep in n else os.path.join(R, n)
    return json.load(open(p)) if os.path.exists(p) else None
def f(st, keys=('mean','sd','p50','p99','max','n')):
    if not st: return '-'
    return ' '.join(f'{k}={st.get(k)}' for k in keys if st.get(k) is not None)

for name in sys.argv[1:] or ['r4-appcost.json']:
    d = load(name)
    if not d: print(f'-- {name}: missing'); continue
    e = d.get('env', {})
    print(f'\n===== {name}  ({e.get("chromium")}, app {e.get("app",{}).get("sha256")}, load1 {e.get("load1")})')
    for r in d.get('typing', []):
        c = r['ms']['to_commit'] or {}
        a = r['ms'].get('app_cost_no_cadence') or {}
        car = r.get('caret_placed_in_the_keystrokes_own_task') or {}
        print(f'{r["regime"]:<24} acct={r["every_keystroke_accounted_for"]!s:<5} caret_static={car.get("all_static")!s:<5} '
              f'commit[mean={c.get("mean")} sd={c.get("sd")} max={c.get("max")}] app[mean={a.get("mean")} max={a.get("max")}] '
              f'sched[{f(r["ms"].get("scheduler_wait"),("mean","p99","max"))}]')
    s = d.get('sessions') or {}
    if s: print('--- pooled across sessions')
    for k, v in s.items():
        pc = v.get('pooled_to_commit') or {}
        pa = v.get('pooled_app_cost_no_cadence') or {}
        print(f'{k:<24} present[{f(v["pooled"])}]')
        print(f'{"":<24} commit[{f(pc)}] ci={v.get("pooled_to_commit_mean_ci95")} '
              f'| app_cost[{f(pa)}] ci={v.get("pooled_app_cost_no_cadence_mean_ci95")} '
              f'| acct={v["every_keystroke_accounted_for"]} caret={v.get("caret_static_in_every_keystroke")}')
    for r in d.get('typing', []):
        if r.get('input_injection', {}).get('kernel_write_to_chromium_event_ms'):
            ii = r['input_injection']
            print(f'--- uinput {r["regime"]}: delivery[{f(ii["kernel_write_to_chromium_event_ms"])}]')
            print(f'    kernel->presented[{f(ii["kernel_write_to_presented_ms"])}]')
        if r.get('display_cadence_ms', {}).get('gaps'):
            dc = r['display_cadence_ms']
            print(f'--- {r["regime"]} display gaps[{f(dc["gaps"],("mean","p50","min","max","n"))}] mult16.67={dc["gaps_that_are_a_multiple_of_16_67ms_pct"]}% phase={dc.get("phase_sd_ms_against_a_16_67ms_grid")}')
            print(f'    commit->present[{f(r.get("ms_commit_to_present"))}]')
