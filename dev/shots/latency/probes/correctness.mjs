// Is anything the reader can SEE ever stale?
//
// Round 2 deferred the re-tokenising below an edit: AHEAD lines are filled inside the keystroke
// and the rest in animation frames. Round 2's version of this probe located the fence with
// `lines().findIndex(l => l.startsWith('```js'))`, which finds the document's OWN fenced block near
// the top, not the one it just typed — so it never checked a single deferred line. This one holds
// the index it typed at, checks the whole visible range rather than one line 400 below the fold,
// and scrolls into the not-yet-caught-up range mid-flight, which is the case that was argued
// rather than measured.
//
//   node dev/shots/latency/probes/correctness.mjs [url] [doc]
import { chromium } from 'playwright-core'; import fs from 'node:fs';
const url = process.argv[2] || 'http://localhost:4173/';
const doc = fs.readFileSync(process.argv[3] || 'dev/shots/latency/doc52k.md', 'utf8');
const b = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true });
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
const p = await ctx.newPage();
await p.addInitScript((t) => { localStorage.removeItem('quill.lib'); localStorage.setItem('quill.doc', t); localStorage.setItem('quill.doc.sel', '0'); }, doc);
await p.goto(url, { waitUntil: 'load' });
await p.evaluate(() => document.fonts.ready);
const res = await p.evaluate(async () => {
  const out = { fail: [] };
  const i = Writer.el.input, M = Writer.el.mirror, sc = Writer.el.scroller;
  const mirrorText = () => Array.from(M.children).map((e) => e.textContent.replace(/ /g, ' ')).join('\n');
  const check = (name, cond) => { out[name] = cond; if (!cond) out.fail.push(name); };
  const isCode = (n) => { const el = Writer.lineEl(n); return !!el && /\bl-code\b/.test(el.className); };
  const visibleAllCode = () => { const [a, b] = Writer.visibleRange(); let bad = []; for (let n = Math.max(a, idx + 1); n <= b; n++) if (!isCode(n)) bad.push(n); return bad; };

  // A tilde fence, because the document contains its own ``` block: a ``` opener is closed by
  // that block's closing fence 18 lines later and only 14 lines are ever deferred, which is the
  // easy case. `~~~` is closed by nothing, so every line to the end of the document changes —
  // the worst thing one keystroke can ask a Markdown editor to do.
  const FENCE = '~~~js\n';
  out.lines = Writer.lineCount();
  out.ahead_lines_for_this_viewport = Writer.aheadLines();
  out.viewport_px = sc.clientHeight;
  check('mirror_matches_textarea_at_boot', mirrorText() === i.value);

  // --- 1. open a fence in the middle, through the real input path ---------------------------
  const at = i.value.indexOf('\n', Math.floor(i.value.length / 2)) + 1;
  i.focus(); i.setSelectionRange(at, at);
  for (const ch of FENCE) document.execCommand('insertText', false, ch);
  const idx = Writer.offsetToPos(at).line;                 // the line WE typed, held, not searched
  out.fence_line_index = idx;
  out.fence_line_text = Writer.lines()[idx];
  check('fence_line_is_a_fence', out.fence_line_text === '~~~js');
  check('fence_line_tokenised_as_code', isCode(idx));
  check('mirror_matches_textarea_right_after_fence', mirrorText() === i.value);

  // --- 2. nothing on screen is stale in the very same task as the keystroke -----------------
  const bad0 = visibleAllCode();
  out.visible_range_after_fence = Writer.visibleRange();
  out.stale_visible_lines_immediately = bad0.length;
  check('nothing_visible_is_stale_immediately', bad0.length === 0);
  check('line_1_below_is_code_immediately', isCode(idx + 1));
  check('line_AHEAD_minus_1_below_is_code_immediately', isCode(idx + Writer.aheadLines() - 1));
  out.line_2000_below_immediately = Writer.lineEl(idx + 2000) ? Writer.lineEl(idx + 2000).className : null;

  // --- 3. scroll into the not-yet-caught-up range MID-FLIGHT --------------------------------
  // No sleep: this is the frame after the keystroke, with thousands of lines still pending.
  const target = Writer.lineEl(idx + 1200);
  out.scrolled_to_line = idx + 1200;
  out.was_stale_before_scroll = target ? !/\bl-code\b/.test(target.className) : null;
  sc.scrollTop = target.offsetTop - 100;
  sc.dispatchEvent(new Event('scroll'));                    // the same event the browser fires
  const badScroll = visibleAllCode();
  out.visible_range_after_scroll = Writer.visibleRange();
  out.stale_visible_lines_after_scroll = badScroll.length;
  check('nothing_visible_is_stale_after_scrolling_into_the_catchup_range', badScroll.length === 0);

  // --- 4. the catch-up finishes, and finishes correctly -------------------------------------
  const t0 = performance.now();
  let frames = 0; const frameMs = [];
  out.lines_pending_after_the_keystroke = Writer.pendingLines();
  while (performance.now() - t0 < 4000 && Writer.pendingLines() > 0) {
    const before = Writer.pendingLines();
    await new Promise((r) => requestAnimationFrame(r));
    if (Writer.pendingLines() !== before) frames++;
  }
  out.catchup_ms = Math.round(performance.now() - t0);
  out.catchup_frames = frames;
  out.lines_deferred = Writer.lineCount() - (idx + 1) - Writer.aheadLines();

  const stale = []; for (let n = idx + 1; n < Writer.lineCount(); n++) if (!isCode(n)) stale.push(n);
  out.stale_lines_after_catchup = stale.length;
  out.stale_line_index = stale.slice(0, 5); out.stale_of = Writer.lineCount(); out.stale_line_texts = stale.slice(0, 5).map((n) => JSON.stringify(Writer.lines()[n].slice(0, 40)) + ' cls=' + (Writer.lineEl(n)||{}).className);
  check('every_line_below_the_fence_is_code_after_catchup', stale.length === 0);

  check('mirror_matches_textarea_after_catchup', mirrorText() === i.value);

  // --- 5. and it all comes back when the fence is deleted -----------------------------------
  i.setSelectionRange(at, at + FENCE.length);
  document.execCommand('delete');
  // One animation frame — which runs BEFORE that frame's style/layout/paint, so this is exactly
  // what the reader's next frame will show, not "some time later".
  await new Promise((r) => requestAnimationFrame(r));
  const bad1 = (() => { const [a, b] = Writer.visibleRange(); let n2 = 0; for (let n = a; n <= b; n++) if (isCode(n)) n2++; return n2; })();
  out.code_lines_still_visible_in_the_first_frame_after_delete = bad1;
  out.viewport_is_far_from_the_edit_lines = Writer.visibleRange()[0] - idx;
  check('nothing_visible_is_stale_in_the_first_frame_after_delete', bad1 === 0);
  await new Promise((r) => setTimeout(r, 800));
  let left = 0; for (let n = idx + 1; n < Writer.lineCount(); n++) if (isCode(n)) left++;
  out.code_lines_left_after_delete_catchup = left;
  check('document_is_back_to_prose_after_delete', left <= 3);   // the document's own fenced block
  check('mirror_matches_textarea_after_delete', mirrorText() === i.value);
  check('heights_match', Math.abs(i.offsetHeight - M.offsetHeight) < 2);
  out.ok = out.fail.length === 0;
  return out;
});
console.log(JSON.stringify(res, null, 1));
await b.close();
process.exit(res.ok ? 0 : 1);
