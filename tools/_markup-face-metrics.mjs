import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const p = await (await b.newContext({viewport:{width:900,height:600}})).newPage();
await p.goto('http://localhost:4173/'); await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(() => {
  const out = [];
  for (const fam of ['iA Writer Duo','iA Writer Quattro','iA Writer Mono']) {
    for (const [w,s] of [[400,'normal'],[400,'italic'],[700,'normal'],[700,'italic']]) {
      const d = document.createElement('div');
      d.style.cssText = `position:absolute;font-family:"${fam}";font-weight:${w};font-style:${s};font-size:100px;line-height:normal;white-space:pre`;
      d.textContent = 'Hxg';
      document.body.appendChild(d);
      out.push([fam,w,s,d.getBoundingClientRect().height]);
      d.remove();
    }
  }
  return out;
});
console.log(r.map(x=>x.join(' ')).join('\n'));
await b.close();
