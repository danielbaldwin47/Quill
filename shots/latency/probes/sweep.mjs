// Try one page-level change at a time and price it in keydown->paint / ->present.
//   node shots/latency/probes/sweep.mjs [port] [keys] [pace] [doc]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT = process.argv[2] || '4173', KEYS = +(process.argv[3] || 250), PACE = +(process.argv[4] ?? 90);
const DOC = process.argv[5] || 'shots/latency/doc10k.md';
const doc = fs.readFileSync(DOC, 'utf8');
const q = (a, p) => { const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.max(0, Math.ceil(p * s.length) - 1))]; };
const mean = (a) => a.reduce((x, y) => x + y, 0) / a.length;
const V = {
  baseline: { css: '' },
  line_contain_layout: { css: '#mirror .line{contain:layout}' },
  line_contain_layout_style: { css: '#mirror .line{contain:layout style}' },
  line_contain_content_hidden: { css: '' , drop: 'has'},
  mirror_contain_layout: { css: '#mirror{contain:layout}' },
  both: { css: '#mirror .line{contain:layout style}', drop: 'has' },
};
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
for (const [name, v] of Object.entries(V)) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
  const p = await ctx.newPage();
  await p.addInitScript(([t, c, drop]) => {
    localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length));
    addEventListener('DOMContentLoaded', () => {
      if (drop === 'has') for (const ss of document.styleSheets) { try { for (let i = ss.cssRules.length - 1; i >= 0; i--) if (/:has\(/.test(ss.cssRules[i].selectorText || '')) ss.deleteRule(i); } catch (e) {} }
      if (c) { const s = document.createElement('style'); s.textContent = c; document.head.appendChild(s); }
    });
  }, [doc, v.css, v.drop || '']);
  await p.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
  const c = await ctx.newCDPSession(p);
  const chunks = []; c.on('Tracing.dataCollected', e => chunks.push(...e.value));
  const done = new Promise(r => c.on('Tracing.tracingComplete', r));
  await c.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline', 'blink,devtools.timeline', 'latency'] } });
  const sample = 'the quick brown fox jumps over the lazy dog while alice considers a daisy chain ';
  for (let i = 0; i < KEYS + 25; i++) { const ch = sample[i % sample.length]; await p.keyboard.press(ch === ' ' ? 'Space' : ch); if (PACE) await p.waitForTimeout(PACE); }
  await p.waitForTimeout(400);
  await c.send('Tracing.end'); await done;
  const et = chunks.filter(e => e.name === 'EventTiming' && e.ph === 'b').map(e => ({ ...e.args.data, ts: e.ts }));
  const paints = chunks.filter(e => e.ph === 'X' && e.name === 'Paint').map(e => e.ts + e.dur).sort((a, b) => a - b);
  const inputs = et.filter(e => e.type === 'input').sort((a, b) => a.ts - b.ts);
  const kd = et.filter(e => e.type === 'keydown' && e.duration).slice(25);
  const painted = [], present = [], handled = [];
  for (const e of kd) {
    const inp = inputs.find(i => i.ts >= e.ts - 500);
    const h = inp ? inp.ts + (inp.processingEnd - inp.processingStart) * 1000 : e.ts;
    const pt = paints.find(t => t >= h);
    if (inp) handled.push((h - e.ts) / 1000);
    if (pt) painted.push((pt - e.ts) / 1000);
    present.push(e.duration);
  }
  const dur = {}; for (const e of chunks) if (e.ph === 'X' && e.dur && /UpdateLayoutTree|^Layout$|^Paint$|^PrePaint$|EventDispatch:input/.test(e.name)) dur[e.name] = (dur[e.name] || 0) + e.dur;
  console.log(name.padEnd(28),
    `jsw=${mean(handled).toFixed(2)}`,
    `paint mean=${mean(painted).toFixed(2)} p50=${q(painted,.5).toFixed(2)} max=${Math.max(...painted).toFixed(2)}`,
    `| present mean=${mean(present).toFixed(2)} p99=${q(present,.99).toFixed(2)} max=${Math.max(...present).toFixed(2)}`,
    '|', Object.entries(dur).map(([k, x]) => `${k.replace('EventDispatch:','ED:')}=${Math.round(x / (KEYS + 25))}`).join(' '));
  await ctx.close();
}
await b.close();
