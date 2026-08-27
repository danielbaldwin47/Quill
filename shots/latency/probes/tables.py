#!/usr/bin/env python3
"""Turn the round-2 result files into the markdown tables used in progress/latency-report.md.
Every number in the report comes out of here, so nothing is transcribed by hand."""
import json, os, sys

D = 'shots/latency/'
def L(n):
    p = D + n
    return json.load(open(p)) if os.path.exists(p) else None

def f(x, n=2):
    return '—' if x is None else f'{x:.{n}f}'.rstrip('0').rstrip('.') if isinstance(x, float) else str(x)

def reg(d, name):
    for t in d.get('typing', []):
        if t['regime'] == name: return t
    return None

HUMAN = ['prose_end_of_draft','prose_middle_of_draft','prose_focus_sentence','paragraph_breaks',
         'revision','markdown_syntax','fence_flip','paste_blocks','letters_only_r1','bursts_and_pauses','fast_typist']
NICE = {
 'prose_end_of_draft':'writing at the end of the draft',
 'prose_middle_of_draft':'writing in the middle of the draft',
 'prose_focus_sentence':'middle, **Focus: Sentence**',
 'paragraph_breaks':'paragraph churn (Enter every 5–10 keys)',
 'revision':'revision (select back, replace, undo)',
 'markdown_syntax':'Markdown syntax (headings, emphasis, a fenced block)',
 'fence_flip':'opening and closing a fenced block, 50×',
 'paste_blocks':'writing with a paste every 25 keys',
 'letters_only_r1':'round 1\'s lowercase letters, for comparison',
 'bursts_and_pauses':'bursts of 25 keys with 1.4 s pauses',
 'fast_typist':'fast typist, 45 ms (~266 wpm)',
 'saturation_stress':'saturation: keys injected back to back',
}

def sessions_table(d, title):
    s = d.get('sessions') or {}
    print(f'\n**{title}**\n')
    print('| regime | p50 | p90 | p99 | p99 95 % CI | worst | per-session p99 | ≤1 frame | n |')
    print('|---|---|---|---|---|---|---|---|---|')
    for name in HUMAN:
        v = s.get(name)
        if not v: continue
        t = reg(d, name)
        p = v['pooled']; ci = v['pooled_p99_ci95'] or {}
        print(f"| {NICE.get(name,name)} | **{f(p['p50'])}** | {f(p['p90'])} | **{f(p['p99'])}** | "
              f"{f(ci.get('lo95'))}–{f(ci.get('hi95'))} | {f(p['max'])} | "
              f"{', '.join(f(x) for x in v['per_session_p99'])} | {f(t['presented_within_one_60hz_frame_pct'],1)} % | {p['n']} |")

def keytypes(d, name):
    t = reg(d, name)
    if not t: return
    print(f"\n`{name}` — {NICE.get(name,name)}\n")
    print('| key | n | p50 | p90 | p99 | worst |')
    print('|---|---|---|---|---|---|')
    for k, v in sorted(t['by_key_type_to_present'].items(), key=lambda kv: -kv[1]['n']):
        if k in ('modifier','nav'): continue
        print(f"| {k} | {v['n']} | {f(v['p50'])} | {f(v['p90'])} | {f(v['p99'])} | {f(v['max'])} |")

def appcost(d, label):
    print(f'\n**{label}**\n')
    print('| regime | present p50 | p90 | p99 | worst | commit p50 | main thread / key | Quill\'s own handler |')
    print('|---|---|---|---|---|---|---|---|')
    for name in HUMAN:
        t = reg(d, name)
        if not t: continue
        m = t['ms']['to_present']; c = t['ms']['to_commit']; mt = t['main_thread']
        print(f"| {NICE.get(name,name)} | **{f(m['p50'])}** | {f(m['p90'])} | **{f(m['p99'])}** | {f(m['max'])} | "
              f"{f(c['p50'])} | {f(mt['busy_ms_per_key'])} ms | {mt['quill_input_handler_us_per_key']} µs |")

