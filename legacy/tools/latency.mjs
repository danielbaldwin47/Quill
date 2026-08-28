// Quill latency bench — keystroke-to-paint, startup and cold start, measured, with every
// keystroke accounted for.  Owner: latency piece.
//
//   node tools/latency.mjs                       one headless session -> stdout / --json
//   node tools/latency.mjs --sessions 3          repeat the whole regime set, report run-to-run spread
//   node tools/latency.mjs --throttle 4          same, with the CPU slowed 4x (Emulation.setCPUThrottlingRate)
//   node tools/latency.mjs --long 3000           one long sustained session (memory, GC, autosave flushes)
//   node tools/latency.mjs --coldstart 10        spawn a browser PROCESS per run: exec -> pixels
//   node tools/latency.mjs --coldstart 8 --fresh    ... with a wiped profile: a true first run
//   node tools/latency.mjs --attach 9333 --t0 <epoch_ms>   measure a window started by bin/quill
//   node tools/latency.mjs --plan <regime>       print what that regime types, and exit
//
// What is measured, per keystroke (nothing averaged over frames, nothing dropped):
//   input_delay      hardware event timestamp -> first JS handler        (queueing)
//   js               time inside our listeners (keydown/keypress/input/keyup)
//   to_commit        event timestamp -> the frame carrying it finished commit
//   to_present       event timestamp -> that frame was presented          <- the headline
// The first three come from Chrome's own EventTiming trace records (unrounded, µs resolution,
// category devtools.timeline); to_present is EventTiming's `duration`, which ends at the
// presentation feedback of the frame that contained the update. An independent in-page probe
// (keydown -> requestAnimationFrame -> MessageChannel task) is recorded alongside as a cross-check.
//
// Round 2: the typist is no longer "lowercase letters and spaces". Every regime types a scripted
// stream of real prose with capitals, punctuation, Enter, Backspace, undo, paste,
// select-and-replace and Markdown syntax, and every keystroke is labelled by kind, so the
// expensive paths (a paragraph break renumbers the lines below it; a backtick can flip fenced
// context for the rest of the document) are reported separately instead of hiding in an average.
import { chromium } from 'playwright-core';
import fs from 'node:fs'; import os from 'node:os'; import path from 'node:path'; import crypto from 'node:crypto';
import { execSync, spawn } from 'node:child_process';

