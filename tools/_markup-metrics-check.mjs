import { chromium } from 'playwright-core'; import fs from 'node:fs';
const text = fs.readFileSync('shots/markup/kitchen.md','utf8') + '\n' + fs.readFileSync('shots/markup/writing.md','utf8');
const b = await chromium.launch({ executablePath:'/usr/bin/chromium', headless:true, args:['--font-render-hinting=none','--disable-lcd-text'] });
for (const font of ['duo','quattro','mono']) {
  const ctx = await b.newContext({ viewport:{width:1200,height:900}, deviceScaleFactor:2 });
  const p = await ctx.newPage();
  await p.addInitScript((f)=>localStorage.setItem('quill.settings', JSON.stringify({theme:'light',font:f,fontSize:20,focus:'off',showChrome:false})), font);
  await p.goto('http://localhost:4173/'); await p.evaluate(async()=>{await document.fonts.ready;});
  await p.evaluate((t)=>Writer.setText(t,{caret:0}), text);
  const res = await p.evaluate(() => {
    // a plain, span-free clone of each line, laid out in the same box
    const mirror = document.getElementById('mirror');
    const probe = document.createElement('div');
    probe.style.cssText = 'position:absolute;left:0;top:0;width:100%;visibility:hidden;white-space:pre-wrap;overflow-wrap:break-word';
    probe.className = mirror.className;
    mirror.parentNode.appendChild(probe);
    const bad = [];
    const lines = Writer.lines();
    for (let i=0;i<lines.length;i++){
      const el = Writer.lineEl(i); if (!el || !lines[i].length) continue;
      const plain = document.createElement('div'); plain.className='line'; plain.textContent = lines[i];
      probe.textContent=''; probe.appendChild(plain);
      const r1 = el.getBoundingClientRect(), r2 = plain.getBoundingClientRect();
      if (Math.abs(r1.height - r2.height) > 0.5) { bad.push({i, why:'height', a:r1.height, b:r2.height, text:lines[i].slice(0,40)}); continue; }
      // x of the last character in both
      const last = (node) => { const w=document.createTreeWalker(node,NodeFilter.SHOW_TEXT); let n,prev=null; while((n=w.nextNode())) prev=n;
        if(!prev) return null; const rg=document.createRange(); rg.setStart(prev,prev.data.length-1); rg.setEnd(prev,prev.data.length);
        const rs=rg.getClientRects(); return rs.length? {x:rs[rs.length-1].right - node.getBoundingClientRect().left, y: rs[rs.length-1].top - node.getBoundingClientRect().top} : null; };
      const a = last(el), c = last(plain);
      if (a && c && (Math.abs(a.x-c.x) > 0.5 || Math.abs(a.y-c.y) > 0.5)) bad.push({i, why:'lastchar', a, b:c, text:lines[i].slice(0,40)});
    }
    probe.remove();
    return {n:lines.length, bad};
  });
  console.log(font, 'lines', res.n, 'drift', res.bad.length, JSON.stringify(res.bad.slice(0,6)));
  await ctx.close();
}
await b.close();
