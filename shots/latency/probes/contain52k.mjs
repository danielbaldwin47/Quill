// Is `contain: layout style` on a mirror line worth anything on a 55k-word manuscript?
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync(process.argv[3] || 'shots/latency/doc52k.md', 'utf8');
const q=(a,p)=>{const s=[...a].sort((x,y)=>x-y);return s[Math.min(s.length-1,Math.max(0,Math.ceil(p*s.length)-1))]};
const mean=a=>a.reduce((x,y)=>x+y,0)/a.length;
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
for (const rep of [1,2]) for (const [name,css] of Object.entries({baseline:'', contain:'#mirror .line{contain:layout style}'})) {
  const ctx = await b.newContext({ viewport:{width:1440,height:900}, reducedMotion:'reduce' });
  const p = await ctx.newPage();
  await p.addInitScript(([t,c])=>{localStorage.setItem('quill.doc',t);localStorage.setItem('quill.doc.sel',String(t.length));if(c)addEventListener('DOMContentLoaded',()=>{const s=document.createElement('style');s.textContent=c;document.head.appendChild(s)})},[doc,css]);
  await p.goto(`http://localhost:${process.argv[2]||'4173'}/`,{waitUntil:'load'});
  await p.evaluate(()=>document.fonts.ready);
  await p.evaluate(()=>{const i=Writer.el.input;i.focus();i.setSelectionRange(i.value.length,i.value.length);});
  const c = await ctx.newCDPSession(p);
  const chunks=[]; c.on('Tracing.dataCollected',e=>chunks.push(...e.value));
  const done=new Promise(r=>c.on('Tracing.tracingComplete',r));
  await c.send('Tracing.start',{transferMode:'ReportEvents',traceConfig:{recordMode:'recordAsMuchAsPossible',includedCategories:['devtools.timeline','blink,devtools.timeline','latency']}});
  const s='the quick brown fox jumps over the lazy dog '; const N=150;
  for(let i=0;i<N;i++){await p.keyboard.press(s[i%s.length]===' '?'Space':s[i%s.length]);await p.waitForTimeout(90);}
  await p.waitForTimeout(400); await c.send('Tracing.end'); await done;
  const et=chunks.filter(e=>e.name==='EventTiming'&&e.ph==='b').map(e=>e.args.data);
  const kd=et.filter(e=>e.type==='keydown'&&e.duration).slice(20).map(e=>e.duration);
  const dur={}; for(const e of chunks) if(e.ph==='X'&&e.dur&&/UpdateLayoutTree|^Layout$|^Paint$|^PrePaint$|EventDispatch:input/.test(e.name)) dur[e.name]=(dur[e.name]||0)+e.dur;
  console.log(`rep${rep}`, name.padEnd(9), `mean=${mean(kd).toFixed(2)} p50=${q(kd,.5).toFixed(2)} p99=${q(kd,.99).toFixed(2)} max=${Math.max(...kd).toFixed(2)}`, '|', Object.entries(dur).map(([k,v])=>k.replace('EventDispatch:','')+'='+Math.round(v/N)).join(' '));
  await ctx.close();
}
await b.close();