// ---------- args ----------
const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (!a.startsWith('--')) continue;
  const k = a.slice(2), v = process.argv[i + 1];
  if (v === undefined || v.startsWith('--')) args[k] = true; else { args[k] = v; i++; }
}
const URL_ = args.url || 'http://localhost:4173/';
const DOC = args.doc || args.text || 'shots/latency/doc10k.md';
const KEYS = +(args.keys || 300);
const RUNS = +(args.runs || 12);
const PACE = args.pace === undefined ? 90 : +args.pace;      // ms between keystrokes (90 ms ~= 133 wpm)
const SESSIONS = +(args.sessions || 1);
const THROTTLE = +(args.throttle || 1);
const VIEW = { width: +(args.w || 1440), height: +(args.h || 900) };
const doc = DOC === 'none' ? '' : fs.readFileSync(DOC, 'utf8');
const words = (t) => (t.match(/[\p{L}\p{N}'’]+/gu) || []).length;
const CHROME = process.env.QUILL_CHROME || '/usr/bin/chromium';
// The app itself animates while you type (the caret glides 62 ms to its new column, the chrome
// bars fade), and an animation is a frame clock: it makes every keystroke wait for the next tick
// exactly as a display does. `--reduced-motion` runs the page under prefers-reduced-motion:
// reduce, which is a real user setting and which caret.css and chrome.css both honour, so the
// page produces frames on demand and the application's own cost is visible.
const MOTION = args['reduced-motion'] ? 'reduce' : 'no-preference';
const CTXOPTS = { viewport: VIEW, colorScheme: 'light', reducedMotion: MOTION };

// ---------- stats ----------
const r2 = (x) => x == null || Number.isNaN(x) ? null : Math.round(x * 100) / 100;
function pct(sorted, p) {                    // nearest-rank
  if (!sorted.length) return null;
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))];
}
function stat(arr) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x)).sort((x, y) => x - y);
  if (!a.length) return null;
  const mean = a.reduce((s, x) => s + x, 0) / a.length;
  const sd = Math.sqrt(a.reduce((s, x) => s + (x - mean) ** 2, 0) / a.length);
  return { n: a.length, min: r2(a[0]), p50: r2(pct(a, .5)), p90: r2(pct(a, .9)), p95: r2(pct(a, .95)),
           p99: r2(pct(a, .99)), max: r2(a[a.length - 1]), mean: r2(mean), sd: r2(sd) };
}
// A p99 of 300 samples is the 3rd-worst sample; without an interval it is a rumour. Percentile
// bootstrap, 2000 resamples, fixed seed so the interval is reproducible.
function ci(arr, p, resamples = 2000) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x));
  if (a.length < 20) return null;
  const rnd = mulberry32(0x51AC1);
  const out = [];
  for (let r = 0; r < resamples; r++) {
    const s = new Array(a.length);
    for (let i = 0; i < a.length; i++) s[i] = a[(rnd() * a.length) | 0];
    s.sort((x, y) => x - y);
    out.push(pct(s, p));
  }
  out.sort((x, y) => x - y);
  return { lo95: r2(pct(out, .025)), hi95: r2(pct(out, .975)) };
}
// The same bootstrap for the mean, because the bar this piece is judged against is a mean.
function ciMean(arr, resamples = 2000) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x));
  if (a.length < 20) return null;
  const rnd = mulberry32(0x3EA11);
  const out = [];
  for (let r = 0; r < resamples; r++) { let s = 0; for (let i = 0; i < a.length; i++) s += a[(rnd() * a.length) | 0]; out.push(s / a.length); }
  out.sort((x, y) => x - y);
  return { lo95: r2(pct(out, .025)), hi95: r2(pct(out, .975)) };
}
function mulberry32(a) { return function () { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
function hash32(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }

// ---------- environment ----------
function displayInfo() {
  try {
    const m = JSON.parse(execSync('hyprctl monitors -j', { stdio: ['ignore', 'pipe', 'ignore'] }).toString());
    return m.map((f) => ({ name: f.name, model: f.description, mode: `${f.width}x${f.height}`, refresh_hz: r2(f.refreshRate), scale: f.scale, vrr: f.vrr, focused: f.focused }));
  } catch (e) { return null; }
}
// Exactly which build of the app these numbers belong to.
function appFingerprint() {
  try {
    const files = [];
    const walk = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const f = path.join(d, e.name); if (e.isDirectory()) { if (e.name !== 'fonts') walk(f); } else if (/\.(js|css|html)$/.test(e.name)) files.push(f); } };
    walk(args.snapshot ? path.join(args.snapshot, 'app') : 'legacy/app');
    files.sort();
    const h = crypto.createHash('sha256');
    for (const f of files) h.update(f + ':' + crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex') + '\n');
    let git = null;
    try { git = execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim(); } catch (e) {}
    return { files: files.length, sha256: h.digest('hex').slice(0, 16), git_head: git, served_from: args.snapshot || 'legacy/app/' };
  } catch (e) { return null; }
}
// This is somebody's workstation. Say what else was running while the numbers were taken.
function otherLoad() {
  try {
    return execSync('ps -eo pcpu,comm --sort=-pcpu | head -6', { stdio: ['ignore', 'pipe', 'ignore'] })
      .toString().trim().split('\n').slice(1).map((l) => l.trim().replace(/\s+/, ' '));
  } catch (e) { return null; }
}
function env(browser, headless) {
  const c = os.cpus();
  return {
    at: new Date().toISOString(),
    chromium: browser ? browser.version() : null,
    headless,
    cpu_throttle_x: THROTTLE,
    reduced_motion: MOTION,
    node: process.version,
    os: `${os.type()} ${os.release()}`,
    cpu: c[0] ? `${c[0].model} (${c.length} threads)` : null,
    load1: r2(os.loadavg()[0]),
    busiest_processes: otherLoad(),
    mem_gb: Math.round(os.totalmem() / 2 ** 30),
    display: headless ? null : displayInfo(),
    viewport: `${VIEW.width}x${VIEW.height}`,
    app: appFingerprint(),
    document: { path: DOC, words: words(doc), chars: doc.length, lines: doc.split('\n').length },
  };
}

// ---------- the typist ----------
// Real writing, not one repeated sentence: capitals, commas, quotes, apostrophes, dashes,
// sentence ends, paragraph breaks, and the corrections everybody makes while drafting.
const PROSE = `The letter arrived on a Tuesday, unsigned, folded twice, and pushed under the door before anyone was awake. Marguerite read it standing up, still holding the kettle. "Not today," she said, to nobody in particular; the room, which had heard worse, said nothing back. She counted the reasons to go — there were four, and two of them were the same reason wearing a different coat — and then she put the kettle down and went anyway.
`;
const MARKDOWN = `## A note on measurement

The *fastest* editor is the one that **never** makes you wait for a character, and the only way to know is to count them. Read the frame back at presentation:

\`\`\`js
const t0 = event.timeStamp;
requestAnimationFrame(() => report(performance.now() - t0));
\`\`\`

> A number without a keystroke count is a number about nothing.

`;
const LETTERS = 'the quick brown fox jumps over the lazy dog while alice considers the pleasure of making a daisy chain ';
const PASTE_TEXT = 'There is no such thing as a small delay in a text editor: the eye notices a tenth of a frame, and the hand notices before the eye does.\n\n';

const PUNCT = `.,;:'"!?-()`;
function charStep(ch, mdMode) {
  if (ch === '\n') return { press: 'Enter', label: 'enter', keydowns: 1 };
  if (ch === ' ') return { press: 'Space', label: 'space', keydowns: 1 };
  if (/[a-z]/.test(ch)) return { press: ch, label: 'letter', keydowns: 1 };
  if (/[A-Z]/.test(ch)) return { press: ch, label: 'capital', keydowns: 1 };
  if (/[*_#`>\[\]]/.test(ch)) return { press: ch, label: 'markdown', keydowns: 1 };
  if (ch === '—') return { press: '-', label: 'punct', keydowns: 1 };          // no em-dash key on a US layout
  if (PUNCT.includes(ch)) return { press: ch, label: 'punct', keydowns: 1 };
  return { press: ch, label: mdMode ? 'markdown' : 'punct', keydowns: 1 };
}
const BACKSPACE = { press: 'Backspace', label: 'backspace', keydowns: 1 };
const UNDO = { press: 'Control+z', label: 'undo', keydowns: 2, labels: ['modifier', 'undo'] };
const PASTE = { press: 'Control+v', label: 'paste', keydowns: 2, labels: ['modifier', 'paste'] };
const SELLEFT = { press: 'Shift+ArrowLeft', label: 'nav', keydowns: 2, labels: ['modifier', 'nav'] };

// Each generator returns exactly `n` steps (a step is one key press; a chord is one step with two
// keydowns). Seeded per regime name, so every session types an identical stream and the spread
// between sessions is the machine, not the script.
function script(mix, n, seed) {
  const rnd = mulberry32(seed);
  const out = [];
  const push = (s) => { if (out.length < n) out.push(s); };
  if (mix === 'letters') {
    for (let i = 0; out.length < n; i++) push(charStep(LETTERS[i % LETTERS.length]));
  } else if (mix === 'prose' || mix === 'paste') {
    let i = 0, since = 0, sincePaste = 0;
    while (out.length < n) {
      push(charStep(PROSE[i++ % PROSE.length]));
      since++; sincePaste++;
      if (mix === 'paste' && sincePaste > 24) { push(PASTE); sincePaste = 0; since = 0; continue; }
      if (since > 34 + Math.floor(rnd() * 26)) {                 // a typo, noticed and fixed
        const k = 1 + Math.floor(rnd() * 3);
        for (let j = 0; j < k; j++) push(BACKSPACE);
        since = 0;
      }
    }
  } else if (mix === 'newlines') {
    // Paragraph churn: short lines and Enter, in the middle of the document. Every Enter changes
    // the line count, which is the branch render() treats differently from typing inside a line.
    let i = 0;
    while (out.length < n) {
      const len = 4 + Math.floor(rnd() * 6);
      for (let j = 0; j < len; j++) push(charStep(PROSE[i++ % PROSE.length]));
      push(charStep('\n'));
    }
  } else if (mix === 'revision') {
    // Write a word, select it back, replace it; undo now and then. Selection-heavy editing.
    let i = 0;
    while (out.length < n) {
      const len = 4 + Math.floor(rnd() * 5);
      for (let j = 0; j < len; j++) { const c = PROSE[i++ % PROSE.length]; push(charStep(/[a-zA-Z]/.test(c) ? c.toLowerCase() : 'e')); }
      for (let j = 0; j < len; j++) push(SELLEFT);
      for (let j = 0; j < len; j++) push(charStep('abcdefgh'[j % 8]));
      push(charStep(' '));
      if (rnd() < 0.25) push(UNDO);
    }
  } else if (mix === 'fences') {
    // Open a fenced code block in the middle of the document and close it again, over and over.
    // The third backtick changes the context of every line below it to the end of the document;
    // the first backspace changes them all back. It is the most expensive thing a single
    // keystroke can ask a Markdown editor to do, and it is one key.
    while (out.length < n) {
      for (let j = 0; j < 3; j++) push(charStep('`'));
      for (let j = 0; j < 3; j++) push(BACKSPACE);
    }
  } else if (mix === 'markdown') {
    let i = 0;
    while (out.length < n) push(charStep(MARKDOWN[i++ % MARKDOWN.length], true));
  } else throw new Error('unknown mix ' + mix);
  return out.slice(0, n);
}
function labelsOf(steps) {                    // one label per keydown, in order
  const out = [];
  for (const s of steps) { if (s.labels) out.push(...s.labels); else out.push(s.label); }
  return out;
}
const TEXT_LABELS = new Set(['letter', 'capital', 'space', 'punct', 'markdown', 'enter', 'backspace', 'undo', 'paste']);
// A real keyboard has no "A" key: it has Shift and "a", and Chromium sees TWO keydowns for one
// character. CDP's Input.dispatchKeyEvent does not — it sets the modifier on the one event. So a
// uinput run has its own keydown accounting, and this is the same US-layout shift table that
// tools/uinput-keys.py types from (kb_layout = us on this machine).
const SHIFTED_CHARS = new Set('!@#$%^&*()_+{}:"~|<>?'.split(''));
const needsShift = (ch) => /[A-Z]/.test(ch) || SHIFTED_CHARS.has(ch);
const pressChar = (st) => (st.press === 'Space' ? ' ' : st.press === 'Enter' ? '\n' : st.press === 'Backspace' ? '\b' : st.press);

// ---------- page helpers ----------
const seedDoc = (t) => {                     // put the document in place before the app boots
  try {
    localStorage.removeItem('quill.lib');
    localStorage.setItem('quill.doc', t);
    localStorage.setItem('quill.doc.sel', String(t.length));
  } catch (e) {}
};
// Time every localStorage write the app makes. files.js flushes the whole document 400 ms after
// the last change, so a bench that never pauses never sees it.
const storageProbe = () => {
  try {
    const proto = Storage.prototype, orig = proto.setItem;
    window.__store = window.__store || [];
    if (proto.__quillTimed) return;          // a reused page registers this script once per regime
    proto.__quillTimed = true;
    proto.setItem = function (k, v) {
      const t = performance.now();
      const r = orig.apply(this, arguments);
      window.__store.push({ k, bytes: String(v).length, ms: performance.now() - t });
      return r;
    };
  } catch (e) {}
};
async function readStartup(p) {
  return p.evaluate(() => new Promise((res) => {
    const collect = () => {
      const nav = performance.getEntriesByType('navigation')[0] || {};
      const paint = performance.getEntriesByType('paint');
      const fcp = paint.find((e) => e.name === 'first-contentful-paint');
      const lcp = (window.__lcp || 0) || null;
      res({
        dcl: nav.domContentLoadedEventEnd || null,
        load: nav.loadEventEnd || null,
        ready: window.__quillReady || null,              // Writer.boot() finished: document rendered, caret placed, focus set
        fcp: fcp ? fcp.startTime : null,                 // first frame with ink, taken at frame presentation
        lcp,
        chars: (window.Writer && Writer.getText().length) || 0,
        lines: (window.Writer && Writer.lineCount()) || 0,
      });
    };
    if (performance.getEntriesByType('paint').length) requestAnimationFrame(() => setTimeout(collect, 60));
    else new PerformanceObserver(() => requestAnimationFrame(() => setTimeout(collect, 60))).observe({ type: 'paint', buffered: true });
  }));
}
const lcpProbe = () => {
  try { new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lcp = e.startTime; }).observe({ type: 'largest-contentful-paint', buffered: true }); } catch (e) {}
};

// ---------- trace ----------
const TRACE_CATS = ['devtools.timeline', 'blink,devtools.timeline', 'latency', 'benchmark',
                    'cc,benchmark,disabled-by-default-devtools.timeline.frame', 'disabled-by-default-devtools.timeline.frame'];
async function startTrace(cdp) {
  const events = [];
  const on = (e) => { for (const x of e.value) events.push(x); };
  cdp.on('Tracing.dataCollected', on);
  const complete = new Promise((r) => cdp.once('Tracing.tracingComplete', r));
  await cdp.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: TRACE_CATS } });
  return { events, stop: async () => { await cdp.send('Tracing.end'); await complete; cdp.off('Tracing.dataCollected', on); return events; } };
}

