import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args:['--font-render-hinting=none','--disable-lcd-text','--hide-scrollbars'] });
const text = fs.readFileSync(process.argv[5]||'shots/type/specimen.md','utf8');
const font=process.argv[2]||'duo', size=+(process.argv[3]||20), out=process.argv[4]||'/tmp/sp';
for (const which of ['mirror','input']) {
  const ctx = await b.newContext({ viewport:{width:1200,height:900}, deviceScaleFactor:2 });
  const p = await ctx.newPage();
  await p.addInitScript((s)=>{localStorage.setItem('quill.settings',JSON.stringify(s));localStorage.removeItem('quill.doc');},{theme:'light',font,focus:'off',typewriter:false,showChrome:false,fontSize:size});
  await p.goto('http://localhost:4173/',{waitUntil:'load'});
  await p.evaluate(async()=>{await document.fonts.ready;});
  await p.evaluate(([t,w])=>{ Writer.setText(t,{caret:0});
    const st=document.createElement('style');
    st.textContent = w==='input'
      ? '#mirror{visibility:hidden}#input{color:#000!important;-webkit-text-fill-color:#000!important}#caret-layer{display:none}'
      : '#caret-layer{display:none}';
    document.head.appendChild(st);
  }, [text,which]);
  await p.evaluate(()=>new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))));
  await p.screenshot({path:out+'_'+which+'.png'});
  await ctx.close();
}
await b.close(); console.log('ok');
