import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text'] });
const p = await b.newPage();
const v = fs.readFileSync('ref/ia/fonts/Duo/iAWriterDuoV.ttf').toString('base64');
const s = fs.readFileSync('app/fonts/Duo/iAWriterDuoS-Regular.woff2').toString('base64');
await p.setContent('<style>@font-face{font-family:DuoV;src:url(data:font/ttf;base64,'+v+') format("truetype");font-weight:100 900}@font-face{font-family:DuoS;src:url(data:font/woff2;base64,'+s+') format("woff2")}body{margin:0}#a{font-family:DuoV;font-size:100px;white-space:pre;display:inline-block}</style><div id="a">nnnnnnnnnnnnnnnnnnnn</div><div id="c" style="font-family:DuoV;font-size:100px;display:inline-block;white-space:pre">mmmmmmmmmm</div><div id="d" style="font-family:DuoS;font-size:100px;display:inline-block;white-space:pre">nnnnnnnnnnnnnnnnnnnn</div>');
await p.evaluate(()=>document.fonts.ready);
const r = await p.evaluate(()=>{
  const out={};
  const a=document.getElementById('a'), c=document.getElementById('c'), d=document.getElementById('d');
  out.static = d.getBoundingClientRect().width;
  for (const w of [200,300,400,500,600,700,800]) {
    a.style.fontVariationSettings = `'wght' ${w}`;
    c.style.fontVariationSettings = `'wght' ${w}`;
    out['n'+w]= a.getBoundingClientRect().width;
    out['m'+w]= c.getBoundingClientRect().width;
  }
  return out;
});
console.log(r);
// axes
const ax = await p.evaluate(()=>{ return null; });
await b.close();