// A real display ticks whether or not anything is animating, and a keystroke waits for the next
// tick. A headless Chromium only ticks when something asks it to: with nothing animating it
// produces a frame on demand, which makes every latency look ~6 ms better than any 60 Hz panel can
// ever be. This 1 px composited animation keeps the clock running, so headless models the panel.
// `--clock off` turns it off, which measures the application's own cost with no display cadence
// at all — the right mode for A/B-ing code changes, the wrong mode for quoting user-facing latency.
const CLOCK_CSS = `@keyframes __quill_clock { from { opacity: .999 } to { opacity: 1 } }
#__quill_clock { position: fixed; left: 0; bottom: 0; width: 1px; height: 1px; opacity: .999;
  background: transparent; pointer-events: none; will-change: opacity;
  animation: __quill_clock 1s linear infinite; }`;
async function installClock(page) {
  await page.evaluate((css) => {
    if (document.getElementById('__quill_clock')) return;
    const st = document.createElement('style'); st.textContent = css; document.head.appendChild(st);
    const el = document.createElement('div'); el.id = '__quill_clock'; document.body.appendChild(el);
  }, CLOCK_CSS);
}

// ---------- trace reduction ----------
function reduceTrace(events) {
  const et = events.filter((e) => e.name === 'EventTiming' && e.ph === 'b' && e.args && e.args.data)
    .map((e) => ({ ...e.args.data, ts: e.ts }))   // ts = Chromium TimeTicks, microseconds; on Linux
    .sort((a, b) => a.timeStamp - b.timeStamp);   // that is CLOCK_MONOTONIC (verified, see report 3.4)
  const kd = et.filter((e) => e.type === 'keydown');
  // Main-thread paint groups, so a keystroke can be timed to the end of the frame update it
  // caused and not only to the compositor's commit. One group = the Paint/Layerize runs of one
  // BeginMainFrame (they are contiguous; a gap of more than 1 ms starts a new frame).
  const paintRuns = events.filter((e) => e.ph === 'X' && e.dur >= 0 && (e.name === 'Paint' || e.name === 'Layerize'))
    .map((e) => ({ a: e.ts, b: e.ts + e.dur })).sort((x, y) => x.a - y.a);
  const groups = [];
  for (const r of paintRuns) {
    const last = groups[groups.length - 1];
    if (last && r.a <= last.b + 1000) { if (r.b > last.b) last.b = r.b; } else groups.push({ a: r.a, b: r.b });
  }
  const groupAfter = (us) => { for (const g of groups) if (g.b >= us) return g; return null; };
  // Top-level main-thread tasks, so a frame's own work can be told apart from the wait for it.
  // Every rendering step of one frame (rAF callbacks, style, layout, pre-paint, paint, layerize)
  // runs inside ONE top-level task; its start is when Chromium actually began that frame on the
  // main thread. Everything before it is scheduling, which no web application can shorten.
  const topAll = events.filter((e) => e.ph === 'X' && e.dur > 0 && e.cat && e.cat.includes('devtools.timeline'))
    .sort((a, b) => a.ts - b.ts || b.dur - a.dur);
  const tops = []; let tend = -1;
  for (const e of topAll) { if (e.ts >= tend) { tops.push({ a: e.ts, b: e.ts + e.dur }); tend = e.ts + e.dur; } }
  const taskAround = (us) => { let lo = 0, hi = tops.length - 1, best = null;
    while (lo <= hi) { const m = (lo + hi) >> 1; if (tops[m].a <= us) { best = tops[m]; lo = m + 1; } else hi = m - 1; }
    return best && best.b >= us ? best : null; };
  // family = every input event between this keydown and the next one (keypress, input, keyup),
  // found with a two-pointer walk rather than a filter per key (O(n), not O(n^2)).
  const per = []; let j = 0;
  for (let i = 0; i < kd.length; i++) {
    const e = kd[i], next = kd[i + 1] ? kd[i + 1].timeStamp : Infinity;
    while (j < et.length && et[j].timeStamp < e.timeStamp) j++;
    let js = 0, fam = 0, k = j;
    let jsDoneUs = e.ts + (e.processingEnd - e.timeStamp) * 1000;
    while (k < et.length && et[k].timeStamp < next) {
      js += Math.max(0, et[k].processingEnd - et[k].processingStart);
      const doneUs = et[k].ts + (et[k].processingEnd - et[k].timeStamp) * 1000;
      if (et[k].type !== 'keyup' && doneUs > jsDoneUs) jsDoneUs = doneUs;
      fam++; k++;
    }
    const g = groupAfter(jsDoneUs);
    const task = g ? taskAround(g.a) : null;
    const fs = task ? task.a : (g ? g.a : null);
    per.push({
      t: e.timeStamp,
      ts_us: e.ts,                            // Chromium TimeTicks = CLOCK_MONOTONIC on Linux
      input_delay: e.processingStart - e.timeStamp,
      js,
      // The application's own JavaScript is finished (last non-keyup handler for this key returns).
      to_js_done: (jsDoneUs - e.ts) / 1000,
      // ... then, before the frame's first paint op: the wait for Chromium's next BeginFrame
      // PLUS that frame's style, layout and pre-paint (paint groups start at the first Paint or
      // Layerize event, and style/layout/pre-paint run before it). The scheduling part of this is
      // not the app's: the app cannot start a frame, it can only ask for one.
      frame_wait: g ? Math.max(0, (g.a - jsDoneUs) / 1000) : null,
      // ... and the frame's own style/layout/paint ends here.
      to_paint: g ? (g.b - e.ts) / 1000 : null,
      to_commit: e.commitFinishTime ? e.commitFinishTime - e.timeStamp : null,
      to_present: e.duration || null,
      keys_in_family: fam,
      // Split `frame_wait` into the two things it is made of. `frame_start` is the beginning of the
      // top-level task that ran this frame's rendering steps.
      sched_wait: fs ? Math.max(0, (fs - jsDoneUs) / 1000) : null,           // nothing is running: waiting to be scheduled
      frame_work: fs && e.commitFinishTime ? (e.ts + e.commitFinishTime * 1000 - e.timeStamp * 1000 - fs) / 1000 : null,
      // The application's own cost with the frame CADENCE removed and nothing else removed:
      // its JavaScript, plus the whole of the frame it caused (style, layout, pre-paint, paint,
      // commit). This is the quantity Fatin's Typometer reports; see the report, section 6.
      app_cost: fs && e.commitFinishTime ? (jsDoneUs - e.ts) / 1000 + (e.ts + (e.commitFinishTime - e.timeStamp) * 1000 - fs) / 1000 : null,
      frame_start_us: fs,
      present_us: e.duration ? e.ts + e.duration * 1000 : null,
    });
  }
  // main-thread busy time, top-level events only (no double counting)
  const mainTid = (() => {
    const c = {};
    for (const e of events) if (e.ph === 'X' && e.cat && e.cat.includes('devtools.timeline')) c[e.pid + ':' + e.tid] = (c[e.pid + ':' + e.tid] || 0) + e.dur;
    return Object.entries(c).sort((a, b) => b[1] - a[1])[0]?.[0];
  })();
  const xs = events.filter((e) => e.ph === 'X' && e.dur > 0 && (e.pid + ':' + e.tid) === mainTid).sort((a, b) => a.ts - b.ts || b.dur - a.dur);
  let busy = 0, end = -1;
  const byName = {};
  for (const e of xs) {
    if (e.ts >= end) { busy += e.dur; end = e.ts + e.dur; }            // top level only
    const k = e.name + (e.args && e.args.data && e.args.data.type ? ':' + e.args.data.type : '');
    byName[k] = (byName[k] || 0) + e.dur;
  }
  const states = {};
  for (const r of events) {
    if (r.name !== 'PipelineReporter' || r.ph !== 'b' || !r.args || !r.args.frame_reporter) continue;
    const f = r.args.frame_reporter;
    const k = f.state + (f.affects_smoothness ? '_affecting_smoothness' : '');
    states[k] = (states[k] || 0) + 1;
  }
  const gc = ['MajorGC', 'MinorGC', 'V8.GCScavenger', 'V8.GCFinalizeMC', 'BlinkGC.AtomicPhase']
    .reduce((s, n) => s + (byName[n] || 0), 0);
  return { per, busy, byName, states, gc, keydowns: kd.length };
}

