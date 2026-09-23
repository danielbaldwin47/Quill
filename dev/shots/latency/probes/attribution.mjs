// Who spends the keystroke? Splits the main-thread work of one keystroke into
//   Chrome's own textarea insertion | Quill's JavaScript | the layout Quill forces by measuring
//   the caret (work the frame would have done anyway, moved earlier) | the frame's style/layout/paint
// by nesting the trace's X events inside one another rather than summing overlapping totals.
//   node dev/shots/latency/probes/attribution.mjs [port] [doc] [keys] [pace] [json]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT = process.argv[2] || '4179', DOC = process.argv[3] || 'dev/shots/latency/doc10k.md';
const KEYS = +(process.argv[4] || 250), PACE = +(process.argv[5] ?? 90), OUT = process.argv[6] || '';
const doc = fs.readFileSync(DOC, 'utf8');
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
const p = await ctx.newPage();
await p.addInitScript((t) => { localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length)); }, doc);
await p.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
await p.evaluate(() => document.fonts.ready);
await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
const c = await ctx.newCDPSession(p);
const chunks = []; c.on('Tracing.dataCollected', (e) => chunks.push(...e.value));
const done = new Promise((r) => c.on('Tracing.tracingComplete', r));
await c.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline', 'blink,devtools.timeline', 'latency', 'disabled-by-default-devtools.timeline.stack'] } });
const sample = 'the quick brown fox jumps over the lazy dog while alice considers a daisy chain ';
for (let i = 0; i < KEYS; i++) { const ch = sample[i % sample.length]; await p.keyboard.press(ch === ' ' ? 'Space' : ch); if (PACE) await p.waitForTimeout(PACE); }
await p.waitForTimeout(400);
await c.send('Tracing.end'); await done;

const X = chunks.filter((e) => e.ph === 'X' && e.dur != null).sort((a, b2) => a.ts - b2.ts || b2.dur - a.dur);
const inside = (e, w) => e.ts >= w.ts && e.ts + e.dur <= w.ts + w.dur + 1;
const inputs = X.filter((e) => e.name === 'EventDispatch' && e.args?.data?.type === 'input');
const keypress = X.filter((e) => e.name === 'EventDispatch' && e.args?.data?.type === 'keypress');
const layoutish = (e) => e.name === 'Layout' || e.name === 'UpdateLayoutTree';
let forced = 0, quillJs = 0, chromeInsert = 0, frameWork = 0;
for (const w of inputs) {
  const nested = X.filter((e) => e !== w && layoutish(e) && inside(e, w));
  const l = nested.reduce((s, e) => s + e.dur, 0);
  forced += l; quillJs += w.dur - l;
}
for (const w of keypress) {
  const kids = inputs.filter((e) => inside(e, w)).reduce((s, e) => s + e.dur, 0);
  chromeInsert += w.dur - kids;
}
// everything style/layout/paint that is NOT inside an input handler = the frame's own pass
for (const e of X) {
  if (!['Layout', 'UpdateLayoutTree', 'PrePaint', 'Paint', 'Layerize'].includes(e.name)) continue;
  if (inputs.some((w) => inside(e, w)) || X.some((o) => o !== e && ['Layout', 'UpdateLayoutTree', 'PrePaint', 'Paint', 'Layerize'].includes(o.name) && o !== e && inside(e, o) && o.dur > e.dur)) continue;
  frameWork += e.dur;
}
const idle = X.filter((e) => e.name === 'FireIdleCallback').reduce((s, e) => s + e.dur, 0);
const per = (x) => Math.round(x / KEYS);
const res = {
  document: DOC, keys: KEYS, pace_ms: PACE,
  us_per_key: {
    chrome_textarea_insertion: per(chromeInsert),
    quill_input_handler_js: per(quillJs),
    layout_quill_forces_by_measuring_the_caret: per(forced),
    frame_style_layout_paint: per(frameWork),
    idle_callbacks_between_keystrokes: per(idle),
  },
};
console.log(JSON.stringify(res, null, 1));
if (OUT) fs.writeFileSync(OUT, JSON.stringify(res, null, 1));
await b.close();
