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
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { LADDER_EM, byteToChar, emForStep, freezeReason, readStates, resolveStates, shootArgv, unservable } from './oracle.mjs';

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
  assert.deepEqual(type.map((s) => s.name), Object.keys(states.pieces.type));
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
  const files = Object.fromEntries(resolveStates(states, 'files').map((s) => [s.name, unservable(states.defaults, s.flags)]));
  assert.deepEqual(files.library, ['library', 'sidebar']);
  assert.deepEqual(files.search, ['library', 'search', 'sidebar']);
  for (const piece of ['type', 'page', 'markup', 'caret', 'theme', 'focus', 'chrome']) {
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
  // `caret/selection` is the one judged state that holds a selection, so it is the only one whose
  // `--select` conversion this can be read off. Since #168 it names a mac-native crop and this tool
  // passes it over rather than freezing it; what is measured here is the conversion, which is the
  // same for every state and is asked of this one because it is the only one that exercises it.
  assert.equal(flag('--caret'), '36');
  assert.equal(flag('--select'), '18,36');
  assert.equal(flag('--active'), 'on');
  // The state names step 5 and the shooter counts in pixels, so the one conversion this tool makes
  // besides the offsets shows up here: the ladder's em, fractional, and not the 21 it rounds to.
  assert.equal(flag('--size'), '21.33');
  assert.ok(!argv.includes('--typewriter'));
  assert.ok(!argv.includes('--nocaret'));

  const unfocused = shootArgv(ROOT, caret.unfocused, 'o.png', 'u');
  assert.equal(unfocused[unfocused.indexOf('--active') + 1], 'off');

  const empty = resolveStates(states, 'chrome').find((s) => s.name === 'empty').flags;
  const noText = shootArgv(ROOT, empty, 'o.png', 'u');
  assert.ok(!noText.includes('--text'), 'an empty Document is shot with no --text');
  assert.ok(!noText.includes('--caret'), 'and with no caret offset into a passage it does not have');

  const tw = resolveStates(states, 'focus').find((s) => s.name === 'dark-sentence-typewriter').flags;
  const dark = shootArgv(ROOT, tw, 'o.png', 'u');
  assert.ok(dark.includes('--typewriter'));
  assert.equal(dark[dark.indexOf('--theme') + 1], 'dark');
  assert.equal(dark[dark.indexOf('--focus') + 1], 'sentence');
  assert.equal(dark[dark.indexOf('--chrome') + 1], 'off');

  // The two chrome flags: absent at rest, and named exactly once when the state names them.
  const chrome = Object.fromEntries(resolveStates(states, 'chrome').map((s) => [s.name, s.flags]));
  const bars = shootArgv(ROOT, chrome.bars, 'o.png', 'u');
  assert.ok(!bars.includes('--typing'), 'the bars at rest are not the chrome stepped back');
  assert.ok(!bars.includes('--menu'), 'and no popover is open over them');
  assert.ok(shootArgv(ROOT, chrome.typing, 'o.png', 'u').includes('--typing'));
  const view = shootArgv(ROOT, chrome['view-menu'], 'o.png', 'u');
  assert.equal(view[view.indexOf('--menu') + 1], 'view');
  const palette = shootArgv(ROOT, chrome.palette, 'o.png', 'u');
  assert.equal(palette[palette.indexOf('--menu') + 1], 'palette');
});

