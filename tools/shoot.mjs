// Screenshot the app in a given state.
// node tools/shoot.mjs --out shots/x.png [--w 1440 --h 900 --dpr 2] [--theme light|dark|auto] [--font duo|quattro|mono]
//   [--size 18] [--focus off|sentence|paragraph] [--typewriter] [--chrome on|off] [--text file.md] [--caret N|end|"needle"]
//   [--mouse] (move mouse so chrome shows) [--nocaret] [--select a,b] [--url http://localhost:4173/] [--wait ms] [--full]
import { chromium } from 'playwright-core'; import fs from 'node:fs'; import path from 'node:path';
const args = {}; for (let i = 2; i < process.argv.length; i++) { const a = process.argv[i]; if (a.startsWith('--')) { const k = a.slice(2); const v = process.argv[i + 1]; if (v === undefined || v.startsWith('--')) args[k] = true; else { args[k] = v; i++; } } }
const W = +(args.w || 1440), H = +(args.h || 900), dpr = +(args.dpr || 2);
const url = args.url || process.env.QUILL_URL || 'http://localhost:4173/';
const settings = { theme: args.theme || 'light', font: args.font || 'duo', focus: args.focus || 'off', typewriter: !!args.typewriter, showChrome: (args.chrome || 'on') !== 'off' };
if (args.size) settings.fontSize = +args.size;
const text = args.text ? fs.readFileSync(args.text, 'utf8') : null;
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--font-render-hinting=none', '--disable-lcd-text', '--hide-scrollbars'] });
const ctx = await b.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: dpr, colorScheme: settings.theme === 'dark' ? 'dark' : 'light', reducedMotion: 'no-preference' });
const p = await ctx.newPage();
await p.addInitScript((s) => { localStorage.setItem('quill.settings', JSON.stringify(s)); localStorage.removeItem('quill.doc'); localStorage.removeItem('quill.doc.sel'); }, settings);
await p.goto(url, { waitUntil: 'load' });
await p.evaluate(async () => { await document.fonts.ready; });
if (text !== null) {
  let caret = text.length;
  if (args.caret && args.caret !== 'end') { if (/^\d+$/.test(args.caret)) caret = +args.caret; else { const i = text.indexOf(args.caret); caret = i >= 0 ? i + args.caret.length : text.length; } }
  await p.evaluate(([t, c]) => { Writer.setText(t, { caret: c }); Writer.el.input.focus(); }, [text, caret]);
}
if (args.select) { const [a, c] = args.select.split(',').map(Number); await p.evaluate(([a, c]) => Writer.setSelection(a, c), [a, c]); }
if (args.mouse) { await p.mouse.move(W / 2, 20); await p.evaluate(() => { document.documentElement.dataset.typing = 'off'; }); }
else await p.evaluate(() => { document.documentElement.dataset.typing = 'off'; });
if (args.typing) await p.evaluate(() => { document.documentElement.dataset.typing = 'on'; });
// freeze caret visible & un-blinking for deterministic shots
await p.evaluate((hide) => { const c = document.querySelector('#caret-layer .caret'); if (c) { c.classList.remove('blink'); c.style.opacity = hide ? '0' : '1'; } }, !!args.nocaret);
await p.evaluate(() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))));
if (args.wait) await p.waitForTimeout(+args.wait);
const out = args.out || 'shots/shot.png'; fs.mkdirSync(path.dirname(out), { recursive: true });
await p.screenshot({ path: out, fullPage: !!args.full });
await b.close(); console.log('wrote', out, `${W}x${H}@${dpr}x`);
