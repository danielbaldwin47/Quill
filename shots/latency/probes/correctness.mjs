// Does the mirror still say the truth after the round-2 render changes?
// Types a fenced block into the middle of a long document and checks that every line below it is
// tokenised as code once the deferred catch-up has run — and that mirror text still equals the
// textarea text, line for line, in every case.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const url = process.argv[2] || 'http://localhost:4173/';
const doc = fs.readFileSync('shots/latency/doc52k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900} });
const p = await ctx.newPage();
await p.addInitScript((t)=>{ localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel','0'); }, doc);
await p.goto(url,{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
const res = await p.evaluate(async () => {
  const out = {};
  const i = Writer.el.input, M = Writer.el.mirror;
  const text = () => Array.from(M.children).map(e => e.textContent.replace(/ /g,' ')).join('\n');
  out.mirror_matches_textarea_at_boot = text() === i.value;
  // type a fence opener in the middle, one character at a time, through the real input path
  const at = i.value.indexOf('\n', Math.floor(i.value.length/2)) + 1;
  i.focus(); i.setSelectionRange(at, at);
  for (const ch of '```js\n') {
    document.execCommand('insertText', false, ch);
  }
  out.mirror_matches_textarea_right_after_fence = text() === i.value;
  const idx = Writer.lines().findIndex((l) => l.startsWith('```js'));
  out.fence_line_index = idx;
  out.fence_line_text = idx >= 0 ? Writer.lines()[idx] : null;
  out.fence_line_class = idx >= 0 ? Writer.lineEl(idx).className : null;
  out.line_1_below_class_immediately = Writer.lineEl(idx + 1) ? Writer.lineEl(idx + 1).className : null;
  out.line_400_below_class_immediately = Writer.lineEl(idx + 400) ? Writer.lineEl(idx + 400).className : null;
  await new Promise(r => setTimeout(r, 400));   // let the catch-up frames run
  out.line_400_below_class_after_catchup = Writer.lineEl(idx + 400) ? Writer.lineEl(idx + 400).className : null;
  out.line_3000_below_class_after_catchup = Writer.lineEl(3000) ? Writer.lineEl(3000).className : null;
  out.mirror_matches_textarea_after_catchup = text() === i.value;
  // and after undoing it all
  for (let k = 0; k < 6; k++) document.execCommand('delete');
  await new Promise(r => setTimeout(r, 400));
  out.mirror_matches_textarea_after_delete = text() === i.value;
  out.line_400_below_class_after_delete = Writer.lineEl(idx + 400) ? Writer.lineEl(idx + 400).className : null;
  out.heights_match = Math.abs(i.offsetHeight - M.offsetHeight) < 2;
  out.lines = Writer.lineCount();
  out.flushPending_exists = typeof Writer.flushPending === 'function';
  return out;
});
console.log(res);
await b.close();
