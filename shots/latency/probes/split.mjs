// Where does the app-internal keystroke cost actually go? Run the same 250-keystroke
// session with one part of the page switched off at a time and compare.
//   node shots/latency/probes/split.mjs [port] [keys] [doc]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT = process.argv[2] || '4173', KEYS = +(process.argv[3] || 250);
const DOC = process.argv[4] || 'shots/latency/doc10k.md';
const doc = fs.readFileSync(DOC, 'utf8');
const q = (a, p) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.max(0, Math.ceil(p * s.length) - 1))] : NaN; };
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const f = (a) => `mean=${mean(a).toFixed(2)} p50=${q(a,.5).toFixed(2)} p99=${q(a,.99).toFixed(2)} max=${Math.max(...a).toFixed(2)} n=${a.length}`;

const VARIANTS = {
  baseline: '',
  no_mirror: '#mirror{display:none!important}',
  no_caret_layer: '#caret-layer{display:none!important}',
  no_chrome: '.chrome{display:none!important}',
  mirror_contain: '#mirror .line{contain:layout style}',
};
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
for (const [name, css] of Object.entries(VARIANTS)) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
  const p = await ctx.newPage();
  await p.addInitScript(([t, c]) => {
    localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length));
    if (c) addEventListener('DOMContentLoaded', () => { const s = document.createElement('style'); s.textContent = c; document.head.appendChild(s); });
  }, [doc, css]);
  await p.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
  const c = await ctx.newCDPSession(p);
  const chunks = []; c.on('Tracing.dataCollected', e => chunks.push(...e.value));
  const done = new Promise(r => c.on('Tracing.tracingComplete', r));
  await c.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline', 'blink,devtools.timeline', 'latency'] } });
  const sample = 'the quick brown fox jumps over the lazy dog while alice considers a daisy chain ';
  for (let i = 0; i < KEYS + 25; i++) { const ch = sample[i % sample.length]; await p.keyboard.press(ch === ' ' ? 'Space' : ch); await p.waitForTimeout(40); }
  await p.waitForTimeout(400);
  await c.send('Tracing.end'); await done;
  const evt = chunks.filter(e => e.name === 'EventTiming' && e.ph === 'b').map(e => e.args.data);
  const kd = evt.filter(e => e.type === 'keydown' && e.duration).slice(25);
  const dur = {};
  for (const e of chunks) if (e.ph === 'X' && e.dur && /UpdateLayoutTree|^Layout$|^Paint$|^PrePaint$|EventDispatch/.test(e.name)) { const k = e.name + (e.args?.data?.type ? ':' + e.args.data.type : ''); dur[k] = (dur[k] || 0) + e.dur; }
  const per = Object.fromEntries(Object.entries(dur).map(([k, v]) => [k, Math.round(v / (KEYS + 25))]));
  console.log(name.padEnd(16), f(kd.map(e => e.duration)));
  console.log(' '.repeat(16), Object.entries(per).sort((a, b) => b[1] - a[1]).slice(0, 8).map(([k, v]) => `${k}=${v}`).join(' '));
  await ctx.close();
}
await b.close();
