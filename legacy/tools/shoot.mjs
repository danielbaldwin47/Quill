// Screenshot the app in a given state.
// node legacy/tools/shoot.mjs --out shots/x.png [--w 1440 --h 900 --dpr 2] [--theme light|dark|auto] [--font duo|quattro|mono]
//   [--size 18] [--focus off|sentence|paragraph] [--typewriter] [--chrome on|off] [--text file.md] [--caret N|end|"needle"]
//   [--scroll px|"needle"] [--mouse] (move mouse so chrome shows) [--nocaret] [--select a,b] [--url http://localhost:4173/] [--wait ms] [--full]
//   [--active on|off]  off blurs the input, so the caret and any selection are drawn in their unfocused state
//   [--typing]  the chrome stepped back, the way it is while the writer is typing
//   [--menu view|document|stats|palette]  that popover open, its first row selected
//   [--library dir] [--sidebar] [--search q]  seed the Library from a fixture folder, show it, narrow it
//   [--state seed.json]  merge {localStorageKey: value} into localStorage before load (e.g. a demo Library)
import { chromium } from 'playwright-core'; import fs from 'node:fs'; import path from 'node:path';
const args = {}; for (let i = 2; i < process.argv.length; i++) { const a = process.argv[i]; if (a.startsWith('--')) { const k = a.slice(2); const v = process.argv[i + 1]; if (v === undefined || v.startsWith('--')) args[k] = true; else { args[k] = v; i++; } } }
const W = +(args.w || 1440), H = +(args.h || 900), dpr = +(args.dpr || 2);
const url = args.url || process.env.QUILL_URL || 'http://localhost:4173/';
const settings = { theme: args.theme || 'light', font: args.font || 'duo', focus: args.focus || 'off', typewriter: !!args.typewriter, showChrome: (args.chrome || 'on') !== 'off' };
if (args.size) settings.fontSize = +args.size;
const text = args.text ? fs.readFileSync(args.text, 'utf8') : null;
// Where --caret puts it in a passage: an offset, a needle to land past, or the end — which is also
// where a bare --caret and no --caret at all put it. Asked of whichever text is being opened, since
// the Library seeds a document rather than typing one. [files piece]
const caretIn = (t) => {
  if (!args.caret || args.caret === true || args.caret === 'end') return t.length;
  if (/^\d+$/.test(args.caret)) return +args.caret;
  const i = t.indexOf(args.caret);
  return i >= 0 ? i + args.caret.length : t.length;
};
// --library <dir>: the fixture folder as the app's device Library. The folder location is a File
// System Access handle behind a picker and cannot be driven headless, so the browser location is
// the only one there is — and that one is localStorage, which is exactly what --state merges.
// The tree is read here rather than handed in, so the seed and the fixture cannot drift: the four
// extensions the Library lists, one level of subfolder (a Collection is one level, files.js
// `entries`), dot-entries left out because the hidden folder is there to be absent, and every
// mtime from the fixture's manifest.json — git carries no mtimes, so the disk's would sort the
// rows by whenever the checkout happened. [files piece]
const LIB_EXT = /\.(md|markdown|mdown|txt|text)$/i;
function libraryDocs(dir) {
  const mtimes = JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8')).mtimes;
  const docs = [];
  const read = (folder) => {
    const at = folder ? path.join(dir, folder) : dir;
    for (const e of fs.readdirSync(at, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      if (e.name.startsWith('.')) continue;
      const rel = folder ? `${folder}/${e.name}` : e.name;
      if (e.isDirectory()) { if (!folder) read(rel); continue; }
      if (!LIB_EXT.test(e.name)) continue;
      if (mtimes[rel] === undefined) { console.error(`shoot: --library ${dir}: ${rel} has no mtime in manifest.json, and the checkout's date is not a judged state`); process.exit(2); }
      docs.push({ id: rel, name: e.name, folder: folder || null, mtime: mtimes[rel] * 1000, text: fs.readFileSync(path.join(dir, rel), 'utf8'), caret: 0 });
    }
  };
  read(null);
  return docs;
}
let seed = args.state ? JSON.parse(fs.readFileSync(args.state, 'utf8')) : null;   // [files piece] --state seeds localStorage
if (args.sidebar !== undefined || args.search !== undefined || args.library !== undefined) {
  if (args.library === undefined || args.library === true) { console.error('shoot: --library takes the fixture folder to seed the Library from, and --sidebar and --search are that Library\'s'); process.exit(2); }
  const dir = String(args.library);
  const docs = libraryDocs(dir);
  // The open document is the one --text names, and --text must name one of these: the page and the
  // sidebar then show the same document, the way they do for a writer who clicked the row. It is
  // seeded rather than typed with setText, which is the point — setText raises `change`, and the
  // autosave 400 ms behind it restamps the open document's mtime with the wall clock and repaints
  // its row, so the shot would be of a different Library every run.
  const rel = text === null ? null : path.relative(dir, args.text);
  const open = rel === null ? [...docs].sort((a, b) => b.mtime - a.mtime)[0] : docs.find((d) => d.id === rel);
  if (open === undefined) { console.error(`shoot: --library ${dir} holds no ${rel ?? 'document'} for --text ${args.text ?? '(none)'} to open`); process.exit(2); }
  open.caret = caretIn(open.text);
  seed = {
    ...(seed || {}),
    'quill.lib': {
      open: !!args.sidebar, loc: 'device', sort: 'mtime', width: 368,
      q: args.search === undefined || args.search === true ? '' : String(args.search),
      openId: open.id, collapsed: {}, dirName: '', device: docs,
    },
  };
}
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--font-render-hinting=none', '--disable-lcd-text', '--hide-scrollbars'] });
const ctx = await b.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: dpr, colorScheme: settings.theme === 'dark' ? 'dark' : 'light', reducedMotion: 'no-preference' });
const p = await ctx.newPage();
await p.addInitScript(([s, seed]) => { localStorage.setItem('quill.settings', JSON.stringify(s)); localStorage.removeItem('quill.doc'); localStorage.removeItem('quill.doc.sel'); if (seed) for (const k in seed) localStorage.setItem(k, typeof seed[k] === 'string' ? seed[k] : JSON.stringify(seed[k])); }, [settings, seed]);
await p.goto(url, { waitUntil: 'load' });
await p.evaluate(async () => { await document.fonts.ready; });
// Under --library the passage is the Library's own document, seeded above and opened by the app's
// own boot; typing it in here would raise the `change` the autosave restamps its mtime behind.
if (text !== null && args.library === undefined) {
  await p.evaluate(([t, c]) => { Writer.setText(t, { caret: c }); Writer.el.input.focus(); }, [text, caretIn(text)]);
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
// A bare `--menu` parses as true, and a state shot with the popover it asked for missing is a
// state shot wrong, so an unnamed menu is as loud as an unknown one rather than quietly nothing.
if (args.menu !== undefined) {
  const want = args.menu === true ? '' : String(args.menu);
  const opened = await p.evaluate((name) => {
    if (name === 'view') Writer.run('chrome.view');
    else if (name === 'document') Writer.run('chrome.doc');
    else if (name === 'palette') Writer.run('palette.open');
    // The stats menu is anchored to the pointer and has no command to run, so it is opened by the
    // click its bar listens for, at the x chrome.js's own hook picks.
    else if (name === 'stats') document.getElementById('stats-bar').dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: Math.round(innerWidth * 0.709) }));
    else return false;
    return document.documentElement.dataset.menu === 'on';
  }, want);
  if (!opened) { console.error(`shoot: --menu ${want || '(unnamed)'} opened no popover (view|document|stats|palette)`); process.exit(2); }
  if (want !== 'palette') await p.keyboard.press('ArrowDown');
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
