import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const p = await (await b.newContext({viewport:{width:900,height:600}})).newPage();
await p.goto('http://localhost:4173/'); await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(() => {
  const out = [];
  for (const fam of ['iA Writer Duo','iA Writer Quattro','iA Writer Mono']) {
    for (const [w,s] of [[400,'normal'],[400,'italic'],[700,'normal'],[700,'italic']]) {
      const d = document.createElement('div');
      d.style.cssText = `position:absolute;top:0;left:0;font-family:"${fam}";font-weight:${w};font-style:${s};font-size:1000px;line-height:normal;white-space:pre`;
      d.innerHTML = 'Hxg<span style="display:inline-block;width:0;height:0;vertical-align:baseline"></span>';
      document.body.appendChild(d);
      const dr = d.getBoundingClientRect(), sr = d.querySelector('span').getBoundingClientRect();
      out.push([fam, w, s, 'H='+dr.height, 'ascent='+(sr.top-dr.top), 'descent='+(dr.bottom-sr.top)]);
      d.remove();
    }
  }
  return out;
});
console.log(r.map(x=>x.join(' ')).join('\n'));
await b.close();
