// How much spare time is there inside one keystroke?
//
// The round-2 critique said to "close the ~2 ms" between the measured app-internal mean and the
// <=5 ms bar by removing work. This asks whether removing work would move the number at all: it
// ADDS a measured amount of pure busy-wait to every `input` event and watches keydown -> committed
// frame. If the keystroke is work-bound, the line rises 1:1 from zero. If it is scheduler-bound —
// the app finishing early and then waiting for Chromium's next BeginFrame — the line is flat until
// the added work exceeds the slack, and only then rises.
//
//   node shots/latency/probes/spare.mjs [port] [keys] [pace] [doc] [json]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT = process.argv[2] || '4179', KEYS = +(process.argv[3] || 150), PACE = +(process.argv[4] ?? 90);
const DOC = process.argv[5] || 'shots/latency/doc10k.md', OUT = process.argv[6] || '';
const doc = fs.readFileSync(DOC, 'utf8');
const q = (a, p) => { const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.max(0, Math.ceil(p * s.length) - 1))]; };
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const r2 = (x) => Math.round(x * 100) / 100;
const ADDED = [0, 0.5, 1, 2, 4, 8];
const rows = [];
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
for (const add of ADDED) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
  const p = await ctx.newPage();
  await p.addInitScript(([t, ms]) => {
    localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length));
    if (ms > 0) addEventListener('load', () => {
      // after the app's own listener, so it is added to the keystroke, not substituted for it
      Writer.el.input.addEventListener('input', () => { const t0 = performance.now(); while (performance.now() - t0 < ms) {} });
    });
  }, [doc, add]);
  await p.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
  const c = await ctx.newCDPSession(p);
  const chunks = []; c.on('Tracing.dataCollected', (e) => chunks.push(...e.value));
  const done = new Promise((r) => c.on('Tracing.tracingComplete', r));
  await c.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline', 'blink,devtools.timeline', 'latency'] } });
  const sample = 'the quick brown fox jumps over the lazy dog while alice considers a daisy chain ';
  for (let i = 0; i < KEYS + 20; i++) { const ch = sample[i % sample.length]; await p.keyboard.press(ch === ' ' ? 'Space' : ch); if (PACE) await p.waitForTimeout(PACE); }
  await p.waitForTimeout(400);
  await c.send('Tracing.end'); await done;
  const et = chunks.filter((e) => e.name === 'EventTiming' && e.ph === 'b').map((e) => e.args.data);
  const kd = et.filter((e) => e.type === 'keydown' && e.commitFinishTime).slice(20);
  const commit = kd.map((e) => e.commitFinishTime - e.timeStamp);
  const pres = kd.filter((e) => e.duration).map((e) => e.duration);
  rows.push({ added_ms: add, n: commit.length, commit_mean: r2(mean(commit)), commit_p50: r2(q(commit, .5)), commit_max: r2(Math.max(...commit)), present_mean: r2(mean(pres)) });
  console.log(rows[rows.length - 1]);
  await ctx.close();
}
const base = rows[0].commit_mean;
console.log('\nadded ms ->  commit mean (delta vs 0)');
for (const r of rows) console.log(`  +${String(r.added_ms).padStart(4)}  ${String(r.commit_mean).padStart(6)}  (${r2(r.commit_mean - base) >= 0 ? '+' : ''}${r2(r.commit_mean - base)})`);
if (OUT) fs.writeFileSync(OUT, JSON.stringify({ doc: DOC, keys: KEYS, pace_ms: PACE, rows }, null, 1));
await b.close();
