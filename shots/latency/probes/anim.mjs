// Does a caret animation that keeps running while you type cost latency?
// Same page, same keys; the only difference is one CSS rule that forces the blink to keep
// animating during typing (the app suppresses it). Reports keystroke -> presented frame.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const url = process.argv[2] || 'http://localhost:4183/';
const keys = +(process.argv[3] || 150);
const doc = fs.readFileSync('shots/latency/doc10k.md', 'utf8');
const pct=(a,p)=>{const s=[...a].sort((x,y)=>x-y);return s[Math.min(s.length-1,Math.max(0,Math.ceil(p*s.length)-1))]};
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
for (const forceAnim of [false, true]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
  const p = await ctx.newPage();
  await p.addInitScript((t) => { localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc', t); }, doc);
  await p.goto(url, { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  if (forceAnim) await p.addStyleTag({ content: '#caret-layer .caret { animation: caret-blink 1.06s linear infinite !important; }' });
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length);
    const r = Writer.caretRect(), sc = Writer.el.scroller; if (r) sc.scrollTop += (r.top - sc.getBoundingClientRect().top) - sc.clientHeight * 0.55; });
  const cdp = await ctx.newCDPSession(p);
  const ev = []; cdp.on('Tracing.dataCollected', (e) => { for (const x of e.value) if (x.name === 'EventTiming' && x.ph === 'b') ev.push(x.args.data); });
  const done = new Promise(r => cdp.once('Tracing.tracingComplete', r));
  for (let i = 0; i < 25; i++) { await p.keyboard.press('a'); await p.waitForTimeout(90); }
  await cdp.send('Tracing.start', { transferMode: 'ReportEvents', traceConfig: { recordMode: 'recordAsMuchAsPossible', includedCategories: ['devtools.timeline'] } });
  for (let i = 0; i < keys; i++) { await p.keyboard.press('b'); await p.waitForTimeout(90); }
  await p.waitForTimeout(400); await cdp.send('Tracing.end'); await done;
  const d = ev.filter(e => e.type === 'keydown').map(e => e.duration).filter(x => x);
  const c = ev.filter(e => e.type === 'keydown' && e.commitFinishTime).map(e => e.commitFinishTime - e.timeStamp);
  console.log((forceAnim ? 'caret animation running while typing ' : 'caret animation suppressed (app default)'),
    '| n', d.length, '| present p50 %s p90 %s p99 %s'.replace(/%s/g, () => ''), pct(d,.5).toFixed(2), pct(d,.9).toFixed(2), pct(d,.99).toFixed(2),
    '| commit p50', pct(c,.5).toFixed(2));
  await ctx.close();
}
await b.close();
