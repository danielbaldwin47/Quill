// Does `tools/gate judge` still judge the way it says? — the judging pipeline's own test.
//
//   node tools/judge-selftest.mjs
//
// No compositor, no window, no critic and no money spent. What is under test is everything that
// decides *what* is judged and *how the answer is read*: the command line a judged state opens ours
// with, the rules that pin it, the toplevel the capture is of, the blindness of the pair, the
// critic's answer, what makes a round, which rounds count as native wins, and the three answers the
// command gives before it touches anything. The shooting and the judging themselves are judged by
// the shots and the rounds, which are committed.
//
// It is the second half of what `tools/oracle-selftest.mjs` is to the oracle: between them, both
// sides of a blind pair are checked without a browser and without a window.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { pair, pairDir, reveal } from './blind.mjs';
import { CAPTURES, cropPng, encodePng, resolveOpponent } from './crop.mjs';
import { APP_ID, appeared, classPattern, launchEnv, parseToplevels, pngSize, quillArgv, rulesLua } from './harness.mjs';
import { criticAnswer, criticPrompt, opponentOf, oursArgv, refusedFlag } from './judge.mjs';
import { decodePng } from './keys-assert.mjs';
import { readStates, resolveStates, unservable } from './oracle.mjs';
import { regimes } from './regimes.mjs';
import { OPPONENTS, decisive, nextRound, opponentName, round, wonBefore } from './rounds.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const GATE = path.join(ROOT, 'tools', 'gate');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`judge selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`judge selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const states = readStates(ROOT);
const flagsOf = (piece) => Object.fromEntries(resolveStates(states, piece).map((s) => [s.name, s.flags]));

// ---------- the command line a judged state opens ours with ----------
ok('a state becomes the native flags that state means', () => {
  const caret = flagsOf('caret');
  const argv = quillArgv(ROOT, caret.selection);
  const flag = (name) => argv[argv.indexOf(name) + 1];
  assert.ok(argv.includes('--deterministic'), 'every judged shot is of a determined frame');
  assert.equal(flag('--w'), '1440');
  assert.equal(flag('--h'), '900');
  assert.equal(flag('--theme'), 'light');
  assert.equal(flag('--font'), 'duo');
  assert.equal(flag('--size'), '20');
  assert.equal(flag('--focus'), 'off');
  // The caret Piece is judged bare (#139), so its states override the defaults' chrome; that
  // override reaching the command line is the half of this case the defaults cannot show.
  assert.equal(flag('--chrome'), 'off');
  assert.equal(flag('--text'), path.join(ROOT, 'ref/sample.md'));
  // Bytes on the way in and bytes on the way out: the native flags take the form states.json
  // writes, which is what the oracle has to convert away from and this does not.
  assert.equal(flag('--caret'), '171');
  assert.equal(flag('--select'), '153,171');
  assert.ok(!argv.includes('--typewriter'));
  assert.ok(!argv.includes('--nocaret'));
  // Neither of the two flags that are not the app's ever reaches its command line.
  assert.ok(!argv.includes('--scale') && !argv.includes('--active'));

  const empty = quillArgv(ROOT, flagsOf('page').empty);
  assert.ok(!empty.includes('--text'), 'an empty Document is opened with no passage');
  assert.ok(!empty.includes('--caret'), 'and with no offset into one it does not have');
  assert.equal(empty[empty.indexOf('--w') + 1], '1440');

  const tw = quillArgv(ROOT, flagsOf('focus')['dark-sentence-typewriter']);
  assert.ok(tw.includes('--typewriter'));
  assert.equal(tw[tw.indexOf('--theme') + 1], 'dark');
  assert.equal(tw[tw.indexOf('--focus') + 1], 'sentence');

  assert.equal(quillArgv(ROOT, flagsOf('page').narrow)[quillArgv(ROOT, flagsOf('page').narrow).indexOf('--w') + 1], '960');

  // The two chrome states the bars alone do not reach. `bars` is the same Piece with neither flag,
  // so the flags are the state's and not the Piece's.
  const chrome = flagsOf('chrome');
  const bars = quillArgv(ROOT, chrome.bars);
  assert.ok(!bars.includes('--typing') && !bars.includes('--menu'), 'the bars at rest name neither');
  assert.ok(quillArgv(ROOT, chrome.typing).includes('--typing'));
  const view = quillArgv(ROOT, chrome['view-menu']);
  assert.equal(view[view.indexOf('--menu') + 1], 'view');
  assert.ok(!view.includes('--typing'), 'an open menu is not the chrome stepped back');
  const palette = quillArgv(ROOT, chrome.palette);
  assert.equal(palette[palette.indexOf('--menu') + 1], 'palette');
});

