// Does `tools/gate oracle` still read the judged states the way it says? — the oracle's own test.
//
//   node tools/oracle-selftest.mjs
//
// Everything here runs without a browser, a display or the document server: the seams under test
// are the ones that decide *what* is shot (which states a Piece has, where the caret goes, which
// flags cannot be served yet, and whether anything has moved since the last freeze), plus the
// answers the command gives before a browser is ever launched. The shooting itself is judged by
// the shots, which are committed.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { byteToChar, freezeReason, readStates, resolveStates, shootArgv, unservable } from './oracle.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const GATE = path.join(ROOT, 'tools', 'gate');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`oracle selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`oracle selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const states = readStates(ROOT);

// ---------- the judged states ----------
ok('a state is the defaults with its own overrides on top', () => {
  const type = resolveStates(states, 'type');
  assert.deepEqual(type.map((s) => s.name), ['duo', 'quattro', 'mono']);
  const duo = type[0].flags;
  assert.equal(duo.chrome, 'off');           // the state's own
  assert.equal(duo.font, 'duo');             // from the defaults
  assert.equal(duo.text, 'ref/sample.md');
  assert.equal(duo.caret, 403);
});

ok('a Piece with no judged states resolves to none, and an unknown Piece is an error', () => {
  assert.deepEqual(resolveStates(states, 'latency'), []);
  assert.throws(() => resolveStates(states, 'nosuch'), /nosuch/);
});

// ---------- the flags this tool cannot serve yet ----------
ok('a state may only name flags the defaults name', () => {
  const chrome = Object.fromEntries(resolveStates(states, 'chrome').map((s) => [s.name, unservable(states.defaults, s.flags)]));
  assert.deepEqual(chrome.bars, []);
  assert.deepEqual(chrome.typing, ['typing']);
  assert.deepEqual(chrome['view-menu'], ['menu']);
  const files = Object.fromEntries(resolveStates(states, 'files').map((s) => [s.name, unservable(states.defaults, s.flags)]));
  assert.deepEqual(files.library, ['library', 'sidebar']);
  assert.deepEqual(files.search, ['library', 'search', 'sidebar']);
  for (const piece of ['type', 'page', 'markup', 'caret', 'theme', 'focus']) {
    for (const s of resolveStates(states, piece)) assert.deepEqual(unservable(states.defaults, s.flags), [], `${piece}/${s.name}`);
  }
});

// ---------- bytes to characters ----------
ok('an offset is UTF-8 bytes on the way in and characters on the way out', () => {
  assert.equal(byteToChar('hello', 0), 0);
  assert.equal(byteToChar('hello', 5), 5);
  assert.equal(byteToChar('a — b', 5), 3);          // the em dash is three bytes and one character
  assert.throws(() => byteToChar('a — b', 3), /boundary/);   // one byte into it, which is no offset at all
  assert.throws(() => byteToChar('hello', 6), /past the end/);
  const passage = fs.readFileSync(path.join(ROOT, 'ref/sample.md'), 'utf8');
  assert.equal(passage.slice(byteToChar(passage, 153), byteToChar(passage, 171)), 'the way boats move');
});

