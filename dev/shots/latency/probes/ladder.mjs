// One keystroke, five milestones. For every keydown, take from Chrome's own trace:
//   t0        the event's hardware timestamp
//   handled   processingEnd of the `input` event (the app's JS is finished)
//   painted   end of the main-frame update (style+layout+paint) that carried it   <- Typometer class
//   committed EventTiming.commitFinishTime (handed to the compositor)
//   presented EventTiming.duration (presentation feedback of the frame)
//   node dev/shots/latency/probes/ladder.mjs [port] [keys] [pace] [doc] [json]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT = process.argv[2] || '4173', KEYS = +(process.argv[3] || 300), PACE = +(process.argv[4] ?? 90);
const DOC = process.argv[5] || 'dev/shots/latency/doc10k.md', OUT = process.argv[6] || '';
const doc = fs.readFileSync(DOC, 'utf8');
const q = (a, p) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.max(0, Math.ceil(p * s.length) - 1))] : NaN; };
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const sd = (a) => { const m = mean(a); return Math.sqrt(a.reduce((s, x) => s + (x - m) ** 2, 0) / (a.length - 1 || 1)); };
const f = (a) => `mean=${mean(a).toFixed(2)} sd=${sd(a).toFixed(2)} p50=${q(a,.5).toFixed(2)} p95=${q(a,.95).toFixed(2)} p99=${q(a,.99).toFixed(2)} max=${Math.max(...a).toFixed(2)}`;

const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
const p = await ctx.newPage();
await p.addInitScript((t) => { localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length)); }, doc);
await p.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
await p.evaluate(() => document.fonts.ready);
await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
const c = await ctx.newCDPSession(p);
const chunks = []; c.on('Tracing.dataCollected', e => chunks.push(...e.value));
const done = new Promise(r => c.on('Tracing.tracingComplete', r));
await c.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline', 'blink,devtools.timeline', 'latency', 'benchmark', 'disabled-by-default-devtools.timeline.frame'] } });
const sample = 'the quick brown fox jumps over the lazy dog while alice considers a daisy chain ';
for (let i = 0; i < KEYS + 25; i++) { const ch = sample[i % sample.length]; await p.keyboard.press(ch === ' ' ? 'Space' : ch); if (PACE) await p.waitForTimeout(PACE); }
await p.waitForTimeout(500);
await c.send('Tracing.end'); await done;

const et = chunks.filter(e => e.name === 'EventTiming' && e.ph === 'b').map(e => ({ ...e.args.data, ts: e.ts }));
// main-frame updates on the renderer main thread, in trace time (µs)
const frames = chunks.filter(e => e.ph === 'X' && (e.name === 'ProxyMain::BeginMainFrame' || e.name === 'BeginMainThreadFrame')).map(e => ({ a: e.ts, b: e.ts + e.dur })).sort((x, y) => x.a - y.a);
const paints = chunks.filter(e => e.ph === 'X' && e.name === 'Paint').map(e => e.ts + e.dur).sort((a, b) => a - b);
const kd = et.filter(e => e.type === 'keydown' && e.duration).slice(25);
const inputs = et.filter(e => e.type === 'input').sort((a, b) => a.ts - b.ts);
const rows = [];
for (const e of kd) {
  // the `input` event dispatched for this keystroke: first one at or after this keydown's trace ts
  const inp = inputs.find(i => i.ts >= e.ts - 500);
  const handledUs = inp ? (inp.ts + (inp.processingEnd - inp.processingStart) * 1000) : null;
  const after = handledUs ?? e.ts;
  const fr = frames.find(fm => fm.b >= after);
  const pt = paints.find(t => t >= after);
  rows.push({
    t0: e.timeStamp,
    delay: e.processingStart - e.timeStamp,
    handled: handledUs ? (handledUs - e.ts) / 1000 + (e.processingStart - e.timeStamp) * 0 + (e.ts - e.ts) : null,
    handled_ms: handledUs ? (handledUs - e.ts) / 1000 : null,
    painted_ms: pt ? (pt - e.ts) / 1000 : null,
    frameend_ms: fr ? (fr.b - e.ts) / 1000 : null,
    commit_ms: e.commitFinishTime ? e.commitFinishTime - e.timeStamp : null,
    present_ms: e.duration,
  });
}
const col = (k) => rows.map(r => r[k]).filter(x => x != null);
console.log('n =', rows.length, ' pace =', PACE, ' doc =', DOC);
for (const k of ['handled_ms', 'painted_ms', 'frameend_ms', 'commit_ms', 'present_ms']) console.log(k.padEnd(12), f(col(k)));
const pres = col('present_ms');
const bins = {}; for (const v of pres) { const b8 = Math.floor(v / 2) * 2; bins[b8] = (bins[b8] || 0) + 1; }
console.log('present histogram (2 ms bins):', Object.entries(bins).sort((a, b) => a[0] - b[0]).map(([k, v]) => `${k}-${+k + 2}:${v}`).join(' '));
if (OUT) fs.writeFileSync(OUT, JSON.stringify({ pace: PACE, doc: DOC, rows }, null, 1));
await b.close();