ok('the launch environment is the one the research pinned', () => {
  const env = launchEnv({ GDK_SCALE: '2', PATH: '/usr/bin', GSK_RENDERER: 'cairo' });
  assert.ok(!('GDK_SCALE' in env), 'GDK_SCALE would force a buffer the compositor then rescales');
  assert.equal(env.GSK_RENDERER, 'gl', 'cairo differs from gl by thousands of pixels on the same window');
  assert.equal(env.GDK_BACKEND, 'wayland');
  assert.equal(env.GTK_A11Y, 'none');
  assert.equal(env.PATH, '/usr/bin', 'and nothing else about the environment is touched');
});

// ---------- the rules that pin the window ----------
ok('the class match is anchored and its dots are escaped, in Lua', () => {
  assert.equal(classPattern('io.github.danielbaldwin47.Quill'), '^io\\\\.github\\\\.danielbaldwin47\\\\.Quill$');
  // Which is the four characters `\\.` in the file the compositor reads, i.e. one escaped dot.
  assert.match(rulesLua(APP_ID, { workspace: 5, w: 1440, h: 900 }), /local C = "\^io\\\\\.github/);
});

ok('the rules carry this state\'s size, and hold focus off only when the state asks', () => {
  const focused = rulesLua(APP_ID, { workspace: 7, w: 960, h: 900, initialFocus: true });
  assert.match(focused, /workspace = "7 silent"/);
  assert.match(focused, /size = "960 900"/);
  assert.match(focused, /move = "80 50"/);
  assert.match(focused, /float = true/);
  // Omarchy composites every window at 98.5%; a judged shot is of an app, not of a mood.
  assert.match(focused, /tag = "-default-opacity"/);
  assert.match(focused, /opacity = "1\.0 1\.0"/);
  for (const off of ['no_anim = true', 'border_size = 0', 'rounding = 0', 'no_shadow = true', 'no_blur = true', 'no_dim = true']) {
    assert.ok(focused.includes(off), `${off} is missing`);
  }
  assert.ok(!focused.includes('no_initial_focus'), 'a focused state takes focus as it maps');
  assert.match(rulesLua(APP_ID, { workspace: 7, w: 1440, h: 900, initialFocus: false }), /no_initial_focus = true/);
  // Every rule is in the one table the teardown empties; a rule outside it could not be taken off.
  assert.equal(focused.match(/rule\(\{/g).length, 5);
});

// ---------- which toplevel the capture is of ----------
const TRACE = `
[03:14:37.673604] {Default Queue} ext_foreign_toplevel_handle_v1#4278190080.identifier("180003c0")
[03:14:37.673609] {Default Queue} ext_foreign_toplevel_handle_v1#4278190080.app_id("foot")
[03:14:37.673612] {Default Queue} ext_foreign_toplevel_handle_v1#4278190080.title("3 awaiting input")
[03:14:37.673623] {Default Queue} ext_foreign_toplevel_handle_v1#4278190081.identifier("18000370")
[03:14:37.673627] {Default Queue} ext_foreign_toplevel_handle_v1#4278190081.app_id("${APP_ID}")
[03:14:37.673631] {Default Queue} ext_foreign_toplevel_handle_v1#4278190081.title("sample.md")
[03:14:37.673712] {Default Queue} zxdg_output_v1#3.done()
`;

ok('a toplevel list is read out of grim narrating its own protocol', () => {
  const seen = parseToplevels(TRACE);
  assert.deepEqual(seen.map((t) => t.id), ['180003c0', '18000370']);
  assert.equal(seen[1].appId, APP_ID);
  assert.equal(seen[1].title, 'sample.md');
  // The colouring is grim's and could change tomorrow; the parse must not depend on it.
  assert.deepEqual(parseToplevels(TRACE.replace(/ext_foreign/g, '[34mext_foreign')).map((t) => t.id), ['180003c0', '18000370']);
});

ok('ours is the toplevel that appeared, not the one that matches', () => {
  const before = parseToplevels(TRACE);
  const after = [...before, { id: 'deadbeef', appId: APP_ID, title: 'sample.md' }];
  // The owner's own Quill and the harness's parking window are both of our class and were both
  // there first; only watching one appear can tell them from the one just launched.
  assert.equal(appeared(before, after, APP_ID).id, 'deadbeef');
  assert.equal(appeared(before, before, APP_ID), null);
  assert.throws(() => appeared(before, [...after, { id: 'cafe', appId: APP_ID }], APP_ID), /cannot tell which/);
});

// ---------- the shot's own size ----------
ok('a PNG says how big it is, and something that is not one says so', () => {
  const png = fs.readFileSync(path.join(ROOT, 'shots/oracle/type/duo.png'));
  assert.deepEqual(pngSize(png), { w: 2880, h: 1800 }, '1440x900 at scale 2 is what every judged state is');
  assert.deepEqual(pngSize(fs.readFileSync(path.join(ROOT, 'shots/oracle/page/narrow.png'))), { w: 1920, h: 1800 });
  assert.throws(() => pngSize(Buffer.alloc(64)), /not a PNG/);
});

// ---------- the blindness of the pair ----------
function inTemp(body) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-judge-selftest-'));
  const was = process.cwd();
  const keys = process.env.BLIND_KEY_DIR;
  process.env.BLIND_KEY_DIR = path.join(dir, 'keys');
  process.chdir(dir);
  try { return body(dir); }
  finally {
    process.chdir(was);
    if (keys === undefined) delete process.env.BLIND_KEY_DIR; else process.env.BLIND_KEY_DIR = keys;
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

ok('a pair is two letters and nothing else, and the key is not in the repository', () => {
  inTemp((dir) => {
    fs.writeFileSync(path.join(dir, 'ours.png'), 'OURS');
    fs.writeFileSync(path.join(dir, 'theirs.png'), 'THEIRS');
    const made = pair('type', 'duo', 'ours.png', 'theirs.png');
    assert.equal(made.dir, path.join('shots/blind', 'type', 'duo'));
    assert.deepEqual(fs.readdirSync(made.dir).sort(), ['A.png', 'B.png']);
    assert.ok(!fs.existsSync(path.join(dir, 'shots/blind/type/duo/key.json')));
    const key = reveal('type', 'duo');
    assert.equal(fs.readFileSync(path.join(made.dir, `${key.ours}.png`), 'utf8'), 'OURS');
    assert.equal(fs.readFileSync(path.join(made.dir, key.ours === 'A' ? 'B.png' : 'A.png'), 'utf8'), 'THEIRS');
    // A pair from a state that has since been renamed must not be left beside this one.
    fs.writeFileSync(path.join(made.dir, 'stale.png'), 'x');
    assert.deepEqual(fs.readdirSync(pair('type', 'duo', 'ours.png', 'theirs.png').dir).sort(), ['A.png', 'B.png']);
  });
});

ok('pairing one image with itself leaves nothing to tell A from B', () => {
  inTemp((dir) => {
    fs.writeFileSync(path.join(dir, 'same.png'), 'SAME');
    const made = pair('theme', 'dark', 'same.png', 'same.png');
    const entries = fs.readdirSync(made.dir).sort();
    assert.deepEqual(entries, ['A.png', 'B.png']);
    const a = fs.readFileSync(made.A);
    const b = fs.readFileSync(made.B);
    assert.ok(a.equals(b), 'the bytes are the same');
    assert.equal(fs.statSync(made.A).size, fs.statSync(made.B).size);
  });
});

ok('which letter is ours is a coin flip, not a habit', () => {
  inTemp((dir) => {
    fs.writeFileSync(path.join(dir, 'o.png'), 'O');
    fs.writeFileSync(path.join(dir, 't.png'), 'T');
    const letters = new Set();
    for (let i = 0; i < 40; i++) letters.add(pair('page', 'light', 'o.png', 't.png').ours);
    assert.deepEqual([...letters].sort(), ['A', 'B']);
  });
});

// ---------- the critic's prompt and the critic's answer ----------
ok('the critic is sent the prompt and not the file it lives in', () => {
  const template = fs.readFileSync(path.join(ROOT, 'tools/critic.md'), 'utf8');
  const prompt = criticPrompt(template, { title: 'The type', judge: 'the typography only' });
  assert.ok(!prompt.includes('{{'), 'every field is filled');
  assert.ok(prompt.includes('The type') && prompt.includes('the typography only'));
  assert.ok(!prompt.includes('tools/gate judge'), 'nothing above the --- reaches the critic');
  assert.ok(!prompt.includes('Quill') && !prompt.includes('oracle'), 'and nothing in it names either app');
  // The judged state is a tell — whichever image does not show it is the one that failed it — so the
  // prompt must not name one.
  for (const tell of ['light', 'dark', 'duo', 'quattro', 'mono', 'typewriter']) {
    assert.doesNotMatch(prompt, new RegExp(`\\b${tell}\\b`, 'i'), `the prompt names ${tell}, which says which image was asked for what`);
  }
  assert.throws(() => criticPrompt('no rule here', {}), /no prompt under/);
  assert.throws(() => criticPrompt('x\n---\njudge {{title}} at {{nosuch}}', { title: 't', judge: 'j' }), /\{\{nosuch\}\}/);
});

ok('the critic\'s answer is the last one it wrote, and a spoilt one is refused', () => {
  const said = 'Here is the shape:\n```json\n{"pick":"A"}\n```\nAnd my answer:\n'
    + '```json\n{"pick":"B","margin":"slight","gapA":"a","gapB":"b","verdict":"v","secondary":["s",3]}\n```\n';
  const answer = criticAnswer(said);
  assert.equal(answer.pick, 'B');
  assert.equal(answer.margin, 'slight');
  assert.deepEqual(answer.secondary, ['s']);
  assert.equal(answer.sameViewport, true, 'silence about the viewport is not a complaint about it');
  assert.equal(criticAnswer('```json\n{"pick":"A","margin":"nonsense","gapA":"a","gapB":"b","verdict":"v"}\n```').margin, 'clear');
  assert.throws(() => criticAnswer('I could not decide.'), /not JSON/);
  assert.throws(() => criticAnswer('```json\n{"pick":"neither","gapA":"a","gapB":"b","verdict":"v"}\n```'), /neither A nor B/);
  assert.throws(() => criticAnswer('```json\n{"pick":"A","gapA":"a","gapB":"","verdict":"v"}\n```'), /no gapB/);
});

// ---------- the round ----------
const JUDGED = [
  { name: 'duo', ours: 'shots/type/r2-duo-ours.png', theirs: 'shots/oracle/type/duo.png', winner: 'ours', margin: 'clear', gap: 'ours-gap-1', gapTheirs: 'theirs-gap-1', verdict: 'v1', secondary: ['one'] },
  { name: 'quattro', ours: 'shots/type/r2-quattro-ours.png', theirs: 'shots/oracle/type/quattro.png', winner: 'theirs', margin: 'slight', gap: 'ours-gap-2', gapTheirs: 'theirs-gap-2', verdict: 'v2', secondary: ['two'] },
  { name: 'mono', ours: 'shots/type/r2-mono-ours.png', theirs: 'shots/oracle/type/mono.png', winner: 'theirs', margin: 'clear', gap: 'ours-gap-3', gapTheirs: 'theirs-gap-3', verdict: 'v3', secondary: [] },
];

ok('a Piece is ours only when every state is, and the round speaks for the state that decided it', () => {
  assert.equal(decisive(JUDGED).name, 'quattro', 'the first one lost');
  assert.equal(decisive(JUDGED.map((s) => ({ ...s, winner: 'ours' }))).name, 'duo', 'and the first of all when none was');

  const lost = round({ piece: 'type', number: 2, judged: JUDGED, opponent: 'oracle', build: { git: 'abc', binary: 'def' }, oracle: 'shots/oracle/type/', note: 'n', at: 'T' });
  assert.equal(lost.winner, 'theirs');
  assert.equal(lost.gap, 'ours-gap-2');
  assert.equal(lost.gapTheirs, 'theirs-gap-2');
  assert.equal(lost.verdict, 'v2');
  assert.equal(lost.oursShot, 'shots/type/r2-quattro-ours.png');
  assert.equal(lost.builderNote, 'n');
  assert.equal(lost.opponent, 'oracle');
  assert.deepEqual(lost.build, { git: 'abc', binary: 'def' });
  assert.deepEqual(lost.states.map((s) => s.name), ['duo', 'quattro', 'mono']);
  // tools/progress.mjs reads a round by these names and has to go on doing so.
  for (const key of ['piece', 'round', 'winner', 'gap', 'verdict', 'oursShot', 'theirsShot', 'builderNote', 'at']) {
    assert.ok(key in lost, `a round with no ${key} is not a round progress.mjs can draw`);
  }
  assert.equal(round({ piece: 'type', number: 2, judged: JUDGED.map((s) => ({ ...s, winner: 'ours' })), opponent: 'oracle', build: {}, oracle: '', note: '', at: 'T' }).winner, 'ours');
});

ok('a Piece once won is never lost — but only a Piece that was won against a native opponent', () => {
  const legacy = [{ piece: 'type', round: 1, winner: 'ours' }];
  assert.equal(nextRound(legacy), 2);
  assert.equal(wonBefore(legacy), null, 'the gauntlet rounds are the reference era, not a native win');
  assert.equal(wonBefore([...legacy, { piece: 'type', round: 2, winner: 'theirs', opponent: 'oracle' }]), null);
  assert.equal(wonBefore([...legacy, { piece: 'type', round: 2, winner: 'ours', opponent: 'oracle' }]).round, 2);
  assert.equal(nextRound([...legacy, { piece: 'type', round: 4, winner: 'ours', opponent: 'oracle' }]), 5);
  assert.equal(nextRound([]), 1);
});

ok('every recorded round names an opponent the progress page can caption', () => {
  // No `opponent` is the gauntlet's own rounds, which the page captions "iA Writer"; anything else
  // has to be a name `tools/rounds.mjs` has words for, or the page prints the key.
  assert.equal(opponentName({ round: 1, winner: 'ours' }), 'iA Writer');
  assert.equal(opponentName({ opponent: 'oracle' }), 'Parity oracle');
  const dir = path.join(ROOT, 'progress/rounds');
  for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.json'))) {
    const r = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
    if (r.opponent) assert.ok(OPPONENTS[r.opponent], `${file} was judged against "${r.opponent}", which tools/rounds.mjs has no caption for`);
  }
});

// ---------- the Design oracle's crop ----------
// A capture that is on disk and is the shape this cuts: the caret mid-word state of #154, which is
// a region rather than a whole window and so exercises `window` and `at` as well.
const CAPTURE = 'mac-native-01-light-caret-midword.png';

// One synthetic Piece, so a state with an `opponent` can be resolved without waiting for the first
// real one — the ticket that changes a row adds that (#161 is the tooling and nothing else).
function synthetic(opponent, overrides = {}) {
  return {
    defaults: { ...states.defaults, w: 200, h: 100, scale: 2 },
    pieces: { synthetic: { crop: { ...overrides, opponent } } },
  };
}

ok('a state that names an opponent carries no such flag, and is shot in Mono at the defaults\' type', () => {
  const made = synthetic({ capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] }, { font: 'quattro', size: 28, theme: 'dark' });
  const [s] = resolveStates(made, 'synthetic');
  // `opponent` says who judges the state, not what it is shot at, so it must never reach the flags:
  // `unservable` would call it a flag no tool serves and refuse the Piece before a window opened.
  assert.deepEqual(unservable(made.defaults, s.flags), []);
  assert.deepEqual(s.opponent, { capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] });
  // Mono at the defaults' size whatever the state itself says, so the two grids compare cell for
  // cell; everything that is not the type is still the state's own.
  assert.equal(s.flags.font, 'mono');
  assert.equal(s.flags.size, states.defaults.size);
  assert.equal(s.flags.theme, 'dark');
  // And a state with no opponent is exactly what it was.
  const plain = resolveStates(states, 'type').find((t) => t.name === 'quattro');
  assert.equal(plain.opponent, null);
  assert.equal(plain.flags.font, 'quattro');
});

ok('a round names the opponent its states were judged against, and says so when they differ', () => {
  const design = { capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] };
  assert.equal(opponentOf([{ opponent: null }, { opponent: null }]), 'oracle');
  assert.equal(opponentOf([{ opponent: design }, { opponent: design }]), 'mac-native');
  // A Piece part-way through: #165-#168 move one row at a time, so this is what the ledger says
  // for every round between the first row moving and the last.
  assert.equal(opponentOf([{ opponent: null }, { opponent: design }]), 'mixed');
  // And every word one of them returns is one the progress page can caption.
  for (const word of ['oracle', 'mac-native', 'mixed']) {
    assert.ok(OPPONENTS[word], `a round recording ${word} would be captioned by its own key`);
    assert.equal(opponentName({ opponent: word }), OPPONENTS[word]);
  }
});

