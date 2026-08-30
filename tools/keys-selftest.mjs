#!/usr/bin/env node
// Would the typed condition still catch the caret that does not follow the writing? — `tools/gate
// keys`'s own test.
//
//   node tools/keys-selftest.mjs
//
// Every other Gate subcommand has one of these beside it, and this one has more to prove than most.
// `tools/gate keys` is the condition that exists because six critics and three judged states missed
// #108, so the question "does it actually go red on that defect?" cannot be left to the day it
// matters. The two builds are committed as pixels — `tools/keys-fixture/fixed-typing-{16,38}.png`
// from the build with `45d1434`, `broken-typing-{16,38}.png` from the build without it — so the
// whole assertion is exercised here with no window, no compositor and no keyboard, which is what
// lets `tools/gate check` run it.

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { decodePng, glyphAdvance, judgeBurst, judgeMove, readBar } from './keys-assert.mjs';
import { resolveScript } from './keys.mjs';

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

ok("the caret's script is the two bursts the fixture was taken with", () => {
  const script = resolveScript(states, 'caret');
  assert.deepEqual(script.bursts.map((b) => b.text), ['dfdfsdfsdfsdfsdf', 'fefefefefefsfesfesfesf']);
  assert.deepEqual(script.bursts.map((b) => b.chars), [16, 38]);
  // Live, on an empty Document, with the chrome off: the defaults with the script's state over them.
  assert.equal(script.flags.text, null);
  assert.equal(script.flags.chrome, 'off');
  assert.equal(script.flags.size, 20);
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

if (failures === 0) {
  console.log('keys selftest: pass');
} else {
  console.log(`keys selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
