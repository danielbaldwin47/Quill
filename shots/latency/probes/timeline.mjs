// Print the main-thread event sequence for a few individual keystrokes.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const PORT=process.argv[2]||'4173';
const doc = fs.readFileSync(process.argv[3]||'shots/latency/doc10k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900}, reducedMotion:'reduce' });
const p = await ctx.newPage();
await p.addInitScript(t=>{localStorage.setItem('quill.doc',t);localStorage.setItem('quill.doc.sel',String(t.length));}, doc);
await p.goto(`http://localhost:${PORT}/`,{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{const i=Writer.el.input;i.focus();i.setSelectionRange(i.value.length,i.value.length);});
const c = await ctx.newCDPSession(p);
const chunks=[]; c.on('Tracing.dataCollected', e=>chunks.push(...e.value));
const done=new Promise(r=>c.on('Tracing.tracingComplete',r));
await c.send('Tracing.start',{transferMode:'ReportEvents',traceConfig:{recordMode:'recordAsMuchAsPossible',includedCategories:['devtools.timeline','blink,devtools.timeline','latency','disabled-by-default-devtools.timeline.stack']}});
const s='the quick brown fox jumps over the lazy dog '; const N=40;
for(let i=0;i<N;i++){await p.keyboard.press(s[i%s.length]===' '?'Space':s[i%s.length]); await p.waitForTimeout(90);}
await p.waitForTimeout(400); await c.send('Tracing.end'); await done;
const et = chunks.filter(e=>e.name==='EventTiming'&&e.ph==='b').map(e=>({...e.args.data, ts:e.ts}));
const kd = et.filter(e=>e.type==='keydown'&&e.duration).slice(20,24);
const X = chunks.filter(e=>e.ph==='X'&&e.dur!=null&&!/EventTiming|RunTask|ThreadControllerImpl/.test(e.name)).sort((a,b)=>a.ts-b.ts);
for (const e of kd) {
  console.log(`\n=== keydown  present=${e.duration.toFixed(2)}ms ===`);
  for (const x of X) if (x.ts >= e.ts-200 && x.ts <= e.ts + e.duration*1000 + 2000) {
    const st = x.args?.beginData?.stackTrace?.[0] || x.args?.data?.stackTrace?.[0];
    console.log(`  +${((x.ts-e.ts)/1000).toFixed(2).padStart(7)}ms ${(x.dur/1000).toFixed(2).padStart(6)}ms ${x.name}${x.args?.data?.type?':'+x.args.data.type:''}${st?'   <- '+(st.functionName||'?')+' '+(st.url||'').split('/').pop()+':'+st.lineNumber:''}`);
  }
}
await b.close();