ok('an opponent is resolved against the capture on disk, and the centre rule carries the container', () => {
  const ours = { w: 2880, h: 1800 };
  const given = resolveOpponent(ROOT, 'crop', { capture: CAPTURE, crop: [1500, 380, 100, 40], ours: [1428, 331, 100, 40] }, ours);
  assert.equal(given.capture, path.join(CAPTURES, CAPTURE));
  assert.deepEqual(given.crop, [1500, 380, 100, 40]);

  // The centre rule, on the numbers ADR 0015 names: the capture sits in a 3024 x 1898 window and
  // ours is 2880 x 1800, both with the container centred, so what transfers is the offset from the
  // centre — 38 px right of it and 549 px above, in both windows.
  const centred = resolveOpponent(ROOT, 'crop', { capture: CAPTURE, crop: [1500, 380, 100, 40], window: [3024, 1898], ours: 'centre' }, ours);
  assert.deepEqual(centred.ours, [1428, 331, 100, 40]);
  // `at` moves the capture within that window, and the crop with it.
  const moved = resolveOpponent(ROOT, 'crop', { capture: CAPTURE, crop: [1500, 380, 100, 40], window: [3024, 1898], at: [40, 100], ours: 'centre' }, ours);
  assert.deepEqual(moved.ours, [1468, 431, 100, 40]);
});

