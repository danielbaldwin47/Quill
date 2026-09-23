import { chromium } from 'playwright-core'; import fs from 'node:fs';
const port = process.argv[2];
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
const p = await ctx.newPage();
const doc = fs.readFileSync('dev/shots/latency/doc10k.md', 'utf8');
await p.addInitScript((t) => { localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc', t); }, doc);
await p.goto('http://localhost:' + port + '/', { waitUntil: 'load' });
await p.evaluate(() => document.fonts.ready);
await p.evaluate(() => {
  const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length);
  window.__c = { selection: 0, render: 0, change: 0, layoutReads: 0 };
  for (const ev of ['selection', 'render', 'change']) Writer.on(ev, () => window.__c[ev]++);
  const gbcr = Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect = function () { window.__c.layoutReads++; return gbcr.call(this); };
  const gcr = Range.prototype.getClientRects;
  Range.prototype.getClientRects = function () { window.__c.layoutReads++; return gcr.call(this); };
});
for (let i = 0; i < 40; i++) { await p.keyboard.press('a'); await p.waitForTimeout(60); }
console.log(port, JSON.stringify(await p.evaluate(() => { const c = window.__c; for (const k in c) c[k] = +(c[k] / 40).toFixed(2); return c; })));
await b.close();
