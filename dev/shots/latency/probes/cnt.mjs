import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync('dev/shots/latency/doc10k.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true });
const ctx = await b.newContext({ viewport:{width:1440,height:900}, reducedMotion:'reduce' });
const p = await ctx.newPage();
await p.addInitScript(t=>{localStorage.setItem('quill.doc',t);localStorage.setItem('quill.doc.sel',String(t.length));}, doc);
await p.goto('http://localhost:4173/',{waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{const i=Writer.el.input;i.focus();i.setSelectionRange(i.value.length,i.value.length);});
const c = await ctx.newCDPSession(p);
const chunks=[]; c.on('Tracing.dataCollected', e=>chunks.push(...e.value));
const done=new Promise(r=>c.on('Tracing.tracingComplete',r));
await c.send('Tracing.start',{transferMode:'ReportEvents',traceConfig:{recordMode:'recordAsMuchAsPossible',includedCategories:['devtools.timeline','blink,devtools.timeline','latency']}});
const s='the quick brown fox jumps over the lazy dog '; const N=120;
for(let i=0;i<N;i++){await p.keyboard.press(s[i%s.length]===' '?'Space':s[i%s.length]); await p.waitForTimeout(90);}
await p.waitForTimeout(400); await c.send('Tracing.end'); await done;
const cnt={}; const dur={};
for(const e of chunks) if(e.ph==='X'&&/UpdateLayoutTree|^Layout$|^Paint$|^PrePaint$|ForcedLayout|InvalidateLayout|Layerize/.test(e.name)){cnt[e.name]=(cnt[e.name]||0)+1; dur[e.name]=(dur[e.name]||0)+e.dur;}
for(const k of Object.keys(cnt)) console.log(k.padEnd(20), 'count/key='+(cnt[k]/N).toFixed(2), 'us/key='+Math.round(dur[k]/N));
// which layouts are forced (have a stack) vs frame
const lay = chunks.filter(e=>e.ph==='X'&&e.name==='Layout');
let forced=0; for(const e of lay) if(e.args&&e.args.beginData&&e.args.beginData.stackTrace) forced++;
console.log('Layout events', lay.length, 'with JS stack (forced):', forced);
const sample = lay.filter(e=>e.args?.beginData?.stackTrace).slice(0,3).map(e=>e.args.beginData.stackTrace[0]);
console.log(JSON.stringify(sample));
await b.close();
