// Screenshot the app in a given state.
// node legacy/tools/shoot.mjs --out shots/x.png [--w 1440 --h 900 --dpr 2] [--theme light|dark|auto] [--font duo|quattro|mono]
//   [--size 18] [--focus off|sentence|paragraph] [--typewriter] [--chrome on|off] [--text file.md] [--caret N|end|"needle"]
//   [--scroll px|"needle"] [--mouse] (move mouse so chrome shows) [--nocaret] [--select a,b] [--url http://localhost:4173/] [--wait ms] [--full]
//   [--active on|off]  off blurs the input, so the caret and any selection are drawn in their unfocused state
//   [--typing]  the chrome stepped back, the way it is while the writer is typing
//   [--menu view|document|stats|palette]  that popover open, its first row selected
//   [--state seed.json]  merge {localStorageKey: value} into localStorage before load (e.g. a demo Library)
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
const seed = args.state ? JSON.parse(fs.readFileSync(args.state, 'utf8')) : null;   // [files piece] --state seeds localStorage
await p.addInitScript(([s, seed]) => { localStorage.setItem('quill.settings', JSON.stringify(s)); localStorage.removeItem('quill.doc'); localStorage.removeItem('quill.doc.sel'); if (seed) for (const k in seed) localStorage.setItem(k, typeof seed[k] === 'string' ? seed[k] : JSON.stringify(seed[k])); }, [settings, seed]);
await p.goto(url, { waitUntil: 'load' });
await p.evaluate(async () => { await document.fonts.ready; });
if (text !== null) {
  let caret = text.length;
  if (args.caret && args.caret !== 'end') { if (/^\d+$/.test(args.caret)) caret = +args.caret; else { const i = text.indexOf(args.caret); caret = i >= 0 ? i + args.caret.length : text.length; } }
  await p.evaluate(([t, c]) => { Writer.setText(t, { caret: c }); Writer.el.input.focus(); }, [text, caret]);
}
// --scroll <px|needle>: put the document where the reference shot has it. A number is a
// scrollTop; a string scrolls the line that contains it to the top of the viewport. [chrome piece]
if (args.scroll !== undefined && args.scroll !== true) {
  await p.evaluate((v) => {
    const sc = Writer.el.scroller;
    if (/^-?\d+(\.\d+)?$/.test(v)) { sc.scrollTop = +v; return; }
    const i = Writer.getText().indexOf(v);
    if (i < 0) return;
    const r = Writer.offsetRect(i); if (!r) return;
    sc.scrollTop += r.top - sc.getBoundingClientRect().top;
  }, String(args.scroll));
}
if (args.select) { const [a, c] = args.select.split(',').map(Number); await p.evaluate(([a, c]) => Writer.setSelection(a, c), [a, c]); }
// --active off: this is the window the user has just clicked away from, so blur the input and let
// caret.js draw its unfocused state (#caret-layer.idle). --active on focuses the input even when
// there is no --text, so an empty Document is shot with a live caret on the paper. [caret piece]
const active = (args.active || 'on') !== 'off';
await p.evaluate((on) => { if (on) Writer.el.input.focus(); else Writer.el.input.blur(); }, active);
if (args.mouse) { await p.mouse.move(W / 2, 20); await p.evaluate(() => { document.documentElement.dataset.typing = 'off'; }); }
else await p.evaluate(() => { document.documentElement.dataset.typing = 'off'; });
if (args.typing) await p.evaluate(() => { document.documentElement.dataset.typing = 'on'; });
// --menu view|document|stats|palette: that popover open, at rest, with its first row selected.
// chrome.js has the same four names behind its ?open= hook, but that one fires on a timer 60 ms
// into the load, which is the same moment --active and --text are settling focus; opening from
// here instead puts the popover after them, so what is shot is not a race. The three menus open
// with nothing selected (openPanel.sel is -1), so one ArrowDown lights row 0 — the window's
// capturing keydown listener eats it, so the caret does not move; the palette renders with its
// first row already on. [chrome piece]
if (args.menu && args.menu !== true) {
  const opened = await p.evaluate((name) => {
    if (name === 'view') Writer.run('chrome.view');
    else if (name === 'document') Writer.run('chrome.doc');
    else if (name === 'palette') Writer.run('palette.open');
    // The stats menu is anchored to the pointer and has no command to run, so it is opened by the
    // click its bar listens for, at the x chrome.js's own hook picks.
    else if (name === 'stats') document.getElementById('stats-bar').dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: Math.round(innerWidth * 0.709) }));
    else return false;
    return document.documentElement.dataset.menu === 'on';
  }, args.menu);
  if (!opened) { console.error(`shoot: --menu ${args.menu} opened no popover (view|document|stats|palette)`); process.exit(2); }
  if (args.menu !== 'palette') await p.keyboard.press('ArrowDown');
}
// freeze caret visible & un-blinking for deterministic shots — but an unfocused caret is dimmed by
// #caret-layer.idle, and an inline opacity here would paint over the very thing --active off shoots.
await p.evaluate(([hide, idle]) => { const c = document.querySelector('#caret-layer .caret'); if (c) { c.classList.remove('blink'); c.style.opacity = hide ? '0' : (idle ? '' : '1'); } }, [!!args.nocaret, !active]);
// A shot has to be of the app at rest, or it is not the same shot twice: the hairline over the
// bottom bar fades in over .2s, and two frames after setText it is caught at whatever opacity it
// had reached — a different pixel row in every run. One wait is not enough, because the attribute
// that starts that transition is itself set in a rAF after the render, so a frame with nothing
// running can still be followed by one that starts something. Settle until a whole frame passes
// with no animation that ends still going, with a ceiling so one that never ends (the caret
// blink) cannot hang a shot.
await p.evaluate(async () => {
  const frame = () => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
  const ending = () => document.getAnimations().filter(a => a.playState === 'running' && Number.isFinite(a.effect?.getComputedTiming?.().endTime));
  const until = performance.now() + 3000;
  for (;;) {
    await frame();
    const going = ending();
    if (!going.length || performance.now() > until) break;
    await Promise.race([Promise.all(going.map(a => a.finished.catch(() => {}))), new Promise(r => setTimeout(r, 500))]);
  }
});
if (args.wait) await p.waitForTimeout(+args.wait);
const out = args.out || 'shots/shot.png'; fs.mkdirSync(path.dirname(out), { recursive: true });
await p.screenshot({ path: out, fullPage: !!args.full });
await b.close(); console.log('wrote', out, `${W}x${H}@${dpr}x`);