// ---------- one typing regime ----------
// A page for one regime, with nothing left over from the last one. Headless gets a new page;
// an attached browser gets its own window RELOADED — a second window from that process might not
// inherit the compositor's placement rule and could land on the user's screen, and a reload gives
// a fresh JS context anyway, which is what makes the probe unrepeatable.
async function regimePage(ctx, reuse) {
  const page = reuse || await ctx.newPage();
  if (!page.__quillInit) {
    page.__quillInit = true;
    await page.addInitScript(storageProbe);
    if (doc) await page.addInitScript(seedDoc, doc);
    // Settings for the profile this run uses, merged into localStorage before the app boots.
    // (Used to ask, for instance, what a keystroke costs with the chrome bars turned off.)
    if (args.settings && args.settings !== true)
      await page.addInitScript((j) => { try { const cur = JSON.parse(localStorage.getItem('quill.settings') || '{}'); localStorage.setItem('quill.settings', JSON.stringify({ ...cur, ...JSON.parse(j) })); } catch (e) {} }, String(args.settings));
  }
  await page.goto(URL_, { waitUntil: 'load' });
  return page;
}

async function typingRun(ctx, opts) {
  const { keys, pace, where, focus, mix } = opts;
  // A fresh page per regime. The round-1 bench reused one page and installed its probe twice,
  // which produced 204 keydown records for 200 presses in the one real-display session it had.
  // A page that has never been measured cannot be double-probed, and every regime then starts
  // from the same document instead of from the previous regime's leftovers.
  const page = await regimePage(ctx, opts.reuse);
  await page.evaluate(() => document.fonts.ready);
  const cdp = await ctx.newCDPSession(page);
  if (THROTTLE > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: THROTTLE });
  if (opts.clock !== 'off') await installClock(page);

  await page.evaluate(async ([where, focus]) => {
    if (focus && Writer.settings.focus !== focus) Writer.setSetting('focus', focus);
    const i = Writer.el.input;
    const text = i.value;
    // Where the writer actually is: at the end of the draft, or in the middle of it.
    let caret = text.length;
    if (where === 'middle') { const half = Math.floor(text.length / 2); caret = text.indexOf('\n', half); if (caret < 0) caret = half; }
    i.focus(); i.setSelectionRange(caret, caret);
    Writer.emitSelection ? Writer.emitSelection(true) : Writer.emit('selection');
    // Scroll the caret into view — the line being edited must really be on screen, or the
    // browser never repaints it and the measurement flatters us.
    const r = Writer.caretRect(), sc = Writer.el.scroller;
    if (r) sc.scrollTop += (r.top - sc.getBoundingClientRect().top) - sc.clientHeight * 0.55;
    await new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(res)));
  }, [where, focus]);

  // in-page probe: every keydown is recorded with the key that caused it; the frame that carries
  // it resolves the whole batch. Installed exactly once, into a page that has never had one.
  await page.evaluate(() => {
    if (window.__latInstalled) throw new Error('probe already installed — this page has been measured before');
    window.__latInstalled = true;
    window.__lat = { keys: [], rafs: 0, batches: 0 };
    const K = window.__lat.keys;
    let batch = [], scheduled = false;
    const ch = new MessageChannel();
    // A message posted from inside the rAF callback runs as a task after that frame's rendering
    // steps, i.e. once the frame has been committed. Indices travel, not objects: postMessage
    // structured-clones its payload, so the records themselves have to be looked up here.
    ch.port1.onmessage = (ev) => {
      const t = performance.now();
      for (const i of ev.data.idx) { const rec = K[i]; rec.commit = t - rec.t; rec.raf = ev.data.raf - rec.t; }
      window.__lat.batches++;
    };
    window.addEventListener('keydown', (e) => {
      const rec = { seq: K.length, key: e.key, shift: e.shiftKey, ctrl: e.ctrlKey, t: e.timeStamp, commit: null, raf: null };
      K.push(rec); batch.push(rec.seq);
      if (scheduled) return;
      scheduled = true;
      requestAnimationFrame((ts) => {
        scheduled = false; window.__lat.rafs++;
        const idx = batch; batch = [];
        ch.port2.postMessage({ idx, raf: ts });
      });
    }, { capture: true });
    // The caret is the one moving thing a typist's eye is on, and until round 4 it GLIDED to its
    // new column over 62 ms — four frames behind the letter. This listener runs after the app's
    // own (it is registered later), reads three strings off the caret's inline style, and never
    // touches layout, so it costs nothing measurable: it proves, per keystroke, that the caret's
    // final position is written in the keystroke's own task and that nothing is animating it.
    // The black-box confirmation (its client rect, frame by frame) is shots/latency/probes/caret-settle.mjs.
    window.__caret = { n: 0, static_and_placed: 0, moved: 0, animating: 0 };
    (function () {
      const c = document.querySelector('#caret-layer .caret');
      if (!c) return;
      let last = '';
      // Bubble phase on window: this runs AFTER the textarea's own listener, i.e. after the app
      // has placed the caret for this keystroke. (Capture phase would read the previous one.)
      window.addEventListener('input', () => {
        const st = window.__caret; st.n++;
        const tr = c.style.transform || 'none', dur = c.style.transitionDuration || '0s';
        const at = c.style.left + '|' + c.style.top;
        if (at !== last) { st.moved++; last = at; }
        if (tr !== 'none' || (dur !== '0s' && dur !== '')) st.animating++;
        else st.static_and_placed++;
      }, false);
    })();
    // The browser's own view of the same thing (8 ms granularity), as a sanity check on the trace.
    window.__evt = [];
    try {
      new PerformanceObserver((l) => { for (const e of l.getEntries()) if (e.name === 'keydown') window.__evt.push(e.duration); })
        .observe({ type: 'event', durationThreshold: 0, buffered: true });
    } catch (e) {}
  });

  if (mix === 'paste') {
    await ctx.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(URL_).origin }).catch(() => {});
    await page.evaluate((t) => navigator.clipboard.writeText(t), PASTE_TEXT).catch(() => {});
  }
  const inEditor = await page.evaluate(() => ({ chars: Writer.getText().length, lines: Writer.lineCount() }));

  // Warm-up, outside the measurement: the first keystrokes into a freshly loaded page pay for
  // lazy compilation and first touch of the editing machinery, and no writer types only 300 keys.
  const warm = script('letters', opts.warmup === undefined ? 25 : opts.warmup, 1);
  for (const s of warm) { await page.keyboard.press(s.press); if (pace) await page.waitForTimeout(pace); }
  await page.waitForTimeout(300);
  await page.evaluate(() => { window.__lat.keys.length = 0; window.__lat.rafs = 0; window.__evt.length = 0; window.__store.length = 0; });

  const steps = script(mix, keys, hash32(opts.name));
  const labels = opts.uinput
    ? steps.flatMap((st) => (needsShift(pressChar(st)) ? ['modifier', st.label] : [st.label]))
    : labelsOf(steps);
  const trace = await startTrace(cdp);
  const t0 = Date.now();
  let pressed = 0, expected = 0, injected = null;
  if (opts.uinput) {
    // Real keys go to whatever the compositor thinks is focused. If that is not this window they
    // go into somebody's editor, so this refuses to inject unless the page itself says it has
    // keyboard focus and the caret is in the textarea.
    const checkFocus = () => page.evaluate(() => ({ has: document.hasFocus(), active: document.activeElement && document.activeElement.id, vis: document.visibilityState }));
    const focused = await checkFocus();
    if (!focused.has || focused.active !== 'input' || focused.vis !== 'visible')
      throw new Error('uinput: refusing to type — window focus is ' + JSON.stringify(focused));
    // Real keys, through /dev/uinput -> libinput -> Hyprland -> Wayland -> Chromium, with
    // CLOCK_MONOTONIC recorded immediately before each write(2). Nothing here goes through CDP.
    const text = steps.map(pressChar).join('');
    if (!/^[\x08\x0a\x20-\x7e]+$/.test(text)) throw new Error('uinput: regime ' + opts.name + ' presses a key this injector cannot express');
    // Typed in chunks, with the page's own view of keyboard focus re-checked between every one of
    // them: real keys go wherever the compositor thinks focus is, and this machine has terminals
    // on it. One 300-key write could not be stopped half way; this can, and does.
    const CHUNK = 25;
    injected = await (async () => {
      const ch = spawn('python3', [path.join(path.dirname(new URL(import.meta.url).pathname), '..', '..', 'tools', 'uinput-keys.py')], { stdio: ['pipe', 'pipe', 'inherit'] });
      let buf = '', lines = [], waiter = null;
      ch.stdout.on('data', (d) => {
        buf += d;
        let i;
        while ((i = buf.indexOf('\n')) >= 0) { lines.push(buf.slice(0, i)); buf = buf.slice(i + 1); }
        if (waiter) { const w = waiter; waiter = null; w(); }
      });
      const closed = new Promise((res) => ch.on('close', res));
      const nextLine = async () => {
        while (!lines.length) { await new Promise((res) => { waiter = res; setTimeout(res, 50); }); if (ch.exitCode != null && !lines.length) return null; }
        return lines.shift();
      };
      ch.stdin.write(JSON.stringify({ text, pace_ms: pace || 8, hold_ms: 12, settle_ms: 1500, chunk: CHUNK }) + '\n');
      const ready = JSON.parse(await nextLine());
      let done = 0, stoppedBecause = null;
      while (done < ready.keys) {
        const f = await checkFocus();
        if (!f.has || f.active !== 'input' || f.vis !== 'visible') { stoppedBecause = f; break; }
        ch.stdin.write('go\n');
        const l = await nextLine();
        if (!l) break;
        done += JSON.parse(l).typed;
      }
      ch.stdin.write('end\n'); ch.stdin.end();
      await closed;
      const tail = buf + lines.join('');
      let res;
      try { res = JSON.parse(tail.slice(tail.lastIndexOf('{"ok"'))); } catch (e) { throw new Error('uinput injector failed: ' + tail.slice(0, 200)); }
      res.chunk_size = CHUNK;
      res.focus_rechecks = Math.ceil(ready.keys / CHUNK);
      res.stopped_because_focus_was_lost = stoppedBecause;
      return res;
    })();
    if (injected.n !== injected.requested)
      throw new Error('uinput: stopped after ' + injected.n + ' of ' + injected.requested + ' keys (focus check: ' + JSON.stringify(injected.stopped_because_focus_was_lost) + ')');
    pressed = steps.length;
    expected = steps.reduce((a, st) => a + (needsShift(pressChar(st)) ? 2 : 1), 0);
  } else {
  for (let i = 0; i < steps.length; i++) {
    await page.keyboard.press(steps[i].press);
    pressed++; expected += steps[i].keydowns;
    if (opts.pauseEvery && (i + 1) % opts.pauseEvery === 0) await page.waitForTimeout(opts.pauseMs || 1200);
    else if (pace) await page.waitForTimeout(pace);
  }
  }
  const wall = Date.now() - t0;
  await page.waitForTimeout(700);                       // let the last frames present, and any autosave land
  const events = await trace.stop();
  const inpage = await page.evaluate(() => ({ keys: window.__lat.keys, rafs: window.__lat.rafs, evt: window.__evt, store: window.__store, caret: window.__caret,
    mem: performance.memory ? Math.round(performance.memory.usedJSHeapSize / 1024) : null,
    chars: Writer.getText().length, lines: Writer.lineCount() }));

  const { per, busy, byName, states, gc, keydowns } = reduceTrace(events);
  const ip = inpage.keys;

  // Label each trace keydown with the key that produced it. Trace and probe are both in time
  // order and both must hold exactly `expected` records; if they do not, the run says so rather
  // than quietly averaging over a hole.
  const aligned = keydowns === expected && ip.length === expected;
  const labelFor = (i) => (aligned ? labels[i] : 'unaligned');
  const byLabel = {};
  for (let i = 0; i < per.length; i++) (byLabel[labelFor(i)] ||= []).push(per[i].to_present);
  const textIdx = per.map((_, i) => i).filter((i) => TEXT_LABELS.has(labelFor(i)));
  const headline = textIdx.map((i) => per[i].to_present);
  const headlineCommit = textIdx.map((i) => per[i].to_commit);
  const headlinePaint = textIdx.map((i) => per[i].to_paint);
  const headlineJs = textIdx.map((i) => per[i].to_js_done);
  const headlineWait = textIdx.map((i) => per[i].frame_wait);
  const headlineSched = textIdx.map((i) => per[i].sched_wait);
  const headlineFrameWork = textIdx.map((i) => per[i].frame_work);
  const headlineAppCost = textIdx.map((i) => per[i].app_cost);
  // A "p99" of four samples is the maximum wearing a hat. Anything under 20 samples gets its n,
  // its mean and its worst and nothing else.
  const thin = (v) => { const st = stat(v); if (!st) return null; if (st.n >= 20) return st;
    return { n: st.n, mean: st.mean, max: st.max, too_few_for_percentiles: true }; };
  const keyTypes = {};
  for (const [k, v] of Object.entries(byLabel)) keyTypes[k] = thin(v);
  const probeKeys = {};
  for (const k of ip) probeKeys[k.key] = (probeKeys[k.key] || 0) + 1;

  // The hop CDP cannot see: kernel write -> the timestamp Chromium gives the event.
  let delivery = null, fromKernel = null, deliveryAligned = false;
  if (injected && injected.ok) {
    const ev = injected.events;
    // One injected write == one character; the Shift half of a shifted character is written in the
    // same packet at the same timestamp, and Chromium reports it as a keydown of its own. So the
    // i-th injected write is the i-th NON-modifier keydown in the trace.
    const charIdx = aligned ? per.map((_, i) => i).filter((i) => labels[i] !== 'modifier') : [];
    if (aligned && charIdx.length === ev.length) {
      deliveryAligned = true;
      delivery = new Array(per.length).fill(null);
      for (let n = 0; n < charIdx.length; n++) delivery[charIdx[n]] = (per[charIdx[n]].ts_us - ev[n].t_ns / 1000) / 1000;
      // the Shift keydown of a shifted character was written at the same instant as the character
      for (let i = 1; i < per.length; i++) if (delivery[i] == null && delivery[i + 1] != null) delivery[i] = (per[i].ts_us - ev[charIdx.indexOf(i + 1)].t_ns / 1000) / 1000;
      fromKernel = textIdx.map((i) => (per[i].to_present == null || delivery[i] == null ? null : per[i].to_present + delivery[i]));
    }
  }
  const out = {
    regime: opts.name,
    mix, where, focus: focus || 'off', pace_ms: pace, pause_every_keys: opts.pauseEvery || null, wall_ms: wall,
    cpu_throttle_x: THROTTLE,
    reduced_motion: MOTION,
    display_clock: opts.clock === 'off' ? 'off (frames produced on demand — application cost only, faster than any real display)' : '60 Hz (a 1 px composited animation keeps the frame clock running, as a real panel does)',
    document_in_editor: inEditor,
    document_after: { chars: inpage.chars, lines: inpage.lines },
    warmup_keys: opts.warmup === undefined ? 25 : opts.warmup,
    steps_pressed: pressed,
    keydowns_expected: expected,
    keys_seen_in_trace: keydowns,
    keys_seen_by_page: ip.length,
    every_keystroke_accounted_for: aligned && ip.every((k) => k.commit != null) && per.every((k) => k.to_present != null),
    keys_unresolved_in_page_probe: ip.filter((k) => k.commit == null).length,
    keys_missing_commit_time: per.filter((k) => k.to_commit == null).length,
    keys_missing_present_time: per.filter((k) => k.to_present == null).length,
    keys_sharing_a_frame: ip.length - inpage.rafs,
    shifted_keydowns: ip.filter((k) => k.shift).length,
    keys_pressed_by_name: probeKeys,
    keystroke_kinds: Object.fromEntries(Object.entries(byLabel).map(([k, v]) => [k, v.length])),
    frames: states,
    frames_per_second: r2(1000 * Object.values(states).reduce((s, x) => s + x, 0) / Math.max(1, wall)),
    dropped_frames_per_second: r2(1000 * ((states.STATE_DROPPED || 0) + (states.STATE_DROPPED_affecting_smoothness || 0)) / Math.max(1, wall)),
    presented_within_one_60hz_frame_pct: r2(100 * headline.filter((x) => x != null && x <= 16.67).length / Math.max(1, headline.length)),
    presented_within_two_60hz_frames_pct: r2(100 * headline.filter((x) => x != null && x <= 33.34).length / Math.max(1, headline.length)),
    // REFERENCE 5.3 states the app-internal bar as "keystroke -> committed frame: <= 5 ms AVERAGE,
    // <= 16 ms WORST CASE" — Fatin's Typometer figures are means, not medians. So the bar is
    // answered in the mean and the max of `to_commit`, in that vocabulary, pass or fail, here.
    bar_app_internal: (() => {
      const c = stat(headlineCommit); if (!c) return null;
      return {
        definition: 'keystroke hardware timestamp -> commitFinishTime of the frame carrying it (Chrome EventTiming), text-affecting keystrokes only',
        bar: '<= 5 ms average, <= 16 ms worst case (REFERENCE 5.3; Fatin Typometer means: Notepad++ 4.3, Emacs 5.3, Sublime 8.2)',
        mean_ms: c.mean, sd_ms: c.sd, max_ms: c.max, n: c.n,
        clears_mean_bar: c.mean <= 5, clears_worst_bar: c.max <= 16,
        also_p50_p99: [c.p50, c.p99],
      };
    })(),
    // Is the wait for a frame a DISPLAY CADENCE or is it slack? Gaps between the starts of
    // consecutive frames, over the whole run: a grid at ~16.7 ms means Chromium is running a
    // 60 Hz BeginFrame source and every keystroke is waiting for a tick of it, whatever this
    // bench's own clock animation is doing.
    frame_cadence_ms: (() => {
      const fs = [...new Set(per.map((k) => k.frame_start_us).filter((x) => x != null))].sort((a, b) => a - b);
      const gaps = []; for (let i = 1; i < fs.length; i++) { const d = (fs[i] - fs[i - 1]) / 1000; if (d < 200) gaps.push(d); }
      const near = (x, m) => Math.abs(x / m - Math.round(x / m)) * m < 2 && x > 8;
      return { frames_that_carried_a_keystroke: fs.length, gaps: stat(gaps),
        gaps_that_are_a_multiple_of_16_67ms_pct: r2(100 * gaps.filter((x) => near(x, 16.667)).length / Math.max(1, gaps.length)) };
    })(),
    // Does the thing presenting these frames really tick at 60 Hz? Gaps between the presentation
    // timestamps of consecutive frames that carried a keystroke: on a display (physical or virtual)
    // they are whole multiples of the refresh interval, because a frame can only be scanned out at
    // a vblank. If they are not, the "display" in this run is not a display.
    display_cadence_ms: (() => {
      const ps = [...new Set(per.map((k) => k.present_us).filter((x) => x != null))].sort((a, b) => a - b);
      const gaps = []; for (let i = 1; i < ps.length; i++) { const d = (ps[i] - ps[i - 1]) / 1000; if (d < 200) gaps.push(d); }
      const near = (x, m) => Math.abs(x / m - Math.round(x / m)) * m < 2 && x > 8;
      return { frames_presented_carrying_a_keystroke: ps.length, gaps: stat(gaps),
        gaps_that_are_a_multiple_of_16_67ms_pct: r2(100 * gaps.filter((x) => near(x, 16.667)).length / Math.max(1, gaps.length)),
        phase_sd_ms_against_a_16_67ms_grid: (() => {                 // 0 = a perfect vblank grid
          if (ps.length < 8) return null;
          const ph = ps.map((x) => ((x / 1000) % 16.667) / 16.667 * 2 * Math.PI);
          const C = ph.reduce((a, t) => a + Math.cos(t), 0) / ph.length, S = ph.reduce((a, t) => a + Math.sin(t), 0) / ph.length;
          const R = Math.hypot(C, S);                                 // 1 = every frame on the same phase
          return { circular_concentration_0_to_1: r2(R), n: ps.length };
        })() };
    })(),
    // The caret, per keystroke, from inside the page: was its new position written in the
    // keystroke's own task, with nothing animating it? (round 4; see report section 5)
    caret_placed_in_the_keystrokes_own_task: inpage.caret
      ? { ...inpage.caret, all_static: inpage.caret.n > 0 && inpage.caret.animating === 0,
          note: 'n = input events seen; `moved` = the caret\'s inline left/top changed in that same task; `animating` = a transform or a transition duration was still set on it' }
      : null,
    input_injection: opts.uinput
      ? { how: 'real keys through /dev/uinput -> libinput -> Hyprland -> Wayland -> Chromium (tools/uinput-keys.py)',
          device: injected && injected.device,
          aligned_with_trace: deliveryAligned,
          chunk_size: injected && injected.chunk_size,
          focus_rechecks_during_the_run: injected && injected.focus_rechecks,
          keys_written: injected && injected.n,
          kernel_write_to_chromium_event_ms: stat(delivery || []),
          kernel_write_to_presented_ms: stat(fromKernel || []),
          samples_kernel_to_presented_ms: (fromKernel || []).map((x) => r2(x)) }
      : { how: 'CDP Input.dispatchKeyEvent — the clock starts inside the browser process, so the kernel/libinput/compositor delivery hop is NOT included' },
    ms: {
      to_present: stat(headline),                       // headline: text-affecting keystrokes only
      to_present_ci95: { p50: ci(headline, .5), p99: ci(headline, .99) },
      to_commit: stat(headlineCommit),
      to_commit_ci95: { mean: ciMean(headlineCommit), p99: ci(headlineCommit, .99) },
      // The ladder inside one keystroke: app JS done -> wait for a BeginFrame -> frame painted.
      to_js_done: stat(headlineJs),
      frame_wait: stat(headlineWait),
      // frame_wait, split: time in which NOTHING was running (waiting to be scheduled) and the
      // frame's own work once it started.
      scheduler_wait: stat(headlineSched),
      frame_work: stat(headlineFrameWork),
      app_cost_no_cadence: stat(headlineAppCost),
      app_cost_no_cadence_mean_ci95: ciMean(headlineAppCost.filter((x) => x != null)),
      to_paint: stat(headlinePaint),
      to_present_every_keydown: stat(per.map((k) => k.to_present)),
      input_delay: stat(per.map((k) => k.input_delay)),
      js_per_key: stat(per.map((k) => k.js)),
      inpage_keydown_to_frame_task: stat(ip.map((k) => k.commit)),
      inpage_keydown_to_raf: stat(ip.map((k) => k.raf)),
      browser_event_timing_ge16ms_only: stat(inpage.evt),   // the JS API rounds to 8 ms and hides anything under 16 ms
    },
    by_key_type_to_present: keyTypes,
    autosave: {
      note: 'files.js flushes the whole document to localStorage 400 ms after the last change; a run that never pauses never triggers it.',
      writes: inpage.store.length,
      bytes_written: inpage.store.reduce((s, x) => s + x.bytes, 0),
      ms: stat(inpage.store.map((x) => x.ms)),
      total_ms: r2(inpage.store.reduce((s, x) => s + x.ms, 0)),
    },
    js_heap_kb_after: inpage.mem,
    main_thread: {
      busy_ms_per_key: r2(busy / 1000 / Math.max(1, keydowns)),
      busy_percent_of_wall: r2(100 * busy / 1000 / Math.max(1, wall)),
      gc_us_per_key: Math.round(gc / Math.max(1, keydowns)),
      // Where the per-keystroke time goes. The first line is Chrome's own editing of the
      // <textarea> (inserting one character into a 53 KB value), which no web editor can avoid;
      // the second is everything Quill runs in response.
      chrome_text_insertion_us_per_key: Math.round((byName['EventDispatch:textInput'] || 0) / Math.max(1, keydowns)),
      quill_input_handler_us_per_key: Math.round((byName['EventDispatch:input'] || 0) / Math.max(1, keydowns)),
      style_layout_paint_us_per_key: Math.round((['UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint'].reduce((s2, n) => s2 + (byName[n] || 0), 0)) / Math.max(1, keydowns)),
      top: Object.entries(byName).sort((a, b) => b[1] - a[1]).slice(0, 12).map(([k, v]) => ({ what: k, us_per_key: Math.round(v / Math.max(1, keydowns)) })),
    },
    samples_to_present_ms: headline.map((x) => r2(x)),    // raw, so anyone can recompute the percentiles
    samples_to_commit_ms: headlineCommit.map((x) => r2(x)),
    samples_to_paint_ms: headlinePaint.map((x) => r2(x)),
    samples_kernel_to_presented_ms_headline: (fromKernel || []).map((x) => r2(x)),
    samples_to_js_done_ms: headlineJs.map((x) => r2(x)),
    samples_app_cost_no_cadence_ms: headlineAppCost.map((x) => r2(x)),
    samples_scheduler_wait_ms: headlineSched.map((x) => r2(x)),
    // How long the frame waits between Chromium finishing its commit and the compositor reporting
    // it presented: on a panel this is the wait for the next scan-out, and it is the one hop a
    // headless run cannot have.
    ms_commit_to_present: stat(textIdx.map((i) => (per[i].to_present != null && per[i].to_commit != null) ? per[i].to_present - per[i].to_commit : null)),
    samples_commit_to_present_ms: textIdx.map((i) => ((per[i].to_present != null && per[i].to_commit != null) ? r2(per[i].to_present - per[i].to_commit) : null)),
  };
  if (opts.quarters) {                                    // long runs: is the end like the beginning?
    const q = Math.floor(headline.length / 4);
    out.quartiles_to_present = [0, 1, 2, 3].map((i) => stat(headline.slice(i * q, (i + 1) * q)));
    out.heap_kb_after = inpage.mem;
  }
  await cdp.detach().catch(() => {});
  if (!opts.reuse) await page.close();
  return out;
}

