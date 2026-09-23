import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync('dev/shots/latency/doc10k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const p = await (await b.newContext({viewport:{width:1440,height:900}})).newPage();
await p.addInitScript((t)=>{ localStorage.setItem('quill.doc',t); localStorage.setItem('quill.doc.sel',String(t.length)); localStorage.removeItem('quill.lib'); }, doc);
await p.goto(process.argv[2],{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{ const i=Writer.el.input; i.focus(); i.setSelectionRange(i.value.length,i.value.length); });
await p.evaluate(()=>{ window.__at=[]; window.addEventListener('keydown',()=>{ window.__at.push(document.getAnimations().filter(a=>a.playState==='running').map(a=>(a.animationName||a.transitionProperty)+'@'+((a.effect&&a.effect.target&&a.effect.target.className)||'?'))); },true); });
for (let burst=0; burst<6; burst++){ for(let i=0;i<8;i++){ await p.keyboard.press('a'); await p.waitForTimeout(90);} await p.waitForTimeout(1400); }
console.log(JSON.stringify(await p.evaluate(()=>window.__at.map((x,i)=>[i,x]).filter(([i,x])=>x.length))));
await b.close();
