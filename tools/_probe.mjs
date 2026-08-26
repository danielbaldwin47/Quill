import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text','--hide-scrollbars'] });
const ctx = await b.newContext({ viewport:{width:942,height:350}, deviceScaleFactor:2 });
const p = await ctx.newPage();
await p.addInitScript((s)=>{localStorage.setItem('quill.settings',JSON.stringify(s));localStorage.removeItem('quill.doc');},{theme:'light',font:'duo',focus:'off',typewriter:false,showChrome:false,fontSize:27.65});
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(()=>{
  const page=document.getElementById('page'), sc=document.getElementById('scroller');
  const cs=getComputedStyle(page);
  const m=document.createElement('span'); m.style.cssText='position:absolute;white-space:pre;font:inherit';
  m.textContent='0'.repeat(100); page.appendChild(m);
  const chw=m.getBoundingClientRect().width/100; m.remove();
  return {pageW:page.getBoundingClientRect().width, scW:sc.getBoundingClientRect().width, lh:cs.lineHeight, fs:cs.fontSize, measure:cs.getPropertyValue('--measure'), ch:chw, mirrorH:document.getElementById('mirror').offsetHeight};
});
console.log(r); await b.close();
