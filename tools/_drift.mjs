import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text','--hide-scrollbars'] });
let text=''; for(let i=0;i<200;i++) text += (i%7===3?'':'Line '+i+' of a long document that keeps going and going and wraps a bit maybe not.')+'\n';
const ctx = await b.newContext({ viewport:{width:1200,height:800}, deviceScaleFactor:2 });
const p = await ctx.newPage();
await p.addInitScript((s)=>{localStorage.setItem('quill.settings',JSON.stringify(s));localStorage.removeItem('quill.doc');},{theme:'light',font:'duo',focus:'off',typewriter:false,showChrome:false,fontSize:27.65});
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(async()=>{await document.fonts.ready;});
const r=await p.evaluate((t)=>{
  Writer.setText(t,{caret:0});
  const mir=Writer.el.mirror, inp=Writer.el.input;
  const pitch=parseFloat(getComputedStyle(mir).lineHeight);
  const mr=mir.getBoundingClientRect();
  let maxErr=0, rows=0;
  const kids=[...mir.children];
  for(const el of kids){ const r=el.getBoundingClientRect(); const expect=mr.top+rows*pitch; maxErr=Math.max(maxErr,Math.abs(r.top-expect)); rows+=Math.round(r.height/pitch); }
  return {pitch, rows, maxErr, mirrorH:mir.getBoundingClientRect().height, expectH:rows*pitch, inputH:inp.getBoundingClientRect().height};
}, text);
console.log(r); await b.close();
