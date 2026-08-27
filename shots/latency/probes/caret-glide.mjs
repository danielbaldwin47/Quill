// The caret is what a writer watches. caret.js glides it to its new column over GLIDE_X = 62 ms
// whenever two keystrokes are more than SNAP_MS = 60 ms apart, which at any normal writing speed
// is every keystroke. This measures, per keystroke: when the glyph's frame is presented, and when
// the caret has actually stopped moving — by reading the caret's box every animation frame.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync('shots/latency/doc10k.md','utf8');
const url = process.argv[2] || 'http://localhost:4173/';
const pace = +(process.argv[3] || 90);
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900} });
const p = await ctx.newPage();
await p.addInitScript((t)=>{ localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel',String(t.length)); localStorage.removeItem('quill.lib'); }, doc);
await p.goto(url,{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{ const i=Writer.el.input; i.focus(); i.setSelectionRange(i.value.length,i.value.length); });
await p.evaluate(() => {
  window.__k = []; window.__f = [];
  const caret = document.querySelector('#caret-layer .caret');
  addEventListener('keydown', (e) => window.__k.push(e.timeStamp), true);
  (function frame(ts) {
    requestAnimationFrame(frame);
    if (!caret) return;
    const r = caret.getBoundingClientRect();
    window.__f.push([performance.now(), Math.round(r.left * 100) / 100, Math.round(r.top * 100) / 100]);
  })();
});
const sample = 'the quick brown fox jumps over a lazy dog and considers the pleasure of a daisy chain ';
for (let i = 0; i < 60; i++) { await p.keyboard.press(sample[i % sample.length] === ' ' ? 'Space' : sample[i % sample.length]); await p.waitForTimeout(pace); }
await p.waitForTimeout(400);
const { k, f } = await p.evaluate(() => ({ k: window.__k, f: window.__f }));
// For each keystroke: the caret's final position is where it is just before the NEXT keystroke;
// the settle time is the first frame after this keystroke at which it is already there.
const v = [];
for (let i = 0; i < k.length - 1; i++) {
  const t0 = k[i], t1 = k[i + 1];
  const win = f.filter((x) => x[0] >= t0 && x[0] < t1);
  if (win.length < 2) continue;
  const fin = win[win.length - 1];
  const hit = win.find((x) => Math.abs(x[1] - fin[1]) < 0.6 && Math.abs(x[2] - fin[2]) < 0.6);
  if (hit) v.push(hit[0] - t0);
}
v.sort((a, b) => a - b);
const pc = (q) => v.length ? +v[Math.min(v.length - 1, Math.ceil(q * v.length) - 1)].toFixed(2) : null;
console.log(JSON.stringify({ url, pace_ms: pace, keystrokes_measured: v.length,
  keydown_to_caret_stationary_ms: { p50: pc(.5), p90: pc(.9), p99: pc(.99), max: pc(1) } }));
await b.close();
