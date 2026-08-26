// Measure startup time and keystroke-to-paint latency.
// node tools/latency.mjs [--url http://localhost:4173/] [--runs 10] [--keys 200] [--text file.md] [--json out.json]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const args = {}; for (let i = 2; i < process.argv.length; i++) { const a = process.argv[i]; if (a.startsWith('--')) { const k = a.slice(2); const v = process.argv[i + 1]; if (v === undefined || v.startsWith('--')) args[k] = true; else { args[k] = v; i++; } } }
const url = args.url || 'http://localhost:4173/'; const runs = +(args.runs || 10); const keys = +(args.keys || 200);
const text = args.text ? fs.readFileSync(args.text, 'utf8') : null;
const pct = (arr, p) => { const s = [...arr].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]; };
const stat = (arr) => !arr.length ? null : ({ n: arr.length, p50: +pct(arr, .5).toFixed(2), p90: +pct(arr, .9).toFixed(2), p99: +pct(arr, .99).toFixed(2), max: +Math.max(...arr).toFixed(2), mean: +(arr.reduce((a, b) => a + b, 0) / arr.length).toFixed(2) });
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--disable-gpu-vsync', '--disable-frame-rate-limit'] });
// ---- startup: navigation start -> first contentful paint, and -> editor ready (focused, rendered)
const fcp = [], ready = [], domLoad = [];
for (let i = 0; i < runs; i++) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
  const p = await ctx.newPage();
  if (text) await p.addInitScript((t) => localStorage.setItem('quill.doc', t), text);
  await p.goto(url, { waitUntil: 'load' });
  const m = await p.evaluate(() => new Promise(res => {
    const done = () => { const nav = performance.getEntriesByType('navigation')[0]; const paint = performance.getEntriesByType('paint').find(e => e.name === 'first-contentful-paint'); res({ fcp: paint ? paint.startTime : null, ready: window.__quillReady, dom: nav.domContentLoadedEventEnd }); };
    if (performance.getEntriesByType('paint').length) done(); else new PerformanceObserver(() => done()).observe({ type: 'paint', buffered: true });
  }));
  if (m.fcp != null) fcp.push(m.fcp); ready.push(m.ready); domLoad.push(m.dom);
  await ctx.close();
}
// ---- keystroke-to-paint: keydown timestamp -> first frame committed after the input was processed
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
const p = await ctx.newPage();
if (text) await p.addInitScript((t) => localStorage.setItem('quill.doc', t), text);
await p.goto(url, { waitUntil: 'load' }); await p.evaluate(() => document.fonts.ready);
await p.evaluate(() => { Writer.el.input.focus(); const v = Writer.el.input.value; Writer.el.input.setSelectionRange(v.length, v.length);
  window.__lat = []; const ch = new MessageChannel(); let pending = null;
  ch.port1.onmessage = () => { if (pending != null) { window.__lat.push(performance.now() - pending); pending = null; } };
  // Event Timing API gives the browser's own input->paint duration too
  window.__evt = []; try { new PerformanceObserver(l => { for (const e of l.getEntries()) if (e.name === 'keydown' || e.name === 'input') window.__evt.push({ name: e.name, dur: e.duration, proc: e.processingEnd - e.startTime }); }).observe({ type: 'event', durationThreshold: 0, buffered: true }); } catch (e) {}
  Writer.el.input.addEventListener('keydown', (e) => { pending = e.timeStamp; requestAnimationFrame(() => ch.port2.postMessage(0)); }, { capture: true });
});
const sample = 'the quick brown fox jumps over the lazy dog and keeps on running through the long afternoon ';
for (let i = 0; i < keys; i++) { const c = sample[i % sample.length]; await p.keyboard.press(c === ' ' ? 'Space' : c); if (i % 17 === 16) await p.keyboard.press('Enter'); }
await p.waitForTimeout(300);
const lat = await p.evaluate(() => window.__lat); const evt = await p.evaluate(() => window.__evt);
const evKeydown = evt.filter(e => e.name === 'keydown').map(e => e.dur);
const docLen = await p.evaluate(() => Writer.getText().length);
await b.close();
const result = { url, docChars: docLen, startup_ms: { first_contentful_paint: stat(fcp), editor_ready: stat(ready), dom_content_loaded: stat(domLoad) }, keystroke_to_paint_ms: { measured: stat(lat), event_timing_keydown_duration: evKeydown.length ? stat(evKeydown) : null }, note: 'keystroke_to_paint = keydown event timestamp -> frame committed after rAF (MessageChannel after paint). Event Timing durations are rounded to 8ms by the browser.' };
console.log(JSON.stringify(result, null, 2)); if (args.json) fs.writeFileSync(args.json, JSON.stringify(result, null, 2));
