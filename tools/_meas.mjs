// node /tmp/meas.mjs img.png cols|rows x y w h thr
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const [inp, mode, x, y, w, h, thr='240'] = process.argv.slice(2);
const mime = inp.endsWith('.webp') ? 'image/webp' : inp.endsWith('.jpg') ? 'image/jpeg' : 'image/png';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true }); const p = await b.newPage();
const res = await p.evaluate(async ([src, mode, x, y, w, h, thr]) => {
  const img = new Image(); img.src = src; await img.decode();
  const c = document.createElement('canvas'); c.width = w; c.height = h;
  const g = c.getContext('2d'); g.fillStyle='#fff'; g.fillRect(0,0,w,h); g.drawImage(img, x, y, w, h, 0, 0, w, h);
  const d = g.getImageData(0,0,w,h).data;
  const lum = (i)=> 0.299*d[i]+0.587*d[i+1]+0.114*d[i+2];
  const out = [];
  if (mode === 'cols') { for (let cx=0; cx<w; cx++){ let m=255; for(let cy=0;cy<h;cy++){ const v=lum((cy*w+cx)*4); if(v<m)m=v; } out.push(m); } }
  else { for (let cy=0; cy<h; cy++){ let m=255; for(let cx=0;cx<w;cx++){ const v=lum((cy*w+cx)*4); if(v<m)m=v; } out.push(m); } }
  // runs below threshold
  const runs=[]; let s=-1;
  out.forEach((v,i)=>{ if(v<thr){ if(s<0)s=i; } else { if(s>=0){runs.push([s,i-1]); s=-1;} } });
  if(s>=0) runs.push([s,out.length-1]);
  return { runs, sample: out.map(v=>Math.round(v)) };
}, ['data:'+mime+';base64,'+fs.readFileSync(inp).toString('base64'), mode, +x, +y, +w, +h, +thr]);
const off = mode==='cols' ? +x : +y;
console.log('runs:', res.runs.map(r=>`${r[0]+off}-${r[1]+off}(${r[1]-r[0]+1})`).join(' '));
await b.close();
