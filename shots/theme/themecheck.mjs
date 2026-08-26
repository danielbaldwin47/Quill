import { chromium } from 'playwright-core'; import fs from 'node:fs';
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const state = (p) => p.evaluate(() => ({ bg: getComputedStyle(document.documentElement).backgroundColor, fg: getComputedStyle(document.documentElement).color, scheme: getComputedStyle(document.documentElement).colorScheme, theme: document.documentElement.dataset.theme, app: document.documentElement.dataset.appearance, shift: document.documentElement.dataset.themeShift || '-' }));
async function open(colorScheme, stored) {
  const ctx = await b.newContext({ colorScheme, viewport: { width: 800, height: 500 }, deviceScaleFactor: 1 });
  const p = await ctx.newPage();
  if (stored) await p.addInitScript((s) => localStorage.setItem('quill.settings', JSON.stringify(s)), stored);
  await p.goto('http://localhost:4173/', { waitUntil: 'load' });
  return { p, ctx };
}
let { p, ctx } = await open('dark', null);
console.log('boot auto/OS-dark  ', JSON.stringify(await state(p)));
await p.evaluate(() => Writer.run('theme.toggle')); await p.waitForTimeout(400);
console.log('after toggle       ', JSON.stringify(await state(p)));
await p.evaluate(() => Writer.el.input.focus());
await p.keyboard.press('Control+Alt+n'); await p.waitForTimeout(400);
console.log('after ctrl+alt+n   ', JSON.stringify(await state(p)));
await p.evaluate(() => Writer.run('theme.auto')); await p.waitForTimeout(400);
console.log('after theme.auto   ', JSON.stringify(await state(p)));
await p.emulateMedia({ colorScheme: 'light' }); await p.waitForTimeout(400);
console.log('OS flips to light  ', JSON.stringify(await state(p)));
await p.emulateMedia({ colorScheme: 'dark' }); await p.waitForTimeout(400);
console.log('OS flips to dark   ', JSON.stringify(await state(p)));
await ctx.close();
// no-flash: stored dark while the OS is light — sample frames from the first commit
const c3 = await b.newContext({ colorScheme: 'light', viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 });
const p3 = await c3.newPage();
await p3.addInitScript((s) => localStorage.setItem('quill.settings', JSON.stringify(s)), { theme: 'dark' });
await p3.goto('http://localhost:4173/', { waitUntil: 'commit' });
const px = [];
for (let i = 0; i < 8; i++) { const s = await p3.screenshot({ clip: { x: 2, y: 2, width: 2, height: 2 } }); px.push(s.toString('base64').slice(0, 24)); }
fs.writeFileSync('/tmp/flashframes.txt', px.join('\n'));
console.log('first-paint frames identical:', new Set(px).size === 1, '(unique:', new Set(px).size + ')');
console.log('stored dark/OS light', JSON.stringify(await state(p3)));
await c3.close(); await b.close();
