import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const p = await b.newPage();
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(async ()=>{
  const S='The quick brown fox jumps over the lazy dog — MWmw il0O 123 “q”, (a) *x*';
  const out=[];
  const d=document.createElement('div');
  d.textContent=S; d.style.cssText='position:absolute;left:-9999px;top:0;white-space:pre;font-size:100px;font-kerning:none;font-variant-ligatures:none;font-feature-settings:"kern" 0,"liga" 0,"clig" 0,"calt" 0;text-rendering:geometricPrecision';
  document.body.appendChild(d);
  for (const fam of ['iA Writer Duo','iA Writer Quattro','iA Writer Mono']) {
    d.style.fontFamily = '"'+fam+'"';
    for (const st of ['normal','italic']) {
      d.style.fontStyle = st;
      const ws=[];
      for (const w of [400,415,500,700]) { d.style.fontWeight=w; await document.fonts.ready; ws.push(+d.getBoundingClientRect().width.toFixed(4)); }
      out.push({fam,st,w400:ws[0],w415:ws[1],w500:ws[2],w700:ws[3],same: new Set(ws).size===1});
    }
  }
  d.remove(); return out;
});
console.table(r); await b.close();
