#!/usr/bin/env node
// Would the typed condition still catch the caret that does not follow the writing? — `tools/gate
// keys`'s own test.
//
//   node tools/keys-selftest.mjs
//
// Every other Gate subcommand has one of these beside it, and this one has more to prove than most.
// `tools/gate keys` is the condition that exists because six critics and three judged states missed
// #108, so the question "does it actually go red on that defect?" cannot be left to the day it
// matters. The builds it is asked of are committed as pixels — `tools/keys-fixture/fixed-typing-
// {16,38}.png` from the build with `45d1434` and `broken-typing-{16,38}.png` from the one without
// it for #108's caret, `fixed-select-all.png` and `broken-select-all.png` for #146's selection, and
// `fill-select-all.png` and `fill-newline-held.png` from the build that fills the container for
// #168's, against `fixed-select-all.png` as the build that filled each row to its ink — so the
// whole assertion is exercised here with no window, no compositor and no keyboard, which is what
// lets `tools/gate check` run it.

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  INK_DARK, PAPER_DARK, decodePng, glyphAdvance, judgeBurst, judgeMove, judgeSelectionFill,
  judgeSelectionNewline, judgeSelectionRows, readBar, readSelectionRows, resolveScript,
} from './keys-assert.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const FIXTURE = path.join(ROOT, 'tools/keys-fixture');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases += 1;
  try {
    body();
    console.log(`keys selftest: ${name} ok`);
  } catch (e) {
    failures += 1;
    console.log(`keys selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const shot = (name) => decodePng(fs.readFileSync(path.join(FIXTURE, `${name}.png`)));

// ---------- a page, by hand ----------
//
// For the cases no fixture can show: a page with nothing on it, and a page with two blue things on
// it. Built as decoded pixels rather than as a PNG, because the assertion takes decoded pixels and
// an encoder here would only be a second thing to get wrong.
function page(w, h, paint) {
  const data = Buffer.alloc(w * h * 3, 0xf9);
  const put = (x, y, [r, g, b]) => {
    const i = (y * w + x) * 3;
    data[i] = r;
    data[i + 1] = g;
    data[i + 2] = b;
  };
  const fill = (x0, y0, x1, y1, colour) => {
    for (let y = y0; y <= y1; y += 1) for (let x = x0; x <= x1; x += 1) put(x, y, colour);
  };
  paint(fill);
  return { w, h, ch: 3, data };
}

const INK_PX = [0x1c, 0x1c, 0x1c];
const BAR_PX = [0x8a, 0xdb, 0xfc]; // the accent over paper, which is what the bar is on the glass

// ---------- the decoder ----------

ok('a grim shot decodes to the size its header claims', () => {
  const png = shot('fixed-typing-16');
  assert.equal(png.w, 2880);
  assert.equal(png.h, 1800);
  assert.equal(png.ch, 3);
  assert.equal(png.data.length, 2880 * 1800 * 3);
});

ok('something that is not a PNG says so rather than being measured', () => {
  assert.throws(() => decodePng(Buffer.alloc(64)), /not a PNG/);
});

// ---------- the green fixture: the build with 45d1434 ----------

ok('the fixed build puts the bar just right of the ink after 16 characters', () => {
  const v = judgeBurst(shot('fixed-typing-16'), { chars: 16 });
  assert.equal(v.pass, true, v.said);
  assert.equal(v.gap, 8);
});

ok('the fixed build puts the bar just right of the ink after 38 characters', () => {
  const v = judgeBurst(shot('fixed-typing-38'), { chars: 38 });
  assert.equal(v.pass, true, v.said);
  assert.equal(v.gap, 9);
});

ok('the fixed build moves the bar right between the two bursts', () => {
  const v = judgeMove(readBar(shot('fixed-typing-16')), readBar(shot('fixed-typing-38')));
  assert.equal(v.pass, true, v.said);
});

// ---------- the red fixture: the build without it, which is the whole point ----------

ok('the broken build fails after 16 characters, and the line names the column', () => {
  const v = judgeBurst(shot('broken-typing-16'), { chars: 16 });
  assert.equal(v.pass, false);
  assert.equal(v.read.bar.left, 675);
  assert.match(v.said, /bar\.left 675/);
  assert.match(v.said, /ink\.right 1053/);
});

ok('the broken build fails after 38 characters too', () => {
  const v = judgeBurst(shot('broken-typing-38'), { chars: 38 });
  assert.equal(v.pass, false);
  assert.equal(v.read.bar.left, 675);
});

ok('the broken build never moves the bar, and that is its own failure', () => {
  const before = readBar(shot('broken-typing-16'));
  const after = readBar(shot('broken-typing-38'));
  assert.equal(before.bar.left, after.bar.left, 'the defect is that these are the same number');
  const v = judgeMove(before, after);
  assert.equal(v.pass, false);
  assert.match(v.said, /675 then 675/);
});

// ---------- the two builds, matched ----------
//
// The pair above was shot at a glyph advance of 38.4 device pixels and the red pair at the judged
// size 20's 24.0, so a reader comparing them has two differences to hold at once. `fixed-size20-*`
// is the same script on the fixed build at the judged size: same ink, same advance, and the bar the
// only thing that moves. That is the A/B the condition exists to make.

ok('matched to the red pair, the fixed build differs in the bar and nothing else', () => {
  for (const [chars, ink] of [[16, 1053], [38, 1581]]) {
    const fixed = judgeBurst(shot(`fixed-size20-typing-${chars}`), { chars });
    const broken = judgeBurst(shot(`broken-typing-${chars}`), { chars });
    assert.equal(fixed.read.inkLeft, broken.read.inkLeft, `${chars}: the same ink starts both`);
    assert.equal(fixed.read.inkRight, ink, `${chars}: the same ink ends both`);
    assert.equal(broken.read.inkRight, ink);
    assert.equal(fixed.advance, broken.advance, `${chars}: one advance covers both`);
    assert.equal(fixed.pass, true, fixed.said);
    assert.equal(broken.pass, false, broken.said);
  }
});

ok('matched, the fixed build moves the bar and the broken one does not', () => {
  const fixed = judgeMove(readBar(shot('fixed-size20-typing-16')), readBar(shot('fixed-size20-typing-38')));
  assert.equal(fixed.pass, true, fixed.said);
  const broken = judgeMove(readBar(shot('broken-typing-16')), readBar(shot('broken-typing-38')));
  assert.equal(broken.pass, false, broken.said);
});

// ---------- the selection reaching the foot of a Document: #146 ----------
//
// The pair here is one build against itself: `fixed-select-all.png` is `tools/gate keys caret
// --shots` on the tip, `broken-select-all.png` is the same command in the same worktree with
// `27f5a21` reverted and nothing else changed. So the three rows are typed the same, the type is
// the same and the window is the same, and the bottom row is the only thing that differs.

ok('the fixed build paints every row of a selection that reaches the foot', () => {
  const v = judgeSelectionRows(shot('fixed-select-all'), { rows: 3 });
  assert.equal(v.pass, true, v.said);
  assert.deepEqual(v.read.bands, [
    { top: 150, bottom: 223, left: 622, right: 1616 },
    { top: 224, bottom: 297, left: 622, right: 744 },
    { top: 298, bottom: 371, left: 622, right: 877 },
  ]);
});

ok("the broken build leaves the bottom row bare, and the line names the count", () => {
  const v = judgeSelectionRows(shot('broken-select-all'), { rows: 3 });
  assert.equal(v.pass, false);
  assert.equal(v.read.bands.length, 2);
  assert.match(v.said, /2 row bands in the selection colour, expected 3/);
});

ok('matched, the two builds differ in the last row and in nothing else', () => {
  const fixed = readSelectionRows(shot('fixed-select-all'));
  const broken = readSelectionRows(shot('broken-select-all'));
  assert.deepEqual(broken.bands, fixed.bands.slice(0, 2), 'every row above the last is the same');
  assert.equal(fixed.bands.length - broken.bands.length, 1, 'the defect is one row, the bottom one');
});

ok('a row band is told from the next by where the fill ends, not by a gap between them', () => {
  // What the shots show, said as the invariant it rests on: the bands touch — 223 then 224, 297
  // then 298 — so nothing separates them but their right edges.
  const { bands } = readSelectionRows(shot('fixed-select-all'));
  for (let i = 1; i < bands.length; i += 1) {
    assert.equal(bands[i].top, bands[i - 1].bottom + 1, `bands ${i - 1} and ${i} touch`);
    assert.notEqual(bands[i].right, bands[i - 1].right, `bands ${i - 1} and ${i} end apart`);
  }
});

// ---------- the selection filling the container: #168 ----------
//
// A second matched pair, one build against itself again: `fill-select-all.png` is `tools/gate keys
// caret --shots` on the tip and `fixed-select-all.png` above is the same burst on the build that
// filled each row to its own ink. Same three rows, same type, same window, same three bands at the
// same heights — the reach along each row is the only thing that moved. `fill-newline-held.png` is
// the fourth burst, whose selection ends past a newline rather than past a glyph.

ok('the container-filling build spans the container on the row between the two ends', () => {
  const v = judgeSelectionFill(shot('fill-select-all'), { rows: 3 });
  assert.equal(v.pass, true, v.said);
  assert.deepEqual(v.read.bands, [
    { top: 150, bottom: 223, left: 622, right: 2437 },
    { top: 224, bottom: 297, left: 442, right: 2437 },
    { top: 298, bottom: 371, left: 442, right: 877 },
  ]);
  // The container is x 442 … 2438 of a 2880 px view: 442 either side of it.
  assert.equal(v.read.bands[1].left + v.read.bands[1].right + 1, 2880);
});

ok('the build that filled each row to its ink fails it, and the line names the container', () => {
  const v = judgeSelectionFill(shot('fixed-select-all'), { rows: 3 });
  assert.equal(v.pass, false);
  assert.match(v.said, /an interior row is filled x 622\.\.744/);
});

ok('matched, the two builds differ in how far each row reaches and in nothing else', () => {
  const fill = readSelectionRows(shot('fill-select-all')).bands;
  const ink = readSelectionRows(shot('fixed-select-all')).bands;
  assert.equal(fill.length, ink.length, 'the same three rows are painted by both');
  for (let i = 0; i < fill.length; i += 1) {
    assert.equal(fill[i].top, ink[i].top, `row ${i} stands at the same height`);
    assert.equal(fill[i].bottom, ink[i].bottom, `row ${i} is the same height`);
  }
  assert.equal(fill[0].left, ink[0].left, 'both start the first row at the anchor');
  assert.equal(fill[2].right, ink[2].right, 'both stop the last row at the focus');
  assert.ok(fill[1].left < ink[1].left, 'only the fill reaches into the left gutter');
  assert.ok(fill[1].right > ink[1].right, 'only the fill reaches the container’s right edge');
});

ok('a selection ending past a newline fills that row to the container’s right edge', () => {
  const v = judgeSelectionNewline(shot('fill-newline-held'), { rows: 2 });
  assert.equal(v.pass, true, v.said);
  assert.deepEqual(v.read.bands, [
    { top: 150, bottom: 223, left: 622, right: 2437 },
    { top: 224, bottom: 297, left: 442, right: 2437 },
  ]);
  // The empty row the trailing newline opens holds none of the selection, so it is a fill of no
  // width and there is no third band for it.
  assert.equal(v.read.bands.length, 2);
});

ok('the stub past the last glyph fails it, and the line names both rows', () => {
  const v = judgeSelectionNewline(shot('fixed-select-all'), { rows: 3 });
  assert.equal(v.pass, false);
  assert.match(v.said, /the row holding the newline is filled x 622\.\.877/);
});

ok('either container assertion without a row count is refused, not passed', () => {
  assert.match(judgeSelectionFill(shot('fill-select-all'), {}).said, /three or more/);
  assert.match(judgeSelectionFill(shot('fill-select-all'), { rows: 2 }).said, /three or more/);
  assert.match(judgeSelectionNewline(shot('fill-newline-held'), {}).said, /two or more/);
});

ok('a container off centre by the pixel its rounding can leave passes, and by more does not', () => {
  // The container is centred by rounding the view's spare width in half, so a view that leaves an
  // odd number over seats it half a logical pixel off centre — two device pixels at scale 2, and
  // no more than that. The whole range is asserted: dead centre, either edge of the play, and the
  // first pixel past it on each side.
  const rows = (right) => page(400, 60, (fill) => {
    fill(60, 0, right, 19, BAR_PX);
    fill(58, 20, right, 39, BAR_PX);
    fill(58, 40, 200, 59, BAR_PX);
  });
  // 58 + right + 1 against a 400 px view: 341 sums to 400 exactly, 339 and 343 to 398 and 402.
  for (const right of [339, 340, 341, 342, 343]) {
    assert.equal(judgeSelectionFill(rows(right), { rows: 3 }).pass, true, `right ${right}`);
  }
  for (const right of [338, 344]) {
    const v = judgeSelectionFill(rows(right), { rows: 3 });
    assert.equal(v.pass, false, `right ${right}`);
    assert.match(v.said, /sums to 400 within 2/);
  }
  // And a band that stops a whole cell short of the container is nowhere near it.
  assert.equal(judgeSelectionFill(rows(320), { rows: 3 }).pass, false);
});

ok('two rows filled to the same column are one band, which is why the script varies its rows', () => {
  const same = page(200, 60, (fill) => {
    fill(20, 10, 120, 29, BAR_PX);
    fill(20, 30, 120, 49, BAR_PX);
  });
  assert.equal(readSelectionRows(same).bands.length, 1);
  const varied = page(200, 60, (fill) => {
    fill(20, 10, 120, 29, BAR_PX);
    fill(20, 30, 90, 49, BAR_PX);
  });
  assert.equal(readSelectionRows(varied).bands.length, 2);
});

ok('a page with no selection colour on it says so rather than counting zero bands', () => {
  const bare = page(200, 60, (fill) => fill(20, 20, 100, 30, INK_PX));
  const v = judgeSelectionRows(bare, { rows: 3 });
  assert.equal(v.pass, false);
  assert.match(v.said, /nothing on the page is drawn in the selection colour/);
});

ok('a burst asserting selection-rows without saying how many rows is refused, not passed', () => {
  const v = judgeSelectionRows(shot('fixed-select-all'), {});
  assert.equal(v.pass, false);
  assert.match(v.said, /has to say how many rows it selects/);
});

// ---------- what is not a defect ----------

ok("a shot caught in the blink's dark half is reported as no bar, not as a bar in the wrong place", () => {
  const blank = page(200, 60, (fill) => fill(20, 20, 100, 30, INK_PX));
  const read = readBar(blank);
  assert.equal(read.bar, null);
  const v = judgeBurst(blank, { chars: 16, read });
  assert.equal(v.pass, false);
  assert.match(v.said, /no caret bar/);
});

ok('two runs of the accent are refused rather than measured across', () => {
  const two = page(200, 60, (fill) => {
    fill(20, 20, 60, 30, INK_PX);
    fill(70, 20, 73, 30, BAR_PX);
    fill(150, 20, 153, 30, BAR_PX);
  });
  const v = judgeBurst(two, { chars: 16 });
  assert.equal(v.pass, false);
  assert.match(v.said, /not one run/);
});

ok('a dark page is read the other way round, and the bar is the same blue on it', () => {
  // `Role::Accent` is one colour on both grounds, so only the ink test changes hands. Ink #cccccc
  // on paper #1a1a1a, which is what a script saying `--theme dark` would put on the glass.
  const dark = { w: 200, h: 60, ch: 3, data: Buffer.alloc(200 * 60 * 3, 0x1a) };
  const put = (x0, x1, [r, g, b]) => {
    for (let y = 20; y <= 30; y += 1) {
      for (let x = x0; x <= x1; x += 1) {
        const i = (y * 200 + x) * 3;
        dark.data[i] = r;
        dark.data[i + 1] = g;
        dark.data[i + 2] = b;
      }
    }
  };
  put(20, 100, [0xcc, 0xcc, 0xcc]);
  put(104, 107, BAR_PX);
  const colours = { ink: INK_DARK, paper: PAPER_DARK };
  const v = judgeBurst(dark, { chars: 16, colours });
  assert.equal(v.read.inkRight, 100, 'the pale glyphs are the ink here, not the dark ground');
  assert.equal(v.gap, 4);
  assert.equal(v.pass, true, v.said);
});

ok('a bar with no ink on its rows says so rather than measuring against nothing', () => {
  const bare = page(200, 60, (fill) => fill(70, 20, 73, 30, BAR_PX));
  const v = judgeBurst(bare, { chars: 16 });
  assert.equal(v.pass, false);
  assert.match(v.said, /no ink on the bar's rows/);
});

// ---------- the tolerance ----------

ok('the advance is derived from the shot and over-states the true one', () => {
  // The two fixtures are the same script at two sizes: 38.4 device pixels per glyph in the green
  // one, 24.0 in the red. A written-down constant would have been wrong for one of them.
  const green = glyphAdvance(readBar(shot('fixed-typing-38')), 38);
  const red = glyphAdvance(readBar(shot('broken-typing-38')), 38);
  assert.ok(green > 38.4 && green < 40, `green advance ${green}`);
  assert.ok(red > 24.0 && red < 26, `red advance ${red}`);
});

ok('too few characters to measure an advance from is said, not guessed at', () => {
  assert.equal(glyphAdvance({ inkLeft: 10, inkRight: 20 }, 1), null);
  const v = judgeBurst(shot('fixed-typing-16'), { chars: 1 });
  assert.equal(v.pass, false);
  assert.match(v.said, /too few/);
});

// ---------- the scripts ----------

const states = JSON.parse(fs.readFileSync(path.join(ROOT, 'shots/oracle/states.json'), 'utf8'));

ok("the caret's script is the four bursts the fixtures were taken with", () => {
  const script = resolveScript(states, 'caret');
  assert.deepEqual(script.bursts.map((b) => b.text),
    ['dfdfsdfsdfsdfsdf', 'fefefefefefsfesfesfesf', '\nffff\nssssssssss', 'aa\nbbbb\n']);
  assert.deepEqual(script.bursts.map((b) => b.chars), [16, 38, 54, 62]);
  // The rows the third burst selects, and the lengths that tell one band from the next: 38 from
  // the first two bursts, then 4 and 10. No two neighbours end in the same column.
  const rows = script.bursts[2].text.split('\n').slice(1);
  assert.deepEqual(rows.map((r) => r.length), [4, 10]);
  assert.equal(script.bursts[2].rows, 1 + rows.length);
  // Only `keys` can spell a chord, and a chord is what puts the selection on the page.
  assert.deepEqual(script.bursts[2].keys, [{ press: 'Control+a' }]);
  assert.equal(script.bursts[2].settle, 'still');
  // The fourth types over that selection and takes the Document down to two short rows and a
  // trailing newline, so the selection it then makes ends past a newline rather than past a glyph.
  // The empty row that newline opens holds none of the selection, which is why it is two rows.
  assert.equal(script.bursts[3].text.endsWith('\n'), true);
  assert.deepEqual(script.bursts[3].text.split('\n'), ['aa', 'bbbb', '']);
  assert.equal(script.bursts[3].rows, 2);
  assert.deepEqual(script.bursts[3].keys, [{ press: 'Control+a' }]);
  assert.equal(script.bursts[3].settle, 'still');
  assert.deepEqual(script.bursts.map((b) => b.assert), [
    ['bar-after-ink'],
    ['bar-after-ink'],
    ['selection-rows', 'selection-container-wide'],
    ['selection-newline-to-edge'],
  ]);
  // Live, on an empty Document, with the chrome off: the defaults with the script's state over them.
  assert.equal(script.flags.text, null);
  assert.equal(script.flags.chrome, 'off');
  assert.equal(script.flags.step, 5);
});

ok('a Piece with no keys script is told so by name', () => {
  assert.throws(() => resolveScript(states, 'type'), /no keys script for type/);
});

ok('the prose in the file is not offered as a Piece', () => {
  assert.throws(() => resolveScript(states, '_about'), /no keys script for _about/);
});

ok('a burst whose chars disagree with what it types is refused', () => {
  const wrong = { ...states, keys: { caret: { ...states.keys.caret } } };
  wrong.keys.caret.bursts = [{ name: 'a', text: 'abc', chars: 4, assert: [] }];
  assert.throws(() => resolveScript(wrong, 'caret'), /says chars 4, but 3/);
});

ok('an assertion the command does not have is refused by name', () => {
  const wrong = { ...states, keys: { caret: { ...states.keys.caret } } };
  wrong.keys.caret.bursts = [{ name: 'a', text: 'abc', chars: 3, assert: ['bar-does-a-jig'] }];
  assert.throws(() => resolveScript(wrong, 'caret'), /no assertion called bar-does-a-jig/);
});

ok('a settle rule the command does not have is refused by name, before a window opens', () => {
  const wrong = { ...states, keys: { caret: { ...states.keys.caret } } };
  wrong.keys.caret.bursts = [{ name: 'a', text: 'abc', chars: 3, settle: 'eventually' }];
  assert.throws(() => resolveScript(wrong, 'caret'), /settles eventually \(this command knows/);
});

ok('a burst that only presses a chord types no characters, and its chars say so', () => {
  const wrong = { ...states, keys: { caret: { ...states.keys.caret } } };
  wrong.keys.caret.bursts = [{ name: 'a', keys: [{ press: 'Control+a' }], chars: 0 }];
  assert.deepEqual(resolveScript(wrong, 'caret').bursts[0].chars, 0);
});

if (failures === 0) {
  console.log('keys selftest: pass');
} else {
  console.log(`keys selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