// ---------- frame-production control: is that "dropped frame per keystroke" real? ----------
// The clock animation asks for a frame every vsync whether or not the main thread has an update.
// Run it with no typing at all and count the same counters: if the dropped-frame rate matches the
// one measured while typing, the drops belong to the bench's own animation — demonstrated, not
// asserted. Run again with the clock off for the third corner of the table.
async function frameControl(ctx, clock, seconds = 4, reuse) {
  const page = await regimePage(ctx, reuse);
  await page.evaluate(() => document.fonts.ready);
  const cdp = await ctx.newCDPSession(page);
  if (clock !== 'off') await installClock(page);
  await page.waitForTimeout(300);
  const trace = await startTrace(cdp);
  const t0 = Date.now();
  await page.waitForTimeout(seconds * 1000);            // no keystrokes at all
  const wall = Date.now() - t0;
  const events = await trace.stop();
  const { states } = reduceTrace(events);
  await cdp.detach().catch(() => {});
  if (!reuse) await page.close();
  const dropped = (states.STATE_DROPPED || 0) + (states.STATE_DROPPED_affecting_smoothness || 0);
  return {
    what: `idle page, no keystrokes at all, display clock ${clock === 'off' ? 'off' : 'on'}`,
    seconds: r2(wall / 1000), frames: states,
    frames_per_second: r2(1000 * Object.values(states).reduce((s, x) => s + x, 0) / wall),
    dropped_frames_per_second: r2(1000 * dropped / wall),
  };
}

