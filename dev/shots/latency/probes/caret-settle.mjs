// Where the caret is on the frame the letter appears — and how many frames later it gets there.
// Owner: latency piece.  Round 4.
//
// The glyph is on the glass ~6 ms after the key. Until round 4 the caret was not: app/js/caret.js
// glided it to its new column over GLIDE_X = 62 ms whenever two keystrokes were more than
// SNAP_MS = 60 ms apart — which at any ordinary writing speed is EVERY keystroke — so the one mark
// a typist's eye is fixated on settled ~58 ms after the letter it is supposed to be beside.
//
// Method, per keystroke, black box (nothing but the caret's own client rect):
//   * every animation frame records [performance.now(), caret.left, caret.top];
//   * the caret's FINAL place for a keystroke is where it is in the last frame before the next one;
//   * `frames_to_settle` is the index of the first frame at or after the keydown at which the caret
//     is already there. 0 means the caret is beside the letter in the letter's own frame;
//   * `lag_px_on_the_glyph_frame` is how far short it is in the first frame after the keydown —
//     the visible distance between the caret and the letter you just typed, in device px and cells.
//   * `keydown_to_settled_ms` is that frame's rAF timestamp minus the keydown timestamp. rAF fires
//     at the start of the frame's rendering steps, so this is a floor on the settled caret's paint;
//     the frame it names is the one whose presentation the main bench reports as `to_present`.
//
// Usage: node dev/shots/latency/probes/caret-settle.mjs <url> [paces] [keys] [mix]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const doc = fs.readFileSync('dev/shots/latency/doc10k.md', 'utf8');
const url = process.argv[2] || 'http://localhost:4173/';
const paces = (process.argv[3] || '45,90,150,250').split(',').map(Number);
const KEYS = +(process.argv[4] || 80);
const mix = process.argv[5] || 'prose';

const SAMPLE = 'the quick brown fox jumps over a lazy dog and considers the pleasure of a daisy chain ';
const PROSE = 'The letter arrived on a Tuesday, unsigned, folded twice, and pushed under the door before anyone was awake. Marguerite read it standing up, still holding the kettle.\nNot today, she said, to nobody in particular; the room, which had heard worse, said nothing back.\n';

const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const out = { url, mix, keys: KEYS, by_pace: {} };
for (const pace of paces) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
  const p = await ctx.newPage();
  await p.addInitScript((t) => { localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', String(t.length)); localStorage.removeItem('quill.lib'); }, doc);
  await p.goto(url, { waitUntil: 'load' });
  await p.evaluate(() => document.fonts.ready);
  await p.evaluate(() => { const i = Writer.el.input; i.focus(); i.setSelectionRange(i.value.length, i.value.length); });
  // A 1px composited animation keeps the frame clock running, so this measures against a 60 Hz
  // cadence rather than against frames produced on demand.
  await p.addStyleTag({ content: '@keyframes __clk{from{opacity:.999}to{opacity:1}} #caret-layer::after{content:"";position:absolute;width:1px;height:1px;opacity:.999;animation:__clk 1s linear infinite;will-change:opacity}' });
  await p.evaluate(() => {
    window.__k = []; window.__f = [];
    const caret = document.querySelector('#caret-layer .caret');
    const em = parseFloat(getComputedStyle(Writer.el.mirror).fontSize) || 16;
    window.__em = em;
    addEventListener('keydown', (e) => window.__k.push({ t: e.timeStamp, key: e.key }), true);
    (function frame() {
      requestAnimationFrame(frame);
      const r = caret.getBoundingClientRect();
      window.__f.push([performance.now(), Math.round(r.left * 100) / 100, Math.round(r.top * 100) / 100]);
    })();
  });
  const text = mix === 'prose' ? PROSE : SAMPLE;
  for (let i = 0; i < KEYS; i++) {
    const ch = text[i % text.length];
    await p.keyboard.press(ch === ' ' ? 'Space' : ch === '\n' ? 'Enter' : ch);
    await p.waitForTimeout(pace);
  }
  await p.waitForTimeout(500);
  const { k, f, em } = await p.evaluate(() => ({ k: window.__k, f: window.__f, em: window.__em }));
  const settle = [], frames = [], lagpx = [], byKey = {};
  for (let i = 0; i < k.length - 1; i++) {
    const t0 = k[i].t, t1 = k[i + 1].t;
    const win = f.filter((x) => x[0] >= t0 && x[0] < t1);
    if (win.length < 2) continue;                       // not enough frames between keys to judge
    const fin = win[win.length - 1];
    const at = (x) => Math.abs(x[1] - fin[1]) < 0.6 && Math.abs(x[2] - fin[2]) < 0.6;
    const idx = win.findIndex(at);
    if (idx < 0) continue;
    settle.push(win[idx][0] - t0);
    frames.push(idx);
    const first = win[0];
    lagpx.push(Math.hypot(first[1] - fin[1], first[2] - fin[2]));
    (byKey[k[i].key === ' ' ? 'space' : k[i].key === 'Enter' ? 'Enter' : 'letter'] ||= []).push(idx);
  }
  const st = (v) => { if (!v.length) return null; const s = [...v].sort((a, c) => a - c); const q = (x) => +s[Math.min(s.length - 1, Math.ceil(x * s.length) - 1)].toFixed(2);
    return { n: s.length, mean: +(s.reduce((a, c) => a + c, 0) / s.length).toFixed(2), p50: q(.5), p90: q(.9), p99: q(.99), max: q(1) }; };
  out.by_pace[pace] = {
    pace_ms: pace, wpm: Math.round(60000 / (pace + 8) / 5),
    keystrokes_measured: settle.length,
    keydown_to_caret_settled_ms: st(settle),
    frames_the_caret_is_behind_the_glyph: st(frames),
    settled_in_the_glyphs_own_frame_pct: +(100 * frames.filter((x) => x === 0).length / Math.max(1, frames.length)).toFixed(2),
    lag_on_the_glyph_frame_px: st(lagpx),
    lag_on_the_glyph_frame_cells: st(lagpx.map((x) => x / (em * 0.6))),
    by_key_frames_behind: Object.fromEntries(Object.entries(byKey).map(([kk, v]) => [kk, st(v)])),
  };
  await ctx.close();
}
console.log(JSON.stringify(out, null, 2));
await b.close();
