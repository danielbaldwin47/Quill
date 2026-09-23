import { chromium } from 'playwright-core'; import fs from 'node:fs';
const pace = +(process.argv[2] ?? 60), keys = +(process.argv[3] ?? 200);
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const ctx = await b.newContext({ viewport:{width:1440,height:900} });
const p = await ctx.newPage();
const doc = fs.readFileSync('dev/shots/latency/doc10k.md','utf8');
await p.addInitScript(t=>{localStorage.setItem('quill.doc',t);localStorage.setItem('quill.doc.sel',String(t.length));}, doc);
await p.goto('http://localhost:4173/', {waitUntil:'load'});
await p.evaluate(()=>document.fonts.ready);
await p.evaluate(()=>{const i=Writer.el.input;i.focus();i.setSelectionRange(i.value.length,i.value.length);});
const c = await ctx.newCDPSession(p);
await c.send('Profiler.enable'); await c.send('Profiler.setSamplingInterval',{interval:50}); await c.send('Profiler.start');
const chunks=[]; c.on('Tracing.dataCollected', e=>chunks.push(...e.value));
const done = new Promise(r=>c.on('Tracing.tracingComplete', r));
await c.send('Tracing.start',{transferMode:'ReportEvents', traceConfig:{recordMode:'recordAsMuchAsPossible', includedCategories:['devtools.timeline','blink,devtools.timeline','latency','blink.user_timing']}});
const sample = 'the quick brown fox jumps over the lazy dog while alice considers the pleasure of making a daisy chain ';
for (let i=0;i<keys;i++){ const ch = sample[i%sample.length]; await p.keyboard.press(ch===' '?'Space':ch); if (pace) await p.waitForTimeout(pace); }
await p.waitForTimeout(400);
await c.send('Tracing.end'); await done;
const prof = (await c.send('Profiler.stop')).profile;
// self time per function
const byId = new Map(prof.nodes.map(n=>[n.id,n]));
const self = new Map();
const total = prof.samples.length;
const dt = prof.timeDeltas;
for (let i=0;i<prof.samples.length;i++){ const n=byId.get(prof.samples[i]); const f=n.callFrame; const k=(f.functionName||'(anon)')+' @'+(f.url||'').split('/').pop()+':'+f.lineNumber; self.set(k,(self.get(k)||0)+(dt[i]||0)); }
const totUs = dt.reduce((a,x)=>a+Math.max(0,x),0);
console.log('--- CPU self time (µs) over', (totUs/1000).toFixed(0),'ms wall,', keys,'keys ---');
console.log([...self].sort((a,b)=>b[1]-a[1]).slice(0,22).map(([k,v])=>(v/1000).toFixed(1).padStart(8)+' ms  '+(v/keys).toFixed(0).padStart(6)+' µs/key  '+k).join('\n'));
// trace: per event type processing, and frame phases
const evt = chunks.filter(e=>e.name==='EventTiming'&&e.ph==='b').map(e=>e.args.data);
const kd = evt.filter(e=>e.type==='keydown');
const q=(a,p)=>{const s=[...a].sort((x,y)=>x-y);return s.length?s[Math.min(s.length-1,Math.floor(p*s.length))]:NaN;};
const f=(a)=>`n=${a.length} p50=${q(a,.5).toFixed(2)} p90=${q(a,.9).toFixed(2)} p99=${q(a,.99).toFixed(2)} max=${Math.max(...a).toFixed(2)}`;
console.log('\nkeydown EventTiming duration (event->presentation):', f(kd.map(e=>e.duration)));
console.log('keydown commitFinish-timeStamp:', f(kd.filter(e=>e.commitFinishTime).map(e=>e.commitFinishTime-e.timeStamp)));
console.log('input delay (processingStart-timeStamp):', f(kd.map(e=>e.processingStart-e.timeStamp)));
for (const t of ['keydown','keypress','beforeinput','input','keyup']) { const a=evt.filter(e=>e.type===t); if(a.length) console.log('processing',t,':',f(a.map(e=>e.processingEnd-e.processingStart))); }
const dur = {};
for (const e of chunks) if (e.ph==='X' && e.dur && /UpdateLayoutTree|Layout$|Paint|PrePaint|CommitLoad|FunctionCall|EventDispatch|HitTest|ParseHTML|UpdateLayer/.test(e.name)) { const k=e.name+(e.args&&e.args.data&&e.args.data.type?':'+e.args.data.type:''); dur[k]=(dur[k]||0)+e.dur; }
console.log('\n--- trace X-event total dur (ms) / per key (µs) ---');
console.log(Object.entries(dur).sort((a,b)=>b[1]-a[1]).slice(0,18).map(([k,v])=>(v/1000).toFixed(1).padStart(8)+' ms '+(v/keys).toFixed(0).padStart(6)+' µs/key  '+k).join('\n'));
await b.close();
