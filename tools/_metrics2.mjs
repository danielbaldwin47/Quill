import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const p = await b.newPage();
const enc=f=>fs.readFileSync(f).toString('base64');
const faces=[];
for (const [fam,dir,base] of [['Duo','Duo','iAWriterDuoS'],['Quattro','Quattro','iAWriterQuattroS'],['Mono','Mono','iAWriterMonoS']]) {
  for (const [st,wt,suf] of [['normal',400,'Regular'],['italic',400,'Italic'],['normal',700,'Bold'],['italic',700,'BoldItalic']]) {
    faces.push(`@font-face{font-family:S${fam};font-style:${st};font-weight:${wt};src:url(data:font/woff2;base64,${enc(`app/fonts/${dir}/${base}-${suf}.woff2`)}) format("woff2")}`);
  }
  faces.push(`@font-face{font-family:V${fam};font-style:normal;font-weight:100 900;src:url(data:font/woff2;base64,${enc(`app/fonts/${dir}/iAWriter${fam}V.woff2`)}) format("woff2")}`);
  faces.push(`@font-face{font-family:V${fam};font-style:italic;font-weight:100 900;src:url(data:font/woff2;base64,${enc(`app/fonts/${dir}/iAWriter${fam}V-Italic.woff2`)}) format("woff2")}`);
}
await p.setContent('<style>'+faces.join('\n')+'</style><div id=d></div>');
await p.evaluate(async()=>{await document.fonts.ready;});
const r = await p.evaluate(async ()=>{
  const S='The quick brown fox jumps over the lazy dog MWmw il0O 123 (a) x';
  const d=document.getElementById('d'); d.textContent=S;
  d.style.cssText='position:absolute;white-space:pre;font-size:100px;font-kerning:none;font-variant-ligatures:none;font-feature-settings:"kern" 0,"liga" 0,"clig" 0,"calt" 0';
  const out=[];
  for (const fam of ['SDuo','VDuo','SQuattro','VQuattro','SMono','VMono']) {
    d.style.fontFamily=fam; const row={fam};
    for (const [k,st,w] of [['reg','normal',400],['ital','italic',400],['bold','normal',700],['bi','italic',700],['w415','normal',415],['w415i','italic',415]]) {
      d.style.fontStyle=st; d.style.fontWeight=w;
      await document.fonts.ready;
      row[k]=+d.getBoundingClientRect().width.toFixed(3);
    }
    out.push(row);
  }
  return out;
});
console.table(r); await b.close();
