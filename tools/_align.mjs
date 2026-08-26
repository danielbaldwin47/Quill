import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text','--hide-scrollbars'] });
const text = fs.readFileSync('shots/type/specimen.md','utf8') + '\n\n' + fs.readFileSync('shots/type/passage.md','utf8');
const out=[];
for (const font of ['duo','quattro','mono']) for (const size of [10,13,16,18,20,24,27.65,32,40]) {
  const ctx = await b.newContext({ viewport:{width:1200,height:800}, deviceScaleFactor:2 });
  const p = await ctx.newPage();
  await p.addInitScript((s)=>{localStorage.setItem('quill.settings',JSON.stringify(s));localStorage.removeItem('quill.doc');},{theme:'light',font,focus:'off',typewriter:false,showChrome:false,fontSize:size});
  await p.goto('http://localhost:4173/',{waitUntil:'load'});
  await p.evaluate(async()=>{await document.fonts.ready;});
  const r = await p.evaluate((t)=>{
    Writer.setText(t,{caret:0});
    const inp=Writer.el.input, mir=Writer.el.mirror;
    const cs=getComputedStyle(mir);
    // textarea intrinsic height with its own wrapping
    const h0=inp.style.height; inp.style.height='0px';
    const sh=inp.scrollHeight; inp.style.height=h0;
    return {mirrorH:mir.offsetHeight, inputSH:sh, lh:cs.lineHeight, fs:cs.fontSize, pageW:document.getElementById('page').getBoundingClientRect().width};
  }, text);
  out.push({font,size,...r, delta: r.inputSH-r.mirrorH});
  await ctx.close();
}
console.table(out);
await b.close();