ok('a crop that cannot be cut says which state and why, and cuts nothing', () => {
  const ours = { w: 2880, h: 1800 };
  const bad = [
    [{ capture: 'ref/ia/shots/mac-native/nope.png', crop: [0, 0, 8, 8], ours: 'centre' }, /a file name under/],
    [{ capture: 'mac-native-99-nothing.png', crop: [0, 0, 8, 8], ours: 'centre' }, /not a capture on disk/],
    [{ capture: CAPTURE, crop: [2900, 0, 100, 8], ours: 'centre' }, /past the capture/],
    [{ capture: CAPTURE, crop: [0, 0, 8], ours: 'centre' }, /is not \[x, y, w, h\]/],
    [{ capture: CAPTURE, crop: [0, 0, 8, 8], ours: [2879, 0, 8, 8] }, /past ours/],
    [{ capture: CAPTURE, crop: [0, 0, 8, 8], ours: 'middle' }, /is not \[x, y, w, h\]/],
    [{ capture: CAPTURE, crop: [0, 0, 8, 8], window: [100, 100], ours: 'centre' }, /does not hold it/],
  ];
  for (const [opponent, why] of bad) {
    assert.throws(() => resolveOpponent(ROOT, 'crop', opponent, ours), why, `${JSON.stringify(opponent)} was resolved`);
  }
});

