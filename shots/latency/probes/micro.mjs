import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync(process.argv[2] || 'shots/latency/doc52k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900} });
const p = await ctx.newPage();
await p.addInitScript((t)=>{ localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel',String(t.length)); }, doc);
await p.goto(process.argv[3] || 'http://localhost:4185/', { waitUntil:'load' });
await p.evaluate(()=>document.fonts.ready);
console.log(await p.evaluate(() => {
  const i = Writer.el.input, N = 40, out = {};
  const time = (f) => { const t = performance.now(); for (let k=0;k<N;k++) f(); return +((performance.now()-t)/N).toFixed(3); };
  let sink;
  out.chars = i.value.length; out.lines = Writer.lineCount();
  out.value_access_ms = time(() => { sink = i.value.length; });
  out.split_ms = time(() => { sink = i.value.split('\n').length; });
  const lines = i.value.split('\n');
  out.tokenize_longest_line_ms = (() => { let L=''; for (const l of lines) if (l.length>L.length) L=l; const t=performance.now(); for(let k=0;k<N;k++) sink=Writer.tokenizeLine(L,null,0).length; return +((performance.now()-t)/N).toFixed(3); })();
  out.full_render_ms = (() => { const t=performance.now(); for(let k=0;k<5;k++) Writer.render(true); return +((performance.now()-t)/5).toFixed(2); })();
  // simulate an inline edit and an Enter through the same path the input handler uses
  const mid = i.value.indexOf('\n', Math.floor(i.value.length/2));
  i.setSelectionRange(mid, mid);
  out.render_incremental_same_linecount_ms = (() => { const t=performance.now(); for(let k=0;k<N;k++) Writer.render(false); return +((performance.now()-t)/N).toFixed(3); })();
  return out;
}));
await b.close();
