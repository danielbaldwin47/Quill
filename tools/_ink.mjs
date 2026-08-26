import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const p = await b.newPage();
const enc=f=>fs.readFileSync(f).toString('base64');
const faces=[];
for (const [fam,dir,base] of [['Quattro','Quattro','iAWriterQuattroS'],['Mono','Mono','iAWriterMonoS'],['Duo','Duo','iAWriterDuoS']])
  for (const [st,wt,suf] of [['normal',400,'Regular'],['italic',400,'Italic'],['normal',700,'Bold'],['italic',700,'BoldItalic']])
    faces.push(`@font-face{font-family:S${fam};font-style:${st};font-weight:${wt};src:url(data:font/woff2;base64,${enc(`app/fonts/${dir}/${base}-${suf}.woff2`)}) format("woff2")}`);
await p.setContent('<style>'+faces.join('\n')+'</style><canvas id=c></canvas><div id=warm style="position:absolute;opacity:0"></div>');
await p.evaluate(async()=>{
  const w=document.getElementById('warm');
  for (const fam of ['SQuattro','SMono','SDuo']) for (const st of ['normal','italic']) for (const wt of [400,700]) {
    const s=document.createElement('span'); s.style.cssText=`font-family:${fam};font-style:${st};font-weight:${wt};font-size:20px`; s.textContent='ft!%j'; w.appendChild(s);
  }
  await document.fonts.ready; await new Promise(r=>setTimeout(r,300));
});
const r = await p.evaluate(()=>{
  const ctx=document.getElementById('c').getContext('2d');
  const tests=[['SQuattro','italic 700 1000px','f',450],['SQuattro','italic 700 1000px','t',450],['SQuattro','italic 700 1000px','!',600],['SDuo','italic 700 1000px','!',600],['SMono','normal 400 1000px','%',600],['SMono','normal 700 1000px','j',600]];
  return tests.map(([fam,f,ch,target])=>{
    ctx.font=f.replace('1000px','1000px "'+fam+'"');
    const m=ctx.measureText(ch);
    return {fam,f,ch,target,adv:+m.width.toFixed(1),inkL:+(-m.actualBoundingBoxLeft).toFixed(1),inkR:+m.actualBoundingBoxRight.toFixed(1)};
  });
});
console.table(r); await b.close();