ok('a state with an opponent pairs the named crop, and both sides are the same rectangle', () => {
  const crop = [1500, 380, 100, 40];
  const mine = [40, 20, 100, 40];
  const made = synthetic({ capture: CAPTURE, crop, ours: mine });
  const [s] = resolveStates(made, 'synthetic');
  const cut = resolveOpponent(ROOT, s.name, s.opponent, { w: s.flags.w * s.flags.scale, h: s.flags.h * s.flags.scale });

  // Ours, as a shot of the size this state would be shot at: 200 x 100 logical at scale 2.
  const w = s.flags.w * s.flags.scale;
  const h = s.flags.h * s.flags.scale;
  const data = Buffer.alloc(w * h * 3);
  for (let i = 0; i < w * h; i += 1) data[i * 3] = i % 251;
  const shot = encodePng({ w, h, ch: 3, data });

  const capture = fs.readFileSync(path.join(ROOT, cut.capture));
  const theirsCrop = cropPng(capture, cut.crop);
  const oursCrop = cropPng(shot, cut.ours);

  inTemp((dir) => {
    fs.writeFileSync(path.join(dir, 'ours.png'), oursCrop);
    fs.writeFileSync(path.join(dir, 'theirs.png'), theirsCrop);
    const paired = pair('synthetic', s.name, 'ours.png', 'theirs.png');
    // A pair of two sizes is the one thing a critic must not be shown: it would answer "the same
    // window at two zooms" whatever the two apps did.
    for (const letter of ['A', 'B']) {
      const size = pngSize(fs.readFileSync(path.join(paired.dir, `${letter}.png`)));
      assert.deepEqual([size.w, size.h], [crop[2], crop[3]], `${letter}.png is not the crop`);
    }
    const key = reveal('synthetic', s.name);
    const theirs = decodePng(fs.readFileSync(path.join(paired.dir, key.ours === 'A' ? 'B.png' : 'A.png')));
    const whole = decodePng(capture);
    // The crop is the rectangle the state named, and not some other rectangle of the same size:
    // its corners are the capture's own pixels at that offset.
    for (const [x, y] of [[0, 0], [crop[2] - 1, crop[3] - 1], [50, 20]]) {
      const from = ((crop[1] + y) * whole.w + crop[0] + x) * whole.ch;
      const to = (y * crop[2] + x) * theirs.ch;
      assert.deepEqual(
        [...theirs.data.subarray(to, to + theirs.ch)],
        [...whole.data.subarray(from, from + whole.ch)],
        `the crop's pixel at ${x}, ${y} is not the capture's at ${crop[0] + x}, ${crop[1] + y}`,
      );
    }
  });
});

