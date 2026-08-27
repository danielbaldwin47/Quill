// Where does an Enter go? Times Writer.render(false) alone (the value mutation is outside the
// clock), then the layout the frame still owes, for a line-count change vs an in-line edit.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync(process.argv[2] || 'shots/latency/doc52k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900} });
const p = await ctx.newPage();
await p.addInitScript((t)=>{ localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel',String(t.length)); }, doc);
await p.goto(process.argv[3] || 'http://localhost:4185/', { waitUntil:'load' });
await p.evaluate(()=>document.fonts.ready);
console.log(process.argv[2], await p.evaluate(() => {
  const i = Writer.el.input, M = Writer.el.mirror, N = 25;
  const out = { lines: Writer.lineCount(), chars: i.value.length };
  const at = i.value.indexOf('\n', Math.floor(i.value.length/2)) + 1;
  const ins = (s) => { const v = i.value; i.value = v.slice(0,at) + s + v.slice(at); i.setSelectionRange(at+s.length, at+s.length); };
  const del = (n) => { const v = i.value; i.value = v.slice(0,at) + v.slice(at+n); i.setSelectionRange(at,at); };
  function run(s) {
    let render = 0, layout = 0;
    for (let k = 0; k < N; k++) {
      ins(s);  void M.offsetHeight;
      let t = performance.now(); Writer.render(false); render += performance.now() - t;
      t = performance.now(); void M.offsetHeight; layout += performance.now() - t;
      del(s.length); void M.offsetHeight;
      t = performance.now(); Writer.render(false); render += performance.now() - t;
      t = performance.now(); void M.offsetHeight; layout += performance.now() - t;
    }
    return { render_ms: +(render/(2*N)).toFixed(3), layout_ms: +(layout/(2*N)).toFixed(3) };
  }
  out.inline_char = run('x');
  out.newline = run('\n');
  out.fence_backtick = run('```js\n');     // opens a fenced block: every line below changes context
  const els = Array.from(document.querySelectorAll('#mirror .line'));
  let t = performance.now(); for (let k=0;k<10;k++) { for (let j=0;j<els.length;j++) { const s = String(j + (k&1)); if (els[j].dataset.i !== s) els[j].dataset.i = s; } }
  out.renumber_every_line_ms = +((performance.now()-t)/10).toFixed(3);
  return out;
}));
await b.close();
