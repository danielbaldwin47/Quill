import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const p = await b.newPage();
const enc=f=>fs.readFileSync(f).toString('base64');
const faces=[];
for (const [fam,dir,base] of [['Quattro','Quattro','iAWriterQuattroS'],['Mono','Mono','iAWriterMonoS'],['Duo','Duo','iAWriterDuoS']])
  for (const [st,wt,suf] of [['normal',400,'Regular'],['italic',400,'Italic'],['normal',700,'Bold'],['italic',700,'BoldItalic']])
    faces.push(`@font-face{font-family:S${fam};font-style:${st};font-weight:${wt};src:url(data:font/woff2;base64,${enc(`app/fonts/${dir}/${base}-${suf}.woff2`)}) format("woff2")}`);
await p.setContent('<style>'+faces.join('\n')+'</style><div id=d></div>');
await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(async ()=>{
  const chars=" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~—–“”‘’…";
  const d=document.getElementById('d');
  d.style.cssText='position:absolute;white-space:pre;font-size:1000px;font-kerning:none;font-variant-ligatures:none';
  const w=async(ch,fam,st,wt)=>{d.style.fontFamily=fam;d.style.fontStyle=st;d.style.fontWeight=wt;d.textContent=ch.repeat(10);await document.fonts.ready;return d.getBoundingClientRect().width/10;};
  const out={};
  for (const fam of ['SQuattro','SMono','SDuo']) {
    const diffs=[];
    for (const ch of chars) {
      const a=await w(ch,fam,'normal',400);
      const i=await w(ch,fam,'italic',400);
      const bo=await w(ch,fam,'normal',700);
      const bi=await w(ch,fam,'italic',700);
      if (Math.abs(a-i)>0.5||Math.abs(a-bo)>0.5||Math.abs(a-bi)>0.5) diffs.push({ch,reg:a,ital:i,bold:bo,bi});
    }
    out[fam]=diffs;
  }
  return out;
});
console.log(JSON.stringify(r,null,1)); await b.close();