// ---------- the command line each state is shot with ----------
ok('a state becomes the shoot.mjs flags that state means', () => {
  const caret = Object.fromEntries(resolveStates(states, 'caret').map((s) => [s.name, s.flags]));
  const argv = shootArgv(ROOT, caret.selection, 'shots/oracle/caret/selection.png', 'http://localhost:4173/');
  const flag = (name) => argv[argv.indexOf(name) + 1];
  assert.equal(flag('--out'), 'shots/oracle/caret/selection.png');
  assert.equal(flag('--url'), 'http://localhost:4173/');
  assert.equal(flag('--text'), 'ref/sample.md');
  assert.equal(flag('--w'), '1440');
  assert.equal(flag('--h'), '900');
  assert.equal(flag('--dpr'), '2');
  assert.equal(flag('--caret'), '171');
  assert.equal(flag('--select'), '153,171');
  assert.equal(flag('--active'), 'on');
  assert.ok(!argv.includes('--typewriter'));
  assert.ok(!argv.includes('--nocaret'));

  const unfocused = shootArgv(ROOT, caret.unfocused, 'o.png', 'u');
  assert.equal(unfocused[unfocused.indexOf('--active') + 1], 'off');

  const empty = resolveStates(states, 'page').find((s) => s.name === 'empty').flags;
  const noText = shootArgv(ROOT, empty, 'o.png', 'u');
  assert.ok(!noText.includes('--text'), 'an empty Document is shot with no --text');
  assert.ok(!noText.includes('--caret'), 'and with no caret offset into a passage it does not have');

  const tw = resolveStates(states, 'focus').find((s) => s.name === 'dark-sentence-typewriter').flags;
  const dark = shootArgv(ROOT, tw, 'o.png', 'u');
  assert.ok(dark.includes('--typewriter'));
  assert.equal(dark[dark.indexOf('--theme') + 1], 'dark');
  assert.equal(dark[dark.indexOf('--focus') + 1], 'sentence');
  assert.equal(dark[dark.indexOf('--chrome') + 1], 'off');
});

// ---------- what makes a frozen Piece stale ----------
ok('a freeze is stale when the app, the shooter, the passage or the states move under it', () => {
  const was = { app: { files: 9, sha256: 'aaaa' }, shoot: 'bbbb', passages: { 'ref/sample.md': 'eeee' }, states: { duo: { font: 'duo' } } };
  const same = JSON.parse(JSON.stringify(was));
  assert.equal(freezeReason(was, same, ['duo']), null);
  assert.match(freezeReason(null, same, ['duo']), /nothing frozen/);
  assert.match(freezeReason({ ...was, app: { files: 9, sha256: 'cccc' } }, same, ['duo']), /legacy\/app/);
  assert.match(freezeReason({ ...was, shoot: 'dddd' }, same, ['duo']), /shoot\.mjs/);
  assert.match(freezeReason(was, { ...same, passages: { 'ref/sample.md': 'ffff' } }, ['duo']), /passage/);
  assert.match(freezeReason(was, { ...same, states: { duo: { font: 'mono' } } }, ['duo']), /judged states/);
  assert.match(freezeReason(was, same, []), /shot is missing/);
});

// ---------- the two answers that need no browser ----------
// The owner's line is the last one on stdout; everything the agent reads on the way to it is on
// stderr, so the two are kept apart here rather than interleaved.
function gate(...argv) {
  try {
    return { code: 0, out: execFileSync(GATE, argv, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }), err: '' };
  } catch (e) {
    return { code: e.status, out: e.stdout || '', err: e.stderr || '' };
  }
}

ok('the latency Piece says it has no judged states, and says it without failing', () => {
  const r = gate('oracle', 'latency');
  assert.equal(r.code, 0, r.out);
  assert.match(r.out.trim().split('\n').pop(), /^gate oracle latency: no judged states/);
});

ok('a Piece whose states need flags this tool cannot serve names them and fails, shooting nothing', () => {
  const r = gate('oracle', 'chrome');
  assert.equal(r.code, 1, r.err);
  assert.match(r.err, /state typing names typing/);
  assert.match(r.err, /state view-menu names menu/);
  assert.match(r.out.trim().split('\n').pop(), /^gate oracle chrome: fail/);
  assert.ok(!fs.existsSync(path.join(ROOT, 'shots/oracle/chrome')), 'a Piece is frozen whole or not at all');
});

ok('a Piece nobody has judged states for is not a Piece', () => {
  const r = gate('oracle', 'nosuch');
  assert.equal(r.code, 2, r.err);
  assert.match(r.err, /nosuch/);
});

if (failures === 0) {
  console.log('oracle selftest: pass');
} else {
  console.log(`oracle selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