// ---------- the answers that need no compositor ----------
// The owner's line is the whole of stdout; everything said on the way to it is held back, and comes
// out on stderr only when the run ends in no verdict at all, so the two are kept apart here rather
// than interleaved.
function gate(...argv) {
  const env = typeof argv[argv.length - 1] === 'object' ? argv.pop() : {};
  try {
    return { code: 0, out: execFileSync(GATE, argv, { cwd: ROOT, encoding: 'utf8', env: { ...process.env, ...env }, stdio: ['ignore', 'pipe', 'pipe'] }), err: '' };
  } catch (e) {
    return { code: e.status, out: e.stdout || '', err: e.stderr || '' };
  }
}
const lastLine = (r) => r.out.trim().split('\n').pop();

// The latency Piece is judged by arithmetic over a bench run, and the arithmetic itself is
// `tools/bench-selftest.mjs`'s to check. What is checked here is the half that is this command's:
// which runs it will not take a verdict from. A run it *would* take one from is not exercised,
// because writing a round into the ledger is not something a test may do.
ok('the latency Piece is judged on a whole bench run, and refuses anything less', () => {
  const missing = gate('judge', 'latency', '--summary', 'shots/latency/summary-nosuchrun.json');
  assert.equal(missing.code, 3, missing.out);
  assert.match(lastLine(missing), /^gate judge latency: refused \(shots\/latency\/summary-nosuchrun\.json is not a file to read\)/);

  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-judge-latency-'));
  const subset = path.join(tmp, 'summary-20260828T000000.json');
  fs.writeFileSync(subset, JSON.stringify({
    ran: '--regimes revision,paste_blocks',
    headline: 'prose_end_of_draft',
    regimes: [{ regime: 'revision', mean_ms: 2, worst_ms: 8, cold_ms: 120, pass: true }],
    regimes_not_run: [],
    lines: ['gate bench --regimes revision,paste_blocks: pass'],
  }));
  const short = gate('judge', 'latency', '--summary', subset);
  fs.rmSync(tmp, { recursive: true, force: true });
  assert.equal(short.code, 3, short.out);
  assert.match(lastLine(short), /^gate judge latency: refused \(.* is not a whole run/,
    'two regimes are a measurement, not a verdict on the Piece');
});

// #66's panel mode measures on the physical display, which is fractional-scale and so is not the
// output the budget or the oracle's numbers belong to. It writes `panel-summary-*.json`, which the
// newest-summary search does not match, so the accident this guards against is the deliberate one:
// a panel run named to `--summary` by hand.
ok('a --panel run is informational, and the latency Piece is never judged from one', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-judge-panel-'));
  // Whole, accounted for, and well inside every bar — so the only thing that can refuse it is that
  // it was taken on the panel.
  const rows = regimes().map((r) => ({
    regime: r.name, mean_ms: 2, worst_ms: 8, p50_ms: 2, p99_ms: 7, cold_ms: 120, pass: true,
  }));
  const body = {
    ran: '--all',
    headline: 'prose_end_of_draft',
    regimes: rows,
    regimes_not_run: [],
    regimes_unaccounted_for: [],
    pass: null,
    informational: 'the physical panel is never a Gate condition',
    panel: { output: 'DP-3', mode: '3840x2160', scale: 1.5 },
    lines: ['gate bench --all --panel: informational'],
  };
  const named = path.join(tmp, 'panel-summary-20260828T000000.json');
  fs.writeFileSync(named, JSON.stringify(body));
  const r = gate('judge', 'latency', '--summary', named);
  fs.rmSync(tmp, { recursive: true, force: true });
  assert.equal(r.code, 3, r.out);
  assert.match(lastLine(r), /^gate judge latency: refused \(.* is informational/,
    'a whole run of twelve inside every bar is still not evidence when it came off the panel');

  // The same body without the mark is deliberately not run here. It is whole, accounted for and
  // inside every bar, so judge would take a verdict from it and write a round — and writing a round
  // into the ledger is not something a test may do. That it would is the point: the mark is the
  // only thing standing between a panel run and the latency Piece.
});

// The chrome states were the other half of this case until the tools learnt `typing` and `menu`;
// `files` is what is left waiting on a spec, and the states.json defaults are the whole of the rule.
ok('a Piece whose states need flags the app has not got names them and judges nothing', () => {
  const r = gate('judge', 'files');
  assert.equal(r.code, 3, r.err);
  assert.match(r.err, /state library names library, sidebar/);
  assert.match(r.err, /state search names library, search, sidebar/);
  assert.match(lastLine(r), /^gate judge files: refused \(2 of 2 states name flags the app has not got\)/);
});

ok('a refused run is one line on stdout, and what it said is on stderr and in its log', () => {
  const log = path.join(ROOT, 'target/gate/judge-files.log');
  fs.rmSync(log, { force: true });
  const r = gate('judge', 'files');
  assert.deepEqual(r.out.trim().split('\n').length, 1, `stdout was more than the owner's line:\n${r.out}`);
  assert.match(r.err, /state library names library, sidebar/);
  assert.match(fs.readFileSync(log, 'utf8'), /state library names library, sidebar/);
});

// ---------- the flags ours has not got yet ----------
// A judged state can be servable and unshootable at once: states.json learns a flag when a tool
// under legacy/ can serve it, and the app learns to parse it a spec later. The judge asks the built
// binary which it is, so only the reading of the answer is pinned here.
ok('the flag ours refused is read out of what ours said, and nothing else is', () => {
  assert.equal(refusedFlag('quill: --typing: not a flag Quill knows\n'), '--typing');
  assert.equal(refusedFlag('quill: --menu: not a flag Quill knows'), '--menu');
  // A refusal for any other reason is not a missing flag, and saying it was would send the agent
  // to the wrong spec.
  assert.equal(refusedFlag('quill: --caret 9999: past the end of the Document'), null);
  assert.equal(refusedFlag(''), null);
});

// ---------- the settings file a round opens ours with ----------
ok('--settings reaches ours and nothing else, and the round says which file it was', () => {
  const flags = flagsOf('chrome').bars;
  const plain = oursArgv(ROOT, flags, null);
  assert.deepEqual(plain, quillArgv(ROOT, flags), 'with no file named, ours opens the way every round opens it');

  // Never opened here, by this or by oursArgv: the path is carried to ours' command line verbatim
  // and it is the app that reads it, so what this asserts is the carrying and not the file.
  const fixture = 'a-settings-file-this-test-never-opens.toml';
  const withIt = oursArgv(ROOT, flags, fixture);
  assert.deepEqual(withIt.slice(0, plain.length), plain, 'the judged state is still the judged state');
  assert.deepEqual(withIt.slice(plain.length), ['--settings', fixture], 'and the fixture is all that was added');

  // The opponent's side of the pair is a png frozen before the round began, so there is no command
  // line to hand it the same file on. What keeps that honest is the round saying which file ours had.
  const rec = round({
    piece: 'chrome', number: 1, judged: [], opponent: OPPONENTS[0], oracle: null, note: '', at: 'now',
    build: { git: 'abc1234', binary: 'def5678', settings: fixture },
    headline: { margin: 0, gap: '', gapTheirs: '', verdict: '', ours: '', theirs: '', secondary: '' },
  });
  assert.equal(rec.build.settings, fixture);
});

ok('--settings needs a path, and the Piece that shoots nothing will not take one', () => {
  const bare = gate('judge', 'chrome', '--settings');
  assert.equal(bare.code, 3, bare.err);
  assert.match(bare.err, /--settings takes the settings file/);

  const latency = gate('judge', 'latency', '--settings', 'a-settings-file-this-test-never-opens.toml');
  assert.equal(latency.code, 3, latency.err);
  assert.match(lastLine(latency), /^gate judge latency: refused \(--settings is not a flag the latency Piece has\)/);
});

ok('a crop that cannot be cut is refused before a window opens, and never as a verdict', () => {
  // Named by the state and by the reason, in one run rather than one per state, for the reason
  // every other refusal here is: a crop found wrong at the third state has already spent two
  // critics. It is checked before the frozen opponent is, so a Piece whose crops are wrong says so
  // rather than sending an agent to run `tools/gate oracle` first.
  const file = path.join(os.tmpdir(), `quill-judge-selftest-${process.pid}.json`);
  const states = JSON.parse(fs.readFileSync(path.join(ROOT, 'shots/oracle/states.json'), 'utf8'));
  states.pieces.type = {
    off: { opponent: { capture: 'mac-native-01-light-caret-midword.png', crop: [2900, 0, 100, 40], ours: 'centre' } },
    gone: { opponent: { capture: 'mac-native-99-nothing.png', crop: [0, 0, 8, 8], ours: 'centre' } },
  };
  fs.writeFileSync(file, JSON.stringify(states));
  try {
    const r = gate('judge', 'type', { QUILL_STATES: file });
    assert.equal(r.code, 3, `${r.out}${r.err}`);
    assert.match(r.err, /state off's opponent's crop runs to 3000 x 40, past the capture/);
    assert.match(r.err, /state gone's opponent names ref\/ia\/shots\/mac-native\/mac-native-99-nothing\.png, which is not a capture on disk/);
    assert.match(lastLine(r), /^gate judge type: refused \(2 of 2 states name a mac-native crop that cannot be cut\)/);
  } finally {
    fs.rmSync(file, { force: true });
  }
});

ok('a Piece nobody has judged states for is not a Piece', () => {
  const r = gate('judge', 'nosuch');
  assert.equal(r.code, 3, r.err);
  assert.match(r.err, /nosuch/);
  assert.match(lastLine(r), /^gate judge nosuch: refused/);
});

ok('a command line this cannot read is not a verdict, and never 2', () => {
  // 2 is "this Piece has been lost", the loudest thing the command says. A typo must not be able to
  // wear it: an agent branching on the code could not tell them apart.
  for (const argv of [['judge'], ['judge', 'type', '--nosuch'], ['judge', 'type', '--note'], ['judge', 'type', 'page']]) {
    const r = gate(...argv);
    assert.equal(r.code, 3, `tools/gate ${argv.join(' ')} exited ${r.code}\n${r.err}`);
    assert.match(r.err, /usage: tools\/gate judge/);
  }
});

if (failures === 0) {
  console.log('judge selftest: pass');
} else {
  console.log(`judge selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