// ---------- regimes ----------
const ALL_REGIMES = [
  // Plain writing at the end of a draft — the most common case there is, and the one quoted.
  { name: 'prose_end_of_draft',    mix: 'prose',    where: 'end',    pace: PACE, focus: 'off' },
  { name: 'prose_middle_of_draft', mix: 'prose',    where: 'middle', pace: PACE, focus: 'off' },
  { name: 'prose_focus_sentence',  mix: 'prose',    where: 'middle', pace: PACE, focus: 'sentence' },
  { name: 'paragraph_breaks',      mix: 'newlines', where: 'middle', pace: PACE, focus: 'off' },
  { name: 'revision',              mix: 'revision', where: 'middle', pace: PACE, focus: 'off' },
  { name: 'markdown_syntax',       mix: 'markdown', where: 'middle', pace: PACE, focus: 'off' },
  { name: 'fence_flip',            mix: 'fences',   where: 'middle', pace: PACE, focus: 'off' },
  { name: 'paste_blocks',          mix: 'paste',    where: 'end',    pace: PACE, focus: 'off' },
  { name: 'letters_only_r1',       mix: 'letters',  where: 'middle', pace: PACE, focus: 'off' },
  { name: 'bursts_and_pauses',     mix: 'prose',    where: 'end',    pace: PACE, focus: 'off', pauseEvery: 25, pauseMs: 1400 },
  { name: 'fast_typist',           mix: 'prose',    where: 'middle', pace: 45,   focus: 'off' },
  { name: 'saturation_stress',     mix: 'prose',    where: 'end',    pace: 0,    focus: 'off' },
];
function regimeList() {
  if (args.regimes && args.regimes !== true) {
    const want = String(args.regimes).split(',');
    return ALL_REGIMES.filter((r) => want.includes(r.name));
  }
  if (args.quick) return [ALL_REGIMES[0]];
  if (THROTTLE > 1) return ALL_REGIMES.filter((r) => ['prose_end_of_draft', 'prose_middle_of_draft', 'paragraph_breaks'].includes(r.name));
  return ALL_REGIMES;
}