def sizes():
    print('\n| document | words | lines | app cost p50 | p99 | worst | main thread / key | Quill handler | with the 60 Hz clock, p50 / p99 |')
    print('|---|---|---|---|---|---|---|---|---|')
    for s in ('2k','10k','26k','52k'):
        rm = L(f'r2-sizerm-{s}.json'); cl = L(f'r2-size-{s}.json')
        if not rm: continue
        t = reg(rm, 'prose_end_of_draft'); c = reg(cl, 'prose_end_of_draft') if cl else None
        e = rm['env']['document']; mt = t['main_thread']; m = t['ms']['to_present']
        print(f"| `doc{s}.md` | {e['words']:,} | {e['lines']:,} | **{f(m['p50'])}** | {f(m['p99'])} | {f(m['max'])} | "
              f"{f(mt['busy_ms_per_key'])} ms | {mt['quill_input_handler_us_per_key']} µs | "
              f"{f(c['ms']['to_present']['p50']) if c else '—'} / {f(c['ms']['to_present']['p99']) if c else '—'} |")
    print('\n**Enter and a fence-opening backtick, by document size** (application cost, reduced motion):\n')
    print('| document | Enter p50 | Enter p99 | fence keystroke p50 | fence p99 | fence worst |')
    print('|---|---|---|---|---|---|')
    for s in ('2k','10k','26k','52k'):
        rm = L(f'r2-sizerm-{s}.json')
        if not rm: continue
        pb = reg(rm, 'paragraph_breaks'); ff = reg(rm, 'fence_flip')
        e = (pb or {}).get('by_key_type_to_present', {}).get('enter')
        g = (ff or {}).get('by_key_type_to_present', {}).get('markdown')
        print(f"| `doc{s}.md` | {f(e['p50']) if e else '—'} | {f(e['p99']) if e else '—'} | "
              f"{f(g['p50']) if g else '—'} | {f(g['p99']) if g else '—'} | {f(g['max']) if g else '—'} |")

def throttle():
    print('\n| CPU | regime | present p50 | p99 | worst | main thread / key | ≤1 frame | ≤2 frames |')
    print('|---|---|---|---|---|---|---|---|')
    for tag, lab in (('r2-headless.json','full speed'), ('r2-throttle2.json','2× slower'), ('r2-throttle4.json','4× slower')):
        d = L(tag)
        if not d: continue
        for name in ('prose_end_of_draft','prose_middle_of_draft','paragraph_breaks'):
            t = reg(d, name)
            if not t: continue
            m = t['ms']['to_present']; mt = t['main_thread']
            print(f"| {lab} | {NICE[name]} | **{f(m['p50'])}** | **{f(m['p99'])}** | {f(m['max'])} | {f(mt['busy_ms_per_key'])} ms | "
                  f"{f(t['presented_within_one_60hz_frame_pct'],1)} % | {f(t['presented_within_two_60hz_frames_pct'],1)} % |")

def startup():
    d = L('r2-headless.json')
    if d and d.get('startup'):
        print('\n**Page load inside an already-running browser** (warm process, warm caches — this is *not* a cold start):\n')
        print('| | 10,062-word document | empty document |')
        print('|---|---|---|')
        a = d['startup']['with_10k_doc']; b = d['startup']['empty_document']
        for lab, k in (('navigation → editor ready','nav_to_editor_ready'),
                       ('navigation → DOM parsed','nav_to_dom_content_loaded'),
                       ('navigation → **first frame with the document**','nav_to_first_contentful_paint')):
            print(f"| {lab} | {f(a[k]['p50'])} ms (sd {f(a[k]['sd'])}) | {f(b[k]['p50'])} ms (sd {f(b[k]['sd'])}) |")
        print(f"\nMedians of {a['runs']} loads each.")
    print('\n**Cold start — a new browser process every run:**\n')
    print('| | exec → first frame with the document | exec → editor ready | exec → navigation start | runs |')
    print('|---|---|---|---|---|')
    for tag, lab in (('r2-cold-warmprofile.json','warm profile holding the document, cold process'),
                     ('r2-cold-fresh.json','**fresh profile** — no profile, no code cache, no storage')):
        d = L(tag)
        if not d or not d.get('cold_start'): continue
        c = d['cold_start']
        A = c['exec_to_first_contentful_paint_ms']; B = c['exec_to_editor_ready_ms']; C = c['exec_to_navigation_start_ms']
        print(f"| {lab} | **{f(A['p50'],0)} ms** (p99 {f(A['p99'],0)}, worst {f(A['max'],0)}) | {f(B['p50'],0)} ms | {f(C['p50'],0)} ms | {c['runs']} |")

def longrun():
    d = L('r2-long.json')
    if not d or not d.get('long_session'): return
    t = d['long_session']
    m = t['ms']['to_present']
    print(f"\n{t['steps_pressed']} keystrokes, {f(t['wall_ms']/60000,1)} minutes, pauses every 60 keys, "
          f"{t['keydowns_expected']} keydowns all accounted for ({t['every_keystroke_accounted_for']}).\n")
    print('| quarter of the session | p50 | p90 | p99 | worst |')
    print('|---|---|---|---|---|')
    for i, q in enumerate(t.get('quartiles_to_present', [])):
        print(f"| {['first','second','third','fourth'][i]} | {f(q['p50'])} | {f(q['p90'])} | {f(q['p99'])} | {f(q['max'])} |")
    a = t['autosave']
    print(f"\nWhole session: p50 {f(m['p50'])}, p99 {f(m['p99'])}, worst {f(m['max'])} ms. "
          f"JS heap at the end {t['js_heap_kb_after']} KB. GC {t['main_thread']['gc_us_per_key']} µs per keystroke. "
          f"`localStorage` writes: **{a['writes']}**, {a['bytes_written']:,} bytes in total, "
          f"p50 {f((a['ms'] or {}).get('p50'))} ms, worst {f((a['ms'] or {}).get('max'))} ms, "
          f"{f(a['total_ms'])} ms over the whole session.")

