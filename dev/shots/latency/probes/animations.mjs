// What is animating while the writer types? Anything that is puts every keystroke on the
// display's clock (see the report, "the display clock").
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync('dev/shots/latency/doc10k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.addInitScript((t)=>{ localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel',String(t.length)); localStorage.removeItem('quill.lib'); }, doc);
await p.goto(process.argv[2] || 'http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{ const i=Writer.el.input; i.focus(); i.setSelectionRange(i.value.length,i.value.length); });
await p.evaluate(() => { window.__seen = {}; window.__polls = 0; window.__any = 0; setInterval(() => {
  window.__polls++;
  let any = 0;
  for (const a of document.getAnimations()) {
    if (a.playState !== 'running') continue;
    any = 1;
    const t = a.effect && a.effect.target;
    const k = (a.animationName || a.transitionProperty || a.constructor.name) + ' on ' + (t ? (t.id || t.className || t.tagName) : '?');
    window.__seen[k] = (window.__seen[k] || 0) + 1;
  }
  window.__any += any;
}, 8); });
for (let i = 0; i < 40; i++) { await p.keyboard.press('a'); await p.waitForTimeout(90); }
console.log(process.argv[2] || 'live', await p.evaluate(() => ({ polls: window.__polls, polls_with_something_running: window.__any, by: window.__seen })));
await b.close();