// ---------- the ladder, on both sides of the port ----------
ok('the ems a step is converted with are the ladder the engine holds', () => {
  const source = fs.readFileSync(path.join(ROOT, 'quill-engine/src/typography.rs'), 'utf8');
  const ems = [...source.matchAll(/Rung \{ em: ([0-9.]+)/g)].map((m) => Number(m[1]));
  assert.ok(ems.length > 0, 'no ladder in quill-engine/src/typography.rs to check against');
  assert.deepEqual(ems, LADDER_EM, 'the ladder moved in the engine and the oracle was left behind');
  assert.equal(emForStep(5), 21.33, "the default step's em is the size iA Writer opens at");
  assert.throws(() => emForStep(LADDER_EM.length), /not on the type ladder/);
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

// ---------- the frozen shots are reproducible ----------
// Shooting one state twice has to give byte-identical files, and the committed shots say whether
// it still does without shooting anything: `caret`'s caret, `page`'s light and `theme`'s light are
// three Pieces asking for the same flags, so they are the same state shot three times, in three
// runs, minutes apart. The moment the shooter stops settling before the shutter they drift.
ok('states with the same flags were shot into the same bytes', () => {
  const bytes = new Map();
  const flags = new Map();
  let shot = 0;
  for (const piece of fs.readdirSync(path.join(ROOT, 'shots/oracle'), { withFileTypes: true }).filter((e) => e.isDirectory())) {
    for (const s of resolveStates(states, piece.name)) {
      const png = path.join(ROOT, 'shots/oracle', piece.name, `${s.name}.png`);
      if (!fs.existsSync(png)) continue;
      shot++;
      const key = JSON.stringify(s.flags);
      const digest = crypto.createHash('sha256').update(fs.readFileSync(png)).digest('hex');
      const where = `${piece.name}/${s.name}`;
      if (bytes.has(key)) assert.equal(digest, bytes.get(key), `${where} and ${flags.get(key)} are the same judged state but not the same bytes`);
      else { bytes.set(key, digest); flags.set(key, where); }
    }
  }
  // Counted rather than written down: a Piece frozen later adds states to both sides, and a number
  // kept here would only say what the last Piece to land happened to make it. Two assertions,
  // because "nothing was frozen" and "nothing shares its flags" are two different ways for this
  // check to be proving nothing, and one message cannot name both.
  assert.ok(shot > 0, 'no frozen shot was read at all — this check is proving nothing');
  assert.ok(bytes.size < shot, `no two of the ${shot} frozen states share their flags — this check is proving nothing`);
});

// ---------- the two answers that need no browser ----------
// The owner's line is the last one on stdout; everything the agent reads on the way to it is on
// stderr, so the two are kept apart here rather than interleaved.
function gate(...argv) {
  const env = typeof argv[argv.length - 1] === 'object' ? argv.pop() : {};
  try {
    return { code: 0, out: execFileSync(GATE, argv, { cwd: ROOT, encoding: 'utf8', env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'pipe'] }), err: '' };
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
  const r = gate('oracle', 'files');
  assert.equal(r.code, 1, r.err);
  assert.match(r.err, /state library names library, sidebar/);
  assert.match(r.err, /state search names library, search, sidebar/);
  assert.match(r.out.trim().split('\n').pop(), /^gate oracle files: fail/);
  assert.ok(!fs.existsSync(path.join(ROOT, 'shots/oracle/files')), 'a Piece is frozen whole or not at all');
});

ok('a state judged against a mac-native crop is not this tool\'s to freeze', () => {
  // The Design oracle's captures are committed under ref/ia/, and no browser driving legacy/ can
  // take one (ADR 0015): the state is passed over, and a Piece with no other kind of state is
  // finished before a server starts. Put in front of the whole command through QUILL_STATES,
  // because the first real such state belongs to the ticket that changes its row, not to this one.
  const file = path.join(os.tmpdir(), `quill-oracle-selftest-${process.pid}.json`);
  const states = JSON.parse(fs.readFileSync(path.join(ROOT, 'shots/oracle/states.json'), 'utf8'));
  states.pieces.type = {
    design: { opponent: { capture: 'mac-native-01-light-caret-midword.png', crop: [1500, 380, 100, 40], ours: 'centre' } },
  };
  fs.writeFileSync(file, JSON.stringify(states));
  try {
    const r = gate('oracle', 'type', { QUILL_STATES: file });
    assert.equal(r.code, 0, `${r.out}${r.err}`);
    assert.match(r.out.trim().split('\n').pop(), /^gate oracle type: nothing to freeze \(its one state names a mac-native crop\)/);
    // And it took nothing away from the Piece as it really stands: the sweep below reads the
    // fingerprint, and this run must not have removed the shots type is actually frozen at. Those
    // are duo and quattro alone since #165 moved `mono` to a Design oracle crop: a real
    // `gate oracle type` took `mono.png` away then, by the removal path below, because the state
    // had stopped being one this freezes — so naming it here would assert a file the command was
    // right to delete. The fixture in front of this case never reaches that path itself.
    for (const name of ['duo', 'quattro']) {
      assert.ok(fs.existsSync(path.join(ROOT, 'shots/oracle/type', `${name}.png`)), `${name}.png went missing`);
    }
    assert.ok(fs.existsSync(path.join(ROOT, 'shots/oracle/type/fingerprint.json')));
  } finally {
    fs.rmSync(file, { force: true });
  }
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
