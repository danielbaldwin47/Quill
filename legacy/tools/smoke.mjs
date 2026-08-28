import { chromium } from 'playwright-core';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const errors = [];
const results = {};
for (const [theme, font] of [['light','duo'],['dark','quattro'],['light','mono']]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, colorScheme: theme });
  const p = await ctx.newPage();
  p.on('pageerror', e => errors.push(theme+'/'+font+': '+e.message)); p.on('console', m => { if (m.type()==='error') errors.push(theme+'/'+font+' console: '+m.text()); });
  await p.addInitScript(([t,f]) => { localStorage.clear(); localStorage.setItem('quill.settings', JSON.stringify({ theme: t, font: f, focus: 'off', typewriter: false })); }, [theme, font]);
  await p.goto('http://localhost:4173/', { waitUntil: 'load' }); await p.evaluate(() => document.fonts.ready);
  await p.click('#input');
  await p.keyboard.type('# The Lighthouse\n\nThe lamp had been lit for an *hour* before she noticed the **boat**.\n\n- rope\n- knife\n\n> Nothing out there is ever in a hurry.', { delay: 5 });
  await p.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
  const r = await p.evaluate(() => {
    const ta = Writer.el.input; const mirror = Writer.el.mirror;
    const mirrorText = [...mirror.querySelectorAll('.line')].map(l => l.textContent).join('\n');
    const caret = document.querySelector('#caret-layer .caret'); const cr = caret && caret.getBoundingClientRect();
    const end = Writer.offsetRect(ta.value.length);
    const fontOk = document.fonts.check(`16px "${getComputedStyle(mirror).fontFamily.split(',')[0].replace(/"/g,'')}"`);
    return { len: ta.value.length, same: mirrorText === ta.value, mirrorLines: mirror.querySelectorAll('.line').length, taLines: ta.value.split('\n').length, caret: cr && { x: Math.round(cr.x), y: Math.round(cr.y), w: +cr.width.toFixed(1), h: +cr.height.toFixed(1) }, end: end && { x: Math.round(end.x), y: Math.round(end.y) }, heightMatch: Math.abs(ta.offsetHeight - mirror.offsetHeight) < 2, font: getComputedStyle(mirror).fontFamily.split(',')[0], fontLoaded: fontOk, words: document.body.innerText.match(/\d+ words?/)?.[0] };
  });
  results[theme+'/'+font] = r;
  await p.mouse.move(700, 20); await p.waitForTimeout(400);
  await p.screenshot({ path: `shots/final-${theme}-${font}.png` });
  await ctx.close();
}
// focus + typewriter + library
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, colorScheme: 'dark' });
const p = await ctx.newPage(); p.on('pageerror', e => errors.push('focus: '+e.message));
await p.addInitScript(() => { localStorage.clear(); localStorage.setItem('quill.settings', JSON.stringify({ theme: 'dark', font: 'duo', focus: 'sentence', typewriter: true })); });
await p.goto('http://localhost:4173/', { waitUntil: 'load' }); await p.evaluate(() => document.fonts.ready);
const doc = (await import('node:fs')).readFileSync('shots/latency/doc10k.md','utf8');
await p.evaluate((t) => { Writer.setText(t, { caret: 4000 }); Writer.el.input.focus(); }, doc);
await p.keyboard.type(' She waited.', { delay: 5 }); await p.waitForTimeout(500);
results.focus = await p.evaluate(() => { const sc = Writer.el.scroller; const r = Writer.caretRect(); return { caretYFrac: +((r.top + r.height/2 - sc.getBoundingClientRect().top) / sc.clientHeight).toFixed(2), dimmed: Writer.el.mirror.querySelectorAll('.dim').length, lines: Writer.lineCount() }; });
await p.screenshot({ path: 'shots/final-dark-focus-typewriter.png' });
// library / palette
await p.keyboard.press('Control+K'); await p.waitForTimeout(300); await p.screenshot({ path: 'shots/final-palette.png' }); await p.keyboard.press('Escape');
const cmds = await p.evaluate(() => Writer.commands().map(c => c.id));
results.commands = cmds.length; results.hasLibrary = cmds.filter(c => /library|sidebar/i.test(c));
if (results.hasLibrary.length) { await p.evaluate((id) => Writer.run(id), results.hasLibrary[0]); await p.waitForTimeout(500); await p.screenshot({ path: 'shots/final-library.png' }); }
await b.close();
console.log(JSON.stringify({ results, errors }, null, 1));
