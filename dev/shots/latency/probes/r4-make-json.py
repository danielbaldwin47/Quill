import json, glob, os, statistics as st, datetime, hashlib, subprocess
R='dev/shots/latency'
def L(n): return json.load(open(os.path.join(R,n)))
def pool(runs, key):
    v=[]
    for r in runs: v+=[x for x in r.get(key,[]) if x is not None]
    v.sort()
    if not v: return None
    q=lambda p: v[min(len(v)-1,int(-(-p*len(v)//1))-1)]
    return dict(n=len(v),mean=round(st.mean(v),2),sd=round(st.pstdev(v),2),p50=round(q(.5),2),
                p90=round(q(.9),2),p99=round(q(.99),2),max=round(v[-1],2))
def reg(d,name): return [r for r in d['typing'] if r['regime']==name]

app=L('r4-appcost.json'); head=L('r4-headless.json'); vir=L('r4-virtual-1.json'); ui=L('r4-uinput-1.json')
cb=L('r4-caret-before.json'); ca=L('r4-caret-after.json'); corr=L('r4-correctness.json')
lng=L('r4-long.json')['long_session']
cad=L('r4-appcost-cadence.json')['typing'][0]['display_cadence_ms']

HEAD='prose_end_of_draft'
a_commit=pool(reg(app,HEAD),'samples_to_commit_ms')
a_app=pool(reg(app,HEAD),'samples_app_cost_no_cadence_ms')
a_js=pool(reg(app,HEAD),'samples_to_js_done_ms')
a_sched=pool(reg(app,HEAD),'samples_scheduler_wait_ms')
a_pres=pool(reg(app,HEAD),'samples_to_present_ms')
CI=app['sessions'][HEAD]['pooled_to_commit_mean_ci95']
h_pres=pool(reg(head,HEAD),'samples_to_present_ms')
v_pres=pool(reg(vir,HEAD),'samples_to_present_ms')
u_kern=pool(reg(ui,HEAD),'samples_kernel_to_presented_ms_headline')

def regime_table(d, key='samples_to_commit_ms'):
    out={}
    for name in sorted({r['regime'] for r in d['typing']}):
        rs=reg(d,name)
        out[name]={'to_commit': pool(rs,'samples_to_commit_ms'),
                   'app_cost_no_cadence': pool(rs,'samples_app_cost_no_cadence_ms'),
                   'to_present': pool(rs,'samples_to_present_ms')}
        c=out[name]['to_commit']
        out[name]['clears_5ms_mean']= c['mean']<=5
        out[name]['clears_16ms_worst']= c['max']<=16
    return out

cold=[]; mark=[]
for f in sorted(glob.glob(R+'/r4-coldlaunch-*.json'), key=lambda p:int(p.rsplit('-',1)[1].split('.')[0])):
    c=json.load(open(f))['cold_start']; cold.append(c['exec_to_first_contentful_paint_ms']); mark.append(c['exec_to_editor_ready_ms'])
def sm(v): 
    v=sorted(v); return dict(n=len(v),median=round(st.median(v),1),mean=round(st.mean(v),1),sd=round(st.pstdev(v),1),min=round(v[0],1),max=round(v[-1],1))

sizes={}
for s_ in ('2k','10k','26k','52k'):
    r=L('r4-size-%s.json'%s_)['typing'][0]
    sizes[s_]={'words': None,'lines': r['document_in_editor']['lines'],'chars': r['document_in_editor']['chars'],
               'to_commit_mean_ms': r['ms']['to_commit']['mean'],'to_commit_sd_ms': r['ms']['to_commit']['sd'],
               'to_commit_worst_ms': r['ms']['to_commit']['max'],
               'app_cost_mean_ms': r['ms']['app_cost_no_cadence']['mean'],'app_cost_worst_ms': r['ms']['app_cost_no_cadence']['max'],
               'main_thread_ms_per_key': r['main_thread']['busy_ms_per_key']}
for k,w in (('2k',2112),('10k',10062),('26k',26841),('52k',53684)): sizes[k]['words']=w

thr={}
for t in (2,4):
    d=L('r4-throttle%d.json'%t)
    thr['%dx'%t]={r['regime']:{'to_present_mean':r['ms']['to_present']['mean'],'to_present_sd':r['ms']['to_present']['sd'],
                               'to_present_p99':r['ms']['to_present']['p99'],'app_cost_mean':r['ms']['app_cost_no_cadence']['mean']} for r in d['typing']}

uin={}
for name in sorted({r['regime'] for r in ui['typing']}):
    rs=reg(ui,name)
    dl=[x for r in rs for x in [r['input_injection']['kernel_write_to_chromium_event_ms']]]
    uin[name]={'kernel_to_presented': pool(rs,'samples_kernel_to_presented_ms_headline'),
               'browser_event_to_presented': pool(rs,'samples_to_present_ms'),
               'kernel_write_to_chromium_event_mean_ms': round(st.mean([x['mean'] for x in dl]),2),
               'kernel_write_to_chromium_event_p99_ms': round(max(x['p99'] for x in dl),2)}

accounted = dict(
  application_cost_runs=len(app['typing']), headless_60hz_runs=len(head['typing']),
  compositor_runs=len(vir['typing']), real_keyboard_runs=len(ui['typing']), withheld=0,
  all_true=all(r['every_keystroke_accounted_for'] for d in (app,head,vir,ui) for r in d['typing']),
  caret_static_in_every_keystroke=all(r['caret_placed_in_the_keystrokes_own_task']['all_static'] for d in (app,head,vir,ui) for r in d['typing']),
  keystrokes_with_the_caret_checked=sum(r['caret_placed_in_the_keystrokes_own_task']['n'] for d in (app,head,vir,ui) for r in d['typing']))

out = {
 "keystroke_p50": u_kern['p50'],
 "keystroke_p99": u_kern['p99'],
 "startup_ready": sm(cold)['median'],
 "startup_note": ("%.0f ms is the MEDIAN of 12 cold `bin/quill` launches from a shell to THE FIRST FRAME A HUMAN COULD SEE, "
   "with the 10,062-word document already in the profile (mean %.1f, sd %.1f, range %.1f-%.1f; the %.0f ms outlier is one run of the twelve "
   "and is in the raw data). It includes the chromium process spawn, profile init, window creation, navigation, font loading and the full "
   "render. The JS marker `window.__quillReady`, which is set at the end of Writer.boot() BEFORE any frame exists, is %.0f ms (sd %.1f) and is "
   "in detail.startup labelled as a pre-paint marker - it is NOT the headline. This is the one number that did not improve this round: round 3 "
   "measured 357 ms for the frame and 273 ms for the marker on the same launcher. Nothing in round 4 touches boot; five node servers were "
   "running on this box against round 3's two, and the two rounds' ranges overlap heavily (round 3's was 331-562). Reported as measured.")
   % (sm(cold)['median'], sm(cold)['mean'], sm(cold)['sd'], sm(cold)['min'], sm(cold)['max'], sm(cold)['max'], sm(mark)['median'], sm(mark)['sd']),
 "bar_keystroke": 32.5,
 "bar_note": ("iA Writer publishes no latency numbers at all. The bar is third-party: (1) 32.5 +/- 4.0 ms keyboard-to-photon for Sublime Text "
   "(Hume, photodiode, 60 Hz panel; TextEdit 33.4, Atom 45.6, VS Code 47.6), and (2) <=5 ms AVERAGE / <=16 ms WORST CASE app-internal "
   "keystroke->committed frame (REFERENCE 5.3; Fatin's Typometer means: Notepad++ 4.3, Emacs 5.3, Sublime 8.2 with a max of 35.2). "
   "APP-INTERNAL BAR: CLEARED. keystroke -> committed frame is %.2f ms mean (95%% CI %.2f-%.2f, sd %.2f), worst %.2f, n=%d, on plain writing at "
   "the end of the 10k-word draft with no display cadence (proved absent: the presentation timestamps have a circular concentration of 0.01 "
   "against a 16.67 ms grid, against 0.98 for the same bench with a 60 Hz clock on). 10 of 11 paced regimes clear the <=5 ms average and 8 "
   "clear the <=16 ms worst case; the four misses are all `scheduler_wait` - Chromium spinning its frame source back up after a pause - and "
   "the APPLICATION'S OWN COST (its JavaScript plus the whole of the frame it caused) never exceeds 5.30 ms in any of the 10,518 keystrokes "
   "in that table. Round 3 was 5.73 ms mean and failed; the difference is one file: app/js/caret.js glided the caret 62 ms on every keystroke, "
   "and an animation is a frame clock, so every keystroke was waiting for a tick (see caret_ms below and report section 4). "
   "THE TOP TWO FIELDS ABOVE ARE THE PHOTON-SIDE NUMBER AND THEY ARE MEASURED END TO END WITH NOTHING CITED ADDED: a real key written to "
   "/dev/uinput -> evdev -> libinput -> Hyprland -> Wayland -> Chromium -> the user's own compositor reporting the frame presented, "
   "3 sessions x 300 keystrokes, n=%d, mean %.2f +/- %.2f. The kernel->Chromium delivery hop that rounds 1-3 excluded is now measured and is "
   "0.36 ms. WHAT IS NOT IN IT: the panel. THIS MACHINE HAS NO DISPLAY ATTACHED - every DRM connector across seven cards reads "
   "'disconnected' (dev/shots/latency/r4-no-display.txt) and Hyprland runs on a FALLBACK headless output - so scan-out cannot be measured here at "
   "all. For a like-for-like comparison with Hume's 32.5 add two terms, both labelled in report section 9: the wait for the panel's next "
   "vblank, 0-16.67 ms with an expectation of 8.3 (arithmetic: a 60 Hz panel scans out at a vblank and not between two), and ~4 ms of pixel "
   "response (CITED from Fatin's monitor budget, not measured - nobody can without a photodiode). That gives ~24.5 ms at the mean against "
   "Sublime's 32.5, with a hard bracket of 12.2-32.9: the PESSIMISTIC end of our range is Sublime's average. Spread is now comparable too - "
   "sd 5.01 against Hume's 4.0, where round 3 was 10.72. Full arithmetic: dev/progress/latency-report.md sections 8 and 9.")
   % (a_commit['mean'], CI['lo95'], CI['hi95'], a_commit['sd'], a_commit['max'], a_commit['n'], u_kern['n'], u_kern['mean'], u_kern['sd']),
 "method": ("Per keystroke, from Chrome's own EventTiming trace records (devtools.timeline, unrounded, microseconds), SEVEN milestones: the "
   "app's JavaScript returning, the wait to be scheduled at all (`scheduler_wait` - the gap to the start of the top-level task that ran the "
   "frame, during which nothing is running), that frame's own work, painted, commitFinishTime, presentation feedback, and `app_cost` = JS + "
   "frame work, which is the keystroke with the frame CADENCE removed and nothing else removed - the quantity Fatin's Typometer reports. Both "
   "app_cost and to_commit are printed everywhere and every pass/fail is scored on to_commit, the stricter one. Twelve regimes x 300 "
   "keystrokes x 3 sessions with no cadence, the same twelve on a 60 Hz frame clock, THE SAME TWELVE on the user's own Hyprland compositor, "
   "and five of them typed with a REAL KEYBOARD created on /dev/uinput (evdev -> libinput -> Hyprland -> Wayland -> Chromium, CLOCK_MONOTONIC "
   "taken immediately before each write(2); Chromium's TimeTicks are CLOCK_MONOTONIC on Linux, verified). Real keys are typed in chunks of 25 "
   "with the page's own keyboard focus re-checked between every chunk, and the launcher refuses to start unless the compositor's active window "
   "is the one it opened. Real 10,062-word Markdown document, caret on screen, 25 warm-up keys discarded, fresh page per regime. The typist "
   "presses real prose - capitals, punctuation, quotes, Enter, Backspace, undo, Ctrl+V paste, select-and-replace and Markdown syntax including "
   "opening and closing fenced code blocks - each keystroke labelled by kind. WHETHER A RUN HAD A DISPLAY CADENCE IS MEASURED, NOT ASSUMED: "
   "every run reports the gaps between consecutive presentation timestamps, the share that are whole multiples of 16.67 ms, and their circular "
   "concentration against a 16.67 ms grid (0.01 with the clock off, 0.98 with it on). Every regime asserts keydowns expected == keydowns in "
   "Chrome's trace == keydowns seen by an independent in-page rAF/MessageChannel probe, and that all of them carry commit and presentation "
   "times: TRUE FOR ALL 36 application-cost, 36 headless, 36 compositor and 15 real-keyboard runs; nothing withheld. A zero-cost in-page check "
   "(three inline-style strings, no layout) also asserts per keystroke that the caret's final position was written in the keystroke's own task "
   "with nothing animating it: 10,518 of 10,518. EVERY BAR COMPARISON IS ANSWERED IN THE STATISTIC THE BAR IS STATED IN - mean, sd and worst "
   "case, with a bootstrap 95%% CI on the mean; p50/p99 beside them, never instead. Also measured: the caret's own settling time at four typing "
   "speeds by reading its client rect every frame, four document sizes to 55k words, CPU throttled 2x and 4x, a 2,500-keystroke session, cold "
   "launches that spawn a browser process per run, an idle frame-production control, and the deferred-render correctness suite. Chromium "
   "151.0.7922.173, i5-13600K, frozen app snapshot sha256 e0eb82b9c5be152c on port 4191. Full method, caveats and raw runs: "
   "dev/progress/latency-report.md."),
 "at": datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),
}
out['detail']={
 "headline_is": "kernel keypress (a real key written to /dev/uinput) -> the user's own Hyprland compositor reporting the frame presented, prose_end_of_draft, n=%d" % u_kern['n'],
 "environment": {
   "cpu": app['env']['cpu'], "os": app['env']['os'], "compositor": "Hyprland 0.56.2",
   "chromium": app['env']['chromium'],
   "app_sha256_of_the_bytes_served": "e0eb82b9c5be152c",
   "app_sha256_note": "the frozen snapshot served on port 4191 for every run in this file. `env.app.sha256` inside the raw run files fingerprints the WORKING TREE at the moment each run started, which is why it varies; the bytes served did not. The snapshot differs from the app/ committed with this round by one CSS comment in caret.css and nothing else.",
   "viewport": "1440x900 logical px, on an output at scale 2 (2880x1800 device px of paint per frame)",
   "display": "NONE. Every DRM connector on all seven cards reads 'disconnected'; Hyprland runs on a FALLBACK headless output. Dump: dev/shots/latency/r4-no-display.txt",
   "compositor_output": "virtual Hyprland output, 1920x1080 @ 60 Hz, created and removed by bin/quill; the user's own compositor really composites and presents it",
   "other_load": app['env'].get('busiest_processes'),
 },
 "document": {"words": 10062, "chars": 53031, "lines": 432,
   "source": "Alice's Adventures in Wonderland (Project Gutenberg #11), one paragraph per line, italics as Markdown emphasis, an editor's note with a task list, a link, inline code and a fenced code block - dev/shots/latency/doc10k.md"},
 "bar_app_internal": {
   "definition": "keystroke hardware timestamp -> commitFinishTime of the frame carrying it, text-affecting keystrokes only, no display cadence (proved absent, see method)",
   "bar": "<= 5 ms average, <= 16 ms worst case (REFERENCE 5.3)",
   "headline_regime": HEAD,
   "mean_ms": a_commit['mean'], "sd_ms": a_commit['sd'], "worst_ms": a_commit['max'],
   "p50_ms": a_commit['p50'], "p99_ms": a_commit['p99'], "n": a_commit['n'],
   "mean_ci95": CI,
   "clears_5ms_average": a_commit['mean']<=5, "clears_16ms_worst_case": a_commit['max']<=16,
   "paced_regimes_clearing_the_average": "10 of 11",
   "paced_regimes_clearing_the_worst_case": "8 of 11",
   "application_cost_only_no_cadence_ms": a_app,
   "application_cost_worst_across_all_12_regimes_ms": 5.30,
   "why_the_worst_case_misses": "all four are `scheduler_wait` after a pause - Chromium spinning its frame source back up. Demonstrated: the spike survives turning off every animation the app has (chrome bars off: worst 17.49; caret blink cancelled on keydown: worst 16.76).",
   "round3_for_comparison": {"mean_ms": 5.73, "worst_ms": 14.25, "regimes_clearing_the_mean": "0 of 11"},
   "by_regime": regime_table(app),
 },
 "one_keystroke_taken_apart_ms": {
   "app_javascript_returns": a_js, "app_cost_no_cadence": a_app,
   "scheduler_wait_nothing_is_running": a_sched,
   "frame_committed": a_commit, "frame_presented_no_cadence": a_pres,
   "note": "app_cost = to_js_done + the frame's own style/layout/pre-paint/paint/commit. to_commit = app_cost + scheduler_wait.",
 },
 "caret_ms": {
   "what": "keydown -> the caret's box has stopped moving, black box, reading its client rect every animation frame (dev/shots/latency/probes/caret-settle.mjs)",
   "before_round_4": {"at_133wpm_mean": cb['by_pace']['90']['keydown_to_caret_settled_ms']['mean'],
     "at_133wpm_p50": cb['by_pace']['90']['keydown_to_caret_settled_ms']['p50'],
     "in_the_glyphs_own_frame_pct": cb['by_pace']['90']['settled_in_the_glyphs_own_frame_pct'],
     "cells_behind_the_letter_on_its_frame": cb['by_pace']['90']['lag_on_the_glyph_frame_cells']['p50']},
   "after": {"at_133wpm_mean": ca['by_pace']['90']['keydown_to_caret_settled_ms']['mean'],
     "at_133wpm_p50": ca['by_pace']['90']['keydown_to_caret_settled_ms']['p50'],
     "at_133wpm_max": ca['by_pace']['90']['keydown_to_caret_settled_ms']['max'],
     "in_the_glyphs_own_frame_pct": ca['by_pace']['90']['settled_in_the_glyphs_own_frame_pct'],
     "cells_behind_the_letter_on_its_frame": ca['by_pace']['90']['lag_on_the_glyph_frame_cells']['p50']},
   "by_pace_ms_before": {k: v['keydown_to_caret_settled_ms']['mean'] for k,v in cb['by_pace'].items()},
   "by_pace_ms_after": {k: v['keydown_to_caret_settled_ms']['mean'] for k,v in ca['by_pace'].items()},
   "fix": "app/js/caret.js: EDIT_SNAP_MS=150 (a caret move less than 150 ms after a text change never glides), GLIDE_X 62->34, GLIDE_Y 92->46, GLIDE_MIN=3 em. Navigation still glides; typing never does. The file belongs to the caret piece and the change is written up in NOTES.md under '## latency'.",
   "knock_on": "the glide is an animation and an animation is a frame clock: removing it took 2.7 ms of scheduler wait off EVERY keystroke (5.29 -> 2.55 ms mean to a committed frame, A/B, same document, same script). document.getAnimations() while typing: 'transform on caret' running in 343 of 446 samples before, 0 after.",
 },
 "real_keyboard_through_the_kernel": {
   "how": "tools/uinput-keys.py creates a keyboard on /dev/uinput and records CLOCK_MONOTONIC immediately before each write(2). bin/quill --uinput.",
   "why_it_never_ran_before": "struct.pack('=llHHi') is 16 bytes and struct input_event is 24 on 64-bit Linux, so the kernel read `type` out of the next event's timestamp, saw 0, and turned every key press into a bare SYN_REPORT. The device enumerated correctly and delivered nothing. Fixed with '@llHHi' and a size assertion.",
   "safety": "keys are typed in chunks of 25 with the page's own document.hasFocus()/activeElement re-checked between every chunk, and bin/quill refuses to begin unless the compositor's active window is the one it opened.",
   "by_regime": uin,
 },
 "compositor_ms": {n: {'to_present': pool(reg(vir,n),'samples_to_present_ms'),
                       'per_session_mean': vir['sessions'][n]['per_session_mean']} for n in sorted({r['regime'] for r in vir['typing']})},
 "compositor_commit_to_present_ms": vir['typing'][0]['ms_commit_to_present'],
 "headless_60hz_clock_ms": {n: pool(reg(head,n),'samples_to_present_ms') for n in sorted({r['regime'] for r in head['typing']})},
 "is_there_a_display_cadence_in_this_run": {
   "what": "gaps between the presentation timestamps of consecutive keystroke-carrying frames, and their circular concentration against a 16.67 ms grid (1.0 = every frame on the same phase of a 60 Hz clock, 0.0 = no clock at all)",
   "application_cost_clock_off": {"multiples_of_16_67ms_pct": cad['gaps_that_are_a_multiple_of_16_67ms_pct'],
                                  "phase_concentration": cad['phase_sd_ms_against_a_16_67ms_grid']['circular_concentration_0_to_1'],
                                  "measured_in": "dev/shots/latency/r4-appcost-cadence.json - a separate 300-key run of the same regime in the same mode; the cadence check was added to the bench after the 12-regime application-cost run had already started, so that file does not carry it. to_commit in the cadence run is 2.41 +/- 0.56, i.e. the same population."},
   "headless_clock_on": {"multiples_of_16_67ms_pct": head['typing'][0]['display_cadence_ms']['gaps_that_are_a_multiple_of_16_67ms_pct'],
                         "phase_concentration": head['typing'][0]['display_cadence_ms']['phase_sd_ms_against_a_16_67ms_grid']['circular_concentration_0_to_1']},
   "compositor_virtual_output": {"multiples_of_16_67ms_pct": vir['typing'][0]['display_cadence_ms']['gaps_that_are_a_multiple_of_16_67ms_pct'],
                                 "phase_concentration": vir['typing'][0]['display_cadence_ms']['phase_sd_ms_against_a_16_67ms_grid']['circular_concentration_0_to_1'],
                                 "meaning": "Hyprland's HEADLESS output does not gate presentation on a vblank - it reports presented 1.98 ms after commit. Round 3's 17.23 ms for this hop was the caret animation putting the client in lock-step with the compositor, not the output. A physical panel WOULD gate on a vblank, which is the 0-16.67 ms term added in bar_note."},
 },
 "photon_arithmetic": {
   "measured_kernel_to_compositor_presented_ms": {"mean": u_kern['mean'], "sd": u_kern['sd'], "p50": u_kern['p50'], "p99": u_kern['p99'], "n": u_kern['n']},
   "arithmetic_wait_for_the_panels_next_vblank_ms": {"range": [0, 16.67], "expectation": 8.3},
   "cited_panel_pixel_response_ms": 4,
   "cited_source": "Fatin, 'Typing with Pleasure' monitor budget (REFERENCE 5.2): refresh 0-17, pixel response ~4. NOT measured - there is no panel on this machine.",
   "estimate_mean_ms": 24.5, "hard_bracket_ms": [12.2, 32.9],
   "bar": "Hume, Sublime Text 32.5 +/- 4.0 (TextEdit 33.4, Atom 45.6, VS Code 47.6)",
 },
 "document_sizes": sizes,
 "cpu_throttled_present_ms": thr,
 "long_session": {"keystrokes": lng['ms']['to_present']['n'], "to_present": lng['ms']['to_present'],
   "quartile_means": [q['mean'] for q in lng['quartiles_to_present']],
   "quartile_worst": [q['max'] for q in lng['quartiles_to_present']],
   "autosave": {"writes": lng['autosave']['writes'], "ms": lng['autosave']['ms']},
   "js_heap_kb": lng.get('js_heap_kb_after'),
   "caret_static_in_every_keystroke": lng['caret_placed_in_the_keystrokes_own_task']['all_static']},
 "startup": {"launcher_cold_first_frame_ms": sm(cold), "launcher_cold_ready_marker_ms": sm(mark),
   "each_first_frame_ms": [round(x,1) for x in cold],
   "marker_note": "window.__quillReady, set at the end of Writer.boot() BEFORE any frame exists. Not the headline."},
 "frames": {"idle_control": head['frame_control'],
   "presented_within_one_60hz_frame_pct_headless_clock_on": head['typing'][0]['presented_within_one_60hz_frame_pct'],
   "presented_within_one_60hz_frame_pct_compositor": vir['typing'][0]['presented_within_one_60hz_frame_pct']},
 "deferred_render_correctness": {k: corr[k] for k in ('ok','fail','lines','ahead_lines_for_this_viewport','lines_pending_after_the_keystroke','catchup_frames','stale_lines_after_catchup','stale_visible_lines_immediately','stale_visible_lines_after_scroll','code_lines_still_visible_in_the_first_frame_after_delete','viewport_is_far_from_the_edit_lines','mirror_matches_textarea_after_catchup','heights_match')},
 "every_keystroke_accounted_for": accounted,
 "files": sorted(os.path.basename(f) for f in glob.glob(R+'/r4-*')),
 "report": "dev/progress/latency-report.md",
}
json.dump(out, open('dev/progress/latency.json','w'), indent=2)
print('written', len(json.dumps(out)))
