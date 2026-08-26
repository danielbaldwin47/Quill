// Quill latency bench — keystroke-to-paint and startup, measured, with every keystroke accounted for.
// Owner: latency piece.
//
//   node tools/latency.mjs                                  # headless, full run, writes shots/latency/r1-headless.json
//   node tools/latency.mjs --runs 12 --keys 400 --pace 90
//   node tools/latency.mjs --attach 9333 --t0 <epoch_ms>    # measure a browser started by bin/quill (real display)
//   node tools/latency.mjs --progress progress/latency.json
//
// What is measured, per keystroke (nothing is averaged over frames, nothing is dropped):
//   input_delay      hardware event timestamp -> first JS handler          (queueing)
//   js               time inside our listeners (keydown/keypress/input/keyup)
//   to_commit        event timestamp -> the frame carrying it finished commit
//   to_present       event timestamp -> that frame was presented           <- the headline
// The first three come from Chrome's own EventTiming trace records (unrounded, µs resolution,
// category devtools.timeline); to_present is EventTiming's `duration`, which ends at the
// presentation feedback of the frame that contained the update. An independent in-page probe
// (keydown -> requestAnimationFrame -> MessageChannel task) is recorded alongside as a cross-check.
import { chromium } from 'playwright-core';
import fs from 'node:fs'; import os from 'node:os'; import path from 'node:path';
import { execSync } from 'node:child_process';

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
const VIEW = { width: +(args.w || 1440), height: +(args.h || 900) };
const doc = DOC === 'none' ? '' : fs.readFileSync(DOC, 'utf8');
const words = (t) => (t.match(/[\p{L}\p{N}'’]+/gu) || []).length;

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

// ---------- environment ----------
function displayInfo() {
  try {
    const m = JSON.parse(execSync('hyprctl monitors -j', { stdio: ['ignore', 'pipe', 'ignore'] }).toString());
    const f = m.find((x) => x.focused) || m[0];
    return { server: 'wayland/hyprland', model: f.description, mode: `${f.width}x${f.height}`, refresh_hz: r2(f.refreshRate), scale: f.scale, vrr: f.vrr };
  } catch (e) { return null; }
}
function env(browser, headless) {
  const c = os.cpus();
  return {
    at: new Date().toISOString(),
    chromium: browser ? browser.version() : null,
    headless,
    node: process.version,
    os: `${os.type()} ${os.release()}`,
    cpu: c[0] ? `${c[0].model} (${c.length} threads)` : null,
    load1: r2(os.loadavg()[0]),
    mem_gb: Math.round(os.totalmem() / 2 ** 30),
    display: headless ? null : displayInfo(),
    viewport: `${VIEW.width}x${VIEW.height}`,
    document: { path: DOC, words: words(doc), chars: doc.length, lines: doc.split('\n').length },
  };
}

// ---------- page helpers ----------
const seedDoc = (t) => {                     // put the document in place before the app boots
  try {
    localStorage.removeItem('quill.lib');
    localStorage.setItem('quill.doc', t);
    localStorage.setItem('quill.doc.sel', String(t.length));
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
  cdp.on('Tracing.dataCollected', (e) => { for (const x of e.value) events.push(x); });
  const complete = new Promise((r) => cdp.once('Tracing.tracingComplete', r));
  await cdp.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: TRACE_CATS } });
  return { events, stop: async () => { await cdp.send('Tracing.end'); await complete; return events; } };
}

// ---------- typing ----------
async function typingRun(page, cdp, opts) {
  const { keys, pace, where, focus } = opts;
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

  // in-page probe: every keydown is recorded; the frame that carries it resolves the whole batch
  await page.evaluate(() => {
    window.__lat = { keys: [], rafs: 0, batches: 0 };
    const K = window.__lat.keys;
    let batch = [], scheduled = false, rafTs = 0;
    const ch = new MessageChannel();
    // A message posted from inside the rAF callback runs as a task after that frame's rendering
    // steps, i.e. once the frame has been committed. Indices travel, not objects: postMessage
    // structured-clones its payload, so the records themselves have to be looked up here.
    ch.port1.onmessage = (ev) => {
      const t = performance.now();
      for (const i of ev.data.idx) { const rec = K[i]; rec.commit = t - rec.t; rec.raf = ev.data.raf - rec.t; }
      window.__lat.batches++;
    };
    Writer.el.input.addEventListener('keydown', (e) => {
      const rec = { seq: K.length, t: e.timeStamp, handler: performance.now() - e.timeStamp, commit: null, raf: null };
      K.push(rec); batch.push(rec.seq);
      if (scheduled) return;
      scheduled = true;
      requestAnimationFrame((ts) => {
        scheduled = false; window.__lat.rafs++;
        const idx = batch; batch = [];
        ch.port2.postMessage({ idx, raf: ts });
      });
    }, { capture: true });
    // The browser's own view of the same thing (8 ms granularity), as a sanity check on the trace.
    window.__evt = [];
    try {
      new PerformanceObserver((l) => { for (const e of l.getEntries()) if (e.name === 'keydown') window.__evt.push(e.duration); })
        .observe({ type: 'event', durationThreshold: 0, buffered: true });
    } catch (e) {}
  });

  const inEditor = await page.evaluate(() => ({ chars: Writer.getText().length, lines: Writer.lineCount() }));
  const trace = await startTrace(cdp);
  const t0 = Date.now();
  const sample = 'the quick brown fox jumps over the lazy dog while alice considers the pleasure of making a daisy chain ';
  let pressed = 0;
  for (let i = 0; i < keys; i++) {
    const c = sample[i % sample.length];
    await page.keyboard.press(c === ' ' ? 'Space' : c);
    pressed++;
    if (pace) await page.waitForTimeout(pace);
  }
  const wall = Date.now() - t0;
  await page.waitForTimeout(500);                       // let the last frames present
  const events = await trace.stop();
  const inpage = await page.evaluate(() => ({ keys: window.__lat.keys, rafs: window.__lat.rafs, evt: window.__evt }));

  // ---- trace: EventTiming carries Chrome's own per-event numbers, unrounded ----
  const et = events.filter((e) => e.name === 'EventTiming' && e.ph === 'b' && e.args && e.args.data).map((e) => e.args.data);
  const kd = et.filter((e) => e.type === 'keydown').sort((a, b) => a.timeStamp - b.timeStamp);
  const per = [];
  for (let i = 0; i < kd.length; i++) {
    const e = kd[i], next = kd[i + 1] ? kd[i + 1].timeStamp : Infinity;
    const family = et.filter((x) => x.timeStamp >= e.timeStamp && x.timeStamp < next);
    per.push({
      t: e.timeStamp,
      input_delay: e.processingStart - e.timeStamp,
      js: family.reduce((s, x) => s + Math.max(0, x.processingEnd - x.processingStart), 0),
      to_commit: e.commitFinishTime ? e.commitFinishTime - e.timeStamp : null,
      to_present: e.duration || null,
      keys_in_family: family.length,
    });
  }
  // ---- trace: main-thread busy time, top-level events only (no double counting) ----
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
  const top = Object.entries(byName).sort((a, b) => b[1] - a[1]).slice(0, 12)
    .map(([k, v]) => ({ what: k, us_per_key: Math.round(v / Math.max(1, kd.length)) }));

  // ---- frames: presented vs dropped while typing ----
  const reporters = events.filter((e) => e.name === 'PipelineReporter' && e.ph === 'b' && e.args?.frame_reporter);
  const states = {};
  for (const r of reporters) {
    const f = r.args.frame_reporter;
    const k = f.state + (f.affects_smoothness ? '_affecting_smoothness' : '');
    states[k] = (states[k] || 0) + 1;
  }

  const ip = inpage.keys;
  return {
    regime: opts.name,
    where, focus: focus || 'off', pace_ms: pace, wall_ms: wall,
    document_in_editor: inEditor,
    keys_pressed: pressed,
    keys_seen_by_page: ip.length,
    keys_seen_in_trace: kd.length,
    keys_unresolved_in_page_probe: ip.filter((k) => k.commit == null).length,
    keys_missing_commit_time: per.filter((k) => k.to_commit == null).length,
    keys_missing_present_time: per.filter((k) => k.to_present == null).length,
    keys_sharing_a_frame: ip.length - inpage.rafs,
    frames: states,
    presented_within_one_60hz_frame_pct: r2(100 * per.filter((k) => k.to_present != null && k.to_present <= 16.67).length / Math.max(1, per.length)),
    presented_within_two_60hz_frames_pct: r2(100 * per.filter((k) => k.to_present != null && k.to_present <= 33.34).length / Math.max(1, per.length)),
    ms: {
      to_present: stat(per.map((k) => k.to_present)),
      to_commit: stat(per.map((k) => k.to_commit)),
      input_delay: stat(per.map((k) => k.input_delay)),
      js_per_key: stat(per.map((k) => k.js)),
      inpage_keydown_to_frame_task: stat(ip.map((k) => k.commit)),
      inpage_keydown_to_raf: stat(ip.map((k) => k.raf)),
      browser_event_timing_ge16ms_only: stat(inpage.evt),   // the JS API rounds to 8 ms and hides anything under 16 ms
    },
    main_thread: {
      busy_ms_per_key: r2(busy / 1000 / Math.max(1, kd.length)),
      busy_percent_of_wall: r2(100 * busy / 1000 / Math.max(1, wall)),
      // Where the per-keystroke time goes. The first line is Chrome's own editing of the
      // <textarea> (inserting one character into a 53 KB value), which no web editor can avoid;
      // the second is everything Quill runs in response.
      chrome_text_insertion_us_per_key: Math.round((byName['EventDispatch:textInput'] || 0) / Math.max(1, kd.length)),
      quill_input_handler_us_per_key: Math.round((byName['EventDispatch:input'] || 0) / Math.max(1, kd.length)),
      style_layout_paint_us_per_key: Math.round((['UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint'].reduce((s2, n) => s2 + (byName[n] || 0), 0)) / Math.max(1, kd.length)),
      top,
    },
  };
}

// ---------- runs ----------
async function measure(browser, page0) {
  const headless = !args.attach;
  const out = { env: env(browser, headless), startup: {}, typing: [] };

  if (!args.attach) {
    // ---- startup, one fresh browsing context per run (empty caches for the first, warm after) ----
    for (const [name, seed] of [['with_10k_doc', doc], ['empty_document', '']]) {
      const runs = [];
      for (let i = 0; i < RUNS; i++) {
        const ctx = await browser.newContext({ viewport: VIEW, colorScheme: 'light' });
        const p = await ctx.newPage();
        await p.addInitScript(lcpProbe);
        if (seed) await p.addInitScript(seedDoc, seed);
        await p.goto(URL_, { waitUntil: 'load' });
        runs.push(await readStartup(p));
        await ctx.close();
      }
      out.startup[name] = {
        runs: runs.length,
        note: name === 'with_10k_doc' ? `${words(doc)} words restored from local storage and fully rendered before the first frame` : 'empty editor',
        chars_rendered: runs[0].chars, lines_rendered: runs[0].lines,
        nav_to_first_contentful_paint: stat(runs.map((r) => r.fcp)),
        nav_to_editor_ready: stat(runs.map((r) => r.ready)),
        nav_to_largest_contentful_paint: stat(runs.map((r) => r.lcp)),
        nav_to_dom_content_loaded: stat(runs.map((r) => r.dcl)),
      };
    }
  }

  // ---- typing ----
  const ctx = args.attach ? page0.context() : await browser.newContext({ viewport: VIEW, colorScheme: 'light' });
  let page = page0;
  if (!args.attach) {
    page = await ctx.newPage();
    if (doc) await page.addInitScript(seedDoc, doc);
    await page.goto(URL_, { waitUntil: 'load' });
  }
  await page.evaluate(() => document.fonts.ready);
  const cdp = await ctx.newCDPSession(page);

  if (KEYS === 0) { if (!args.attach) await ctx.close(); return out; }
  const regimes = args.quick ? [{ name: 'paced', where: 'end', pace: PACE, focus: 'off' }] : [
    { name: 'paced_end_of_draft', where: 'end', pace: PACE, focus: 'off' },
    { name: 'paced_middle_of_draft', where: 'middle', pace: PACE, focus: 'off' },
    { name: 'paced_focus_mode', where: 'middle', pace: PACE, focus: 'sentence' },
    { name: 'burst_no_pause', where: 'end', pace: 0, focus: 'off' },
  ];
  for (const r of regimes) {
    out.typing.push(await typingRun(page, cdp, { keys: KEYS, ...r }));
    await page.evaluate(() => { const i = Writer.el.input; }); // keep the doc growing; no reset (worst case)
  }
  if (!args.attach) await ctx.close();
  return out;
}

// ---------- main ----------
let browser, page0, result;
if (args.attach) {
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${args.attach}`);
  const ctx = browser.contexts()[0];
  page0 = ctx.pages().find((p) => p.url().startsWith('http')) || (await ctx.waitForEvent('page'));
  await page0.waitForLoadState('load');
  // cold start of the app itself: process exec -> pixels, using the wall clock handed over by bin/quill
  const cold = {};
  if (args.t0) {
    const m = await readStartup(page0);                       // waits for the paint entry to exist
    m.origin = await page0.evaluate(() => performance.timeOrigin);
    const t0 = +args.t0;
    cold.cold_start = {
      note: 'bin/quill: shell exec of the launcher -> the named milestone, wall clock. Includes chromium process spawn, profile init, window creation, navigation and the full render of the document.',
      launcher_to_first_contentful_paint_ms: m.fcp == null ? null : r2(m.origin + m.fcp - t0),
      launcher_to_editor_ready_ms: m.ready == null ? null : r2(m.origin + m.ready - t0),
      launcher_to_navigation_start_ms: r2(m.origin - t0),
      navigation_to_first_contentful_paint_ms: r2(m.fcp),
      navigation_to_editor_ready_ms: r2(m.ready),
      navigation_to_dom_content_loaded_ms: r2(m.dcl),
      profile: args.profile || null,
      document_in_editor: await page0.evaluate(() => ({ chars: Writer.getText().length, lines: Writer.lineCount(), words: (Writer.getText().match(/[\p{L}\p{N}'’]+/gu) || []).length })),
    };
  }
  result = { ...cold, ...(await measure(browser, page0)) };
  if (args.seed) {                                   // leave the benchmark document in this profile
    await page0.evaluate((t) => { Writer.setText(t, { caret: t.length }); }, doc);
    await page0.waitForTimeout(900);
    await page0.reload({ waitUntil: 'load' });        // beforeunload flushes the document to storage
    await page0.waitForTimeout(300);
    result.seeded_chars = await page0.evaluate(() => Writer.getText().length);
    // A graceful browser shutdown, not a signal: local storage is committed to disk on exit, and
    // the next launch is only a real cold start if the document is actually there to be opened.
    try {
      const bs = await browser.newBrowserCDPSession();
      await bs.send('Browser.close').catch(() => {});
    } catch (e) {}
    for (let i = 0; i < 100; i++) {                   // wait for the endpoint to go away
      const ok = await fetch(`http://127.0.0.1:${args.attach}/json/version`).then(() => true).catch(() => false);
      if (!ok) break;
      await new Promise((r) => setTimeout(r, 50));
    }
  }
} else {
  browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
  result = await measure(browser);
  await browser.close();
}

const json = JSON.stringify(result, null, 2);
if (args.json) { fs.mkdirSync(path.dirname(args.json), { recursive: true }); fs.writeFileSync(args.json, json); }
if (!args.quiet) console.log(json);
if (args.attach) process.exit(0);      // the CDP connection would otherwise keep the launcher waiting
