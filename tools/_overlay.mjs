import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text','--hide-scrollbars'] });
const text = fs.readFileSync(process.argv[5]||'shots/type/specimen.md','utf8');
const font=process.argv[2]||'duo', size=+(process.argv[3]||20), out=process.argv[4]||'/tmp/ov.png';
const ctx = await b.newContext({ viewport:{width:1200,height:900}, deviceScaleFactor:2 });
const p = await ctx.newPage();
await p.addInitScript((s)=>{localStorage.setItem('quill.settings',JSON.stringify(s));localStorage.removeItem('quill.doc');},{theme:'light',font,focus:'off',typewriter:false,showChrome:false,fontSize:size});
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(async()=>{await document.fonts.ready;});
await p.evaluate((t)=>{ Writer.setText(t,{caret:0});
  const st=document.createElement('style');
  st.textContent='#input{color:rgba(255,0,0,.85)!important;-webkit-text-fill-color:rgba(255,0,0,.85)!important;caret-color:transparent!important}#mirror{color:#000!important}#mirror span{color:#000!important}#caret-layer{display:none}';
  document.head.appendChild(st);
}, text);
await p.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
await p.screenshot({path:out});
// measure max horizontal/vertical mismatch by scanning
await b.close(); console.log('wrote',out);