// `--plan <regime>` prints exactly what a regime would type, and exits: no browser, no server. The
// regimes and the typist above are about to move to tools/regimes.mjs, and identical output from
// this switch before and after that move is what proves the move changed nothing.
function formatPlan(r, keys) {
  const steps = script(r.mix, keys, hash32(r.name));
  const out = [];
  out.push(`regime  ${r.name}`);
  out.push(`  mix          ${r.mix}`);
  out.push(`  caret        ${r.where === 'end' ? 'end of the document' : 'middle of the document, at the first line break past half way'}`);
  out.push(`  pace         ${r.pace === 0 ? 'no wait between keystrokes (as fast as the driver types)' : r.pace + ' ms between keystrokes'}`);
  out.push(`  focus        ${r.focus || 'off'}`);
  out.push(`  pauses       ${r.pauseEvery ? `every ${r.pauseEvery} keys, ${r.pauseMs || 1200} ms` : 'none'}`);
  out.push(`  seed         ${hash32(r.name)}`);
  out.push(`  warm-up      25 letter keys, outside the measurement`);
  out.push(`  keys         ${steps.length} steps, ${steps.reduce((a, s) => a + s.keydowns, 0)} keydowns`);
  out.push('');
  for (let i = 0; i < steps.length; i++) {
    const s = steps[i];
    out.push(`${String(i).padStart(6)}  ${s.press.padEnd(16)}${s.label}${s.labels ? '  [' + s.labels.join(' ') + ']' : ''}`);
  }
  return out.join('\n');
}
if (args.plan) {
  const r = args.plan === true ? null : ALL_REGIMES.find((x) => x.name === args.plan);
  if (!r) { console.error(`--plan takes one regime: ${ALL_REGIMES.map((x) => x.name).join(', ')}`); process.exit(2); }
  console.log(formatPlan(r, KEYS));
  process.exit(0);
}

// ---------- a session ----------
async function typingSession(browser, ctx0, reuse) {
  const ctx = ctx0 || await browser.newContext(CTXOPTS);
  const out = [];
  for (const r of regimeList()) out.push(await typingRun(ctx, { keys: KEYS, clock: args.clock || 'on', uinput: !!args.uinput, reuse, ...r }));
  if (!ctx0) await ctx.close();
  return out;
}
// Across sessions: pool the samples for a percentile with an interval, and show the spread of the
// per-session p99 too, because that is exactly what one run cannot tell you.
function acrossSessions(sessions) {
  const byRegime = {};
  for (const s of sessions) for (const r of s) (byRegime[r.regime] ||= []).push(r);
  const out = {};
  for (const [name, runs] of Object.entries(byRegime)) {
    const pooled = runs.flatMap((r) => r.samples_to_present_ms).filter((x) => x != null);
    const pooledCommit = runs.flatMap((r) => r.samples_to_commit_ms || []).filter((x) => x != null);
    const pooledPaint = runs.flatMap((r) => r.samples_to_paint_ms || []).filter((x) => x != null);
    const pooledJs = runs.flatMap((r) => r.samples_to_js_done_ms || []).filter((x) => x != null);
    const pooledApp = runs.flatMap((r) => r.samples_app_cost_no_cadence_ms || []).filter((x) => x != null);
    const pooledSched = runs.flatMap((r) => r.samples_scheduler_wait_ms || []).filter((x) => x != null);
    out[name] = {
      sessions: runs.length,
      keys_per_session: runs[0].ms.to_present ? runs[0].ms.to_present.n : null,
      every_keystroke_accounted_for: runs.every((r) => r.every_keystroke_accounted_for),
      per_session_mean: runs.map((r) => r.ms.to_present && r.ms.to_present.mean),
      per_session_p50: runs.map((r) => r.ms.to_present && r.ms.to_present.p50),
      per_session_p99: runs.map((r) => r.ms.to_present && r.ms.to_present.p99),
      per_session_max: runs.map((r) => r.ms.to_present && r.ms.to_present.max),
      pooled: stat(pooled),
      pooled_mean_ci95: ciMean(pooled),
      pooled_p50_ci95: ci(pooled, .5),
      pooled_p99_ci95: ci(pooled, .99),
      pooled_to_commit: stat(pooledCommit),
      pooled_to_commit_mean_ci95: ciMean(pooledCommit),
      pooled_to_paint: stat(pooledPaint),
      pooled_to_js_done: stat(pooledJs),
      pooled_app_cost_no_cadence: stat(pooledApp),
      pooled_app_cost_no_cadence_mean_ci95: ciMean(pooledApp),
      pooled_scheduler_wait: stat(pooledSched),
      caret_static_in_every_keystroke: runs.every((r) => r.caret_placed_in_the_keystrokes_own_task && r.caret_placed_in_the_keystrokes_own_task.all_static),
      main_thread_ms_per_key: runs.map((r) => r.main_thread.busy_ms_per_key),
    };
  }
  return out;
}

