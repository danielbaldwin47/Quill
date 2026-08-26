import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const p = await b.newPage();
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(async ()=>{
  const chars=" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~—–“”‘’…·«»";
  const d=document.createElement('div'); document.body.appendChild(d);
  d.style.cssText='position:absolute;white-space:pre;font-size:1000px;font-kerning:none;font-variant-ligatures:none;font-feature-settings:"kern" 0,"liga" 0,"clig" 0,"calt" 0';
  const meas=async(s,fam,st,wt)=>{d.style.fontFamily='"'+fam+'"';d.style.fontStyle=st;d.style.fontWeight=wt;d.textContent=s;await document.fonts.ready;return d.getBoundingClientRect().width;};
  const out={};
  for (const fam of ['iA Writer Duo','iA Writer Quattro','iA Writer Mono']) {
    const diffs=[];
    for (const ch of chars) {
      const w=async(st,wt)=> (await meas('X'+ch.repeat(10)+'X',fam,st,wt) - await meas('XX',fam,st,wt))/10;
      const a=await w('normal',400), i=await w('italic',400), bo=await w('normal',700), bi=await w('italic',700);
      if (Math.max(Math.abs(a-i),Math.abs(a-bo),Math.abs(a-bi))>0.5) diffs.push({ch,reg:Math.round(a),ital:Math.round(i),bold:Math.round(bo),bi:Math.round(bi)});
    }
    out[fam]=diffs;
  }
  return out;
});
for (const k of Object.keys(r)) { console.log('==',k); console.table(r[k]); }
await b.close();