def frames():
    print('\n| condition | keystrokes/s | frames/s | dropped frames/s | dropped per keystroke |')
    print('|---|---|---|---|---|')
    d = L('r2-headless.json')
    for fc in (d.get('frame_control') or []):
        print(f"| {fc['what']} | 0 | {f(fc['frames_per_second'],1)} | {f(fc['dropped_frames_per_second'],2)} | — |")
    for name in ('prose_end_of_draft','fast_typist','saturation_stress'):
        t = reg(d, name)
        if not t: continue
        kps = 1000 * t['keydowns_expected'] / t['wall_ms']
        print(f"| typing: {NICE[name]} | {f(kps,1)} | {f(t['frames_per_second'],1)} | {f(t['dropped_frames_per_second'],1)} | "
              f"{f(t['dropped_frames_per_second']/kps,2)} |")

def headed():
    d = L('r2-headed-3.json')
    if not d: return
    s = d.get('sessions') or {}
    print('\n| regime | p50 | p90 | p99 | p99 95 % CI | worst | per-session p50 | per-session p99 | n |')
    print('|---|---|---|---|---|---|---|---|---|')
    for name in HUMAN:
        v = s.get(name)
        if not v: continue
        p = v['pooled']; c = v['pooled_p99_ci95'] or {}
        print(f"| {NICE.get(name,name)} | **{f(p['p50'])}** | {f(p['p90'])} | **{f(p['p99'])}** | {f(c.get('lo95'))}–{f(c.get('hi95'))} | "
              f"{f(p['max'])} | {', '.join(f(x) for x in v['per_session_p50'])} | {', '.join(f(x) for x in v['per_session_p99'])} | {p['n']} |")
    t = reg(d, 'prose_end_of_draft')
    c2p = t.get('ms_commit_to_present')
    print(f"\nChromium finished committing the frame **{f(t['ms']['to_commit']['p50'])} ms** after the key event "
          f"(p99 {f(t['ms']['to_commit']['p99'])}); the compositor then reported it presented "
          f"**{f(c2p['p50'])} ms** later at the median, {f(c2p['p99'])} ms at p99 (min {f(c2p['min'])}).")

def coldheaded():
    import glob
    for pat, lab in ((D+'r2-coldheaded-[0-9].json','the 10,062-word document in the profile'),
                     (D+'r2-coldheaded-fresh-*.json','**fresh profile**, empty document — a true first run')):
        fs = sorted(glob.glob(pat))
        if not fs: continue
        runs = [json.load(open(x))['cold_start'] for x in fs]
        fcp = sorted(r['exec_to_first_contentful_paint_ms'] for r in runs)
        rdy = sorted(r['exec_to_editor_ready_ms'] for r in runs)
        spn = sorted(r['exec_to_navigation_start_ms'] for r in runs)
        med = lambda a: a[len(a)//2]
        print(f"| {lab} | **{med(fcp):.0f} ms** ({fcp[0]:.0f}–{fcp[-1]:.0f}) | {med(rdy):.0f} ms | {med(spn):.0f} ms | {len(fs)} |")

what = sys.argv[1] if len(sys.argv) > 1 else 'all'
if what in ('all','sessions'):
    d = L('r2-headless.json')
    if d: sessions_table(d, 'Headless, 60 Hz frame clock, three sessions of 300 keystrokes each')
if what in ('all','appcost'):
    d = L('r2-appcost.json')
    if d: appcost(d, 'Application cost: no animation anywhere (reduced motion), frames produced on demand')
if what in ('all','keys'):
    d = L('r2-headless.json')
    for r in ('paragraph_breaks','fence_flip','revision','paste_blocks','prose_end_of_draft'): keytypes(d, r)
if what in ('all','sizes'): sizes()
if what in ('all','throttle'): throttle()
if what in ('all','startup'): startup()
if what in ('all','long'): longrun()
if what in ('all','frames'): frames()
if what in ('all','headed'): headed()
if what in ('all','coldheaded'): coldheaded()