// ---------- startup inside an already-running browser ----------
async function startupRuns(browser) {
  const startup = {};
  for (const [name, seed] of [['with_10k_doc', doc], ['empty_document', '']]) {
    const runs = [];
    for (let i = 0; i < RUNS; i++) {
      const ctx = await browser.newContext(CTXOPTS);
      const p = await ctx.newPage();
      await p.addInitScript(lcpProbe);
      if (seed) await p.addInitScript(seedDoc, seed);
      await p.goto(URL_, { waitUntil: 'load' });
      runs.push(await readStartup(p));
      await ctx.close();
    }
    startup[name] = {
      what: 'navigation -> paint inside an ALREADY RUNNING browser: warm browser process, warm GPU process, warm fonts, warm V8 code cache, warm server. This is a page-load number and is NOT a cold start — for that see cold_start.',
      runs: runs.length,
      note: name === 'with_10k_doc' ? `${words(doc)} words restored from local storage and fully rendered before the first frame` : 'empty editor',
      chars_rendered: runs[0].chars, lines_rendered: runs[0].lines,
      nav_to_first_contentful_paint: stat(runs.map((r) => r.fcp)),
      nav_to_editor_ready: stat(runs.map((r) => r.ready)),
      nav_to_largest_contentful_paint: stat(runs.map((r) => r.lcp)),
      nav_to_dom_content_loaded: stat(runs.map((r) => r.dcl)),
    };
  }
  return startup;
}

// ---------- cold start: a new browser PROCESS every run ----------
const COLD_FLAGS = (profile, port, headless) => [
  `--app=${URL_}`, `--user-data-dir=${profile}`, `--remote-debugging-port=${port}`,
  '--window-size=1440,900', '--no-first-run', '--no-default-browser-check', '--disable-component-update',
  '--disable-background-networking', '--disable-sync', '--disable-features=Translate,MediaRouter',
  ...(headless ? ['--headless=new'] : ['--ozone-platform-hint=auto']),
];
async function primeProfile(profile, text) {
  // Put the document into that profile's local storage the way the app itself would, and let the
  // browser exit cleanly so leveldb is on disk before the cold run opens it.
  fs.mkdirSync(profile, { recursive: true });
  const ctx = await chromium.launchPersistentContext(profile, { executablePath: CHROME, headless: true, viewport: VIEW, args: ['--no-first-run'] });
  const p = await ctx.newPage();
  await p.addInitScript(seedDoc, text);
  await p.goto(URL_, { waitUntil: 'load' });
  await p.waitForTimeout(600);
  await ctx.close();
}
async function coldStart(n) {
  const headless = !args.headed;
  const profile = (args.profile && args.profile !== true) ? args.profile : path.join(os.tmpdir(), 'quill-cold-profile');
  const fresh = !!args.fresh;
  const runs = [];
  for (let i = 0; i < n; i++) {
    fs.rmSync(profile, { recursive: true, force: true });
    if (!fresh) await primeProfile(profile, doc);       // warm profile holding the document, cold process
    const port = 9500 + Math.floor(Math.random() * 400);
    const t0 = Date.now();
    const child = spawn(CHROME, COLD_FLAGS(profile, port, headless), { stdio: 'ignore' });
    let ok = false;
    for (let k = 0; k < 900; k++) {                      // 10 ms polling; it decides when we attach, not what is timed
      ok = await fetch(`http://127.0.0.1:${port}/json/version`).then(() => true).catch(() => false);
      if (ok) break;
      await new Promise((r) => setTimeout(r, 10));
    }
    if (!ok) { child.kill('SIGKILL'); continue; }
    const b = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    const c = b.contexts()[0];
    let page = c.pages().find((p) => p.url().startsWith('http'));
    if (!page) page = await c.waitForEvent('page');
    await page.waitForLoadState('load');
    const m = await readStartup(page);
    m.origin = await page.evaluate(() => performance.timeOrigin);
    runs.push({
      exec_to_fcp: m.fcp == null ? null : m.origin + m.fcp - t0,
      exec_to_ready: m.ready == null ? null : m.origin + m.ready - t0,
      exec_to_nav: m.origin - t0,
      nav_to_fcp: m.fcp, nav_to_ready: m.ready,
      chars: m.chars, lines: m.lines,
    });
    await b.close().catch(() => {});
    child.kill('SIGTERM');
    await new Promise((r) => setTimeout(r, 500));
  }
  return {
    what: `${fresh ? 'FRESH PROFILE — a true first run: no profile, no V8 code cache, no local storage (so the document cannot be there)' : 'warm profile holding the 10k-word document, but a cold browser PROCESS'}, ${headless ? 'headless' : 'headed, on a real compositor'}: shell exec of chromium -> the named milestone, wall clock, one new process per run`,
    runs: runs.length,
    document_in_editor: runs[0] ? { chars: runs[0].chars, lines: runs[0].lines } : null,
    exec_to_first_contentful_paint_ms: stat(runs.map((r) => r.exec_to_fcp)),
    exec_to_editor_ready_ms: stat(runs.map((r) => r.exec_to_ready)),
    exec_to_navigation_start_ms: stat(runs.map((r) => r.exec_to_nav)),
    navigation_to_first_contentful_paint_ms: stat(runs.map((r) => r.nav_to_fcp)),
    navigation_to_editor_ready_ms: stat(runs.map((r) => r.nav_to_ready)),
    each_ms: runs.map((r) => ({ fcp: r2(r.exec_to_fcp), ready: r2(r.exec_to_ready) })),
  };
}

// ---------- main ----------
let browser, result;
if (args.attach) {
  // A browser someone else started (bin/quill): a real window on a real compositor.
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${args.attach}`);
  const ctx = browser.contexts()[0];
  const page0 = ctx.pages().find((p) => p.url().startsWith('http')) || (await ctx.waitForEvent('page'));
  await page0.waitForLoadState('load');
  result = { env: env(browser, false), where: args.where || 'headed', cold_start: null, typing: [], sessions: null };
  if (args.t0) {
    const m = await readStartup(page0);
    m.origin = await page0.evaluate(() => performance.timeOrigin);
    const t0 = +args.t0;
    result.cold_start = {
      what: 'bin/quill: shell exec of the launcher -> the named milestone, wall clock. Includes the chromium process spawn, profile init, window creation, navigation and the full render of the document.',
      profile: args.profile || null,
      exec_to_first_contentful_paint_ms: m.fcp == null ? null : r2(m.origin + m.fcp - t0),
      exec_to_editor_ready_ms: m.ready == null ? null : r2(m.origin + m.ready - t0),
      exec_to_navigation_start_ms: r2(m.origin - t0),
      navigation_to_first_contentful_paint_ms: r2(m.fcp),
      navigation_to_editor_ready_ms: r2(m.ready),
      document_in_editor: await page0.evaluate(() => ({ chars: Writer.getText().length, lines: Writer.lineCount() })),
    };
  }
  if (KEYS > 0) {
    const sessions = [];
    for (let s = 0; s < SESSIONS; s++) sessions.push(await typingSession(browser, ctx, page0));
    result.typing = sessions.flat();
    result.sessions = acrossSessions(sessions);
    result.frame_control = await frameControl(ctx, args.clock || 'on', 4, page0);
  }
  if (args.seed) {                                   // leave the benchmark document in this profile
    await page0.evaluate((t) => { Writer.setText(t, { caret: t.length }); }, doc);
    await page0.waitForTimeout(900);
    await page0.reload({ waitUntil: 'load' });        // the unload flushes the document to storage
    await page0.waitForTimeout(300);
    result.seeded_chars = await page0.evaluate(() => Writer.getText().length);
    try { const bs = await browser.newBrowserCDPSession(); await bs.send('Browser.close').catch(() => {}); } catch (e) {}
  }
} else if (args.coldstart) {
  result = { env: env(null, !args.headed), cold_start: await coldStart(+args.coldstart) };
} else {
  browser = await chromium.launch({ executablePath: CHROME, headless: true });
  result = { env: env(browser, true), startup: {}, typing: [], sessions: null };
  if (args.long) {
    const ctx = await browser.newContext(CTXOPTS);
    result.long_session = await typingRun(ctx, {
      name: 'long_session', mix: 'prose', where: 'end', pace: PACE, focus: 'off',
      keys: +args.long, clock: args.clock || 'on', pauseEvery: 60, pauseMs: 900, quarters: true,
    });
    await ctx.close();
  } else if (!args.startuponly) {
    const sessions = [];
    for (let s = 0; s < SESSIONS; s++) sessions.push(await typingSession(browser));
    result.typing = sessions.flat();
    result.sessions = acrossSessions(sessions);
    const ctx = await browser.newContext(CTXOPTS);
    result.frame_control = [await frameControl(ctx, 'on'), await frameControl(ctx, 'off')];
    await ctx.close();
  }
  if (!args.nostartup && !args.long) result.startup = await startupRuns(browser);
  await browser.close();
}

const json = JSON.stringify(result, null, 2);
if (args.json) { fs.mkdirSync(path.dirname(args.json), { recursive: true }); fs.writeFileSync(args.json, json); }
if (!args.quiet) console.log(json);
if (args.attach) process.exit(0);      // the CDP connection would otherwise keep the launcher waiting
