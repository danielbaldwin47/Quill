// Functional check of the core changes: heights, selection events, caret, fonts, themes.
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const out = [];
for (const [theme, font] of [['light','duo'],['dark','quattro'],['light','mono']]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: theme });
  const p = await ctx.newPage();
  const doc = fs.readFileSync('dev/shots/latency/doc10k.md','utf8');
  await p.addInitScript(([t,s]) => { localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc', t); localStorage.setItem('quill.settings', JSON.stringify(s)); }, [doc, {theme, font, fontSize:20, focus:'off', showChrome:true}]);
  await p.goto('http://localhost:4173/', { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  const r = {};
  r.bootHeights = await p.evaluate(() => [Writer.el.input.offsetHeight, Writer.el.mirror.offsetHeight]);
  // type text that adds lines, then check the textarea grew with the mirror
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(0,0); });
  for (let i=0;i<12;i++) await p.keyboard.press('Enter');
  await p.waitForTimeout(200);
  r.afterEnterHeights = await p.evaluate(() => [Writer.el.input.offsetHeight, Writer.el.mirror.offsetHeight]);
  // caret rect must be on the same line box as the mirror's line
  r.caretOk = await p.evaluate(() => {
    Writer.setSelection(0,0);
    const c = document.querySelector('#caret-layer .caret');
    const rect = Writer.caretRect(); const line = Writer.lineEl(0).getBoundingClientRect();
    return !!c && Math.abs(rect.top - line.top) < line.height;
  });
  // glyph alignment: the mirror line and the same text in the textarea must be the same width
  r.align = await p.evaluate(() => {
    const el = Writer.lineEl(30); const r = document.createRange(); r.selectNodeContents(el);
    const w = r.getBoundingClientRect().width;
    const probe = document.createElement('div');
    const cs = getComputedStyle(Writer.el.input);
    probe.style.cssText = `position:absolute;visibility:hidden;white-space:pre;font:${cs.font};letter-spacing:${cs.letterSpacing}`;
    probe.textContent = Writer.lines()[30];
    document.body.appendChild(probe);
    const pw = probe.getBoundingClientRect().width; probe.remove();
    return [Math.round(w*100)/100, Math.round(pw*100)/100];
  });
  // font size change must resync the height
  await p.evaluate(() => Writer.setSetting('fontSize', 28));
  await p.waitForTimeout(250);
  r.afterFontHeights = await p.evaluate(() => [Writer.el.input.offsetHeight, Writer.el.mirror.offsetHeight]);
  r.errors = await p.evaluate(() => window.__errs || []);
  out.push({ theme, font, ...r });
  await ctx.close();
}
console.log(JSON.stringify(out, null, 1));
await b.close();
