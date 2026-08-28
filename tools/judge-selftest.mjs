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
import { APP_ID, appeared, classPattern, launchEnv, parseToplevels, pngSize, quillArgv, rulesLua } from './harness.mjs';
import { OPPONENTS, criticAnswer, criticPrompt, decisive, nextRound, round, wonBefore } from './judge.mjs';
import { readStates, resolveStates } from './oracle.mjs';

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
  assert.equal(flag('--chrome'), 'on');
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
  const dir = path.join(ROOT, 'progress/rounds');
  for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.json'))) {
    const r = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
    // No `opponent` is the gauntlet's own rounds, which the page captions "iA Writer"; anything
    // else has to be a name `tools/progress.mjs` has words for, or the page prints the key.
    if (r.opponent) assert.ok(OPPONENTS[r.opponent], `${file} was judged against "${r.opponent}", which tools/judge.mjs has no caption for`);
  }
});

// ---------- the answers that need no compositor ----------
// The owner's line is the last one on stdout; everything the agent reads on the way to it is on
// stderr, so the two are kept apart here rather than interleaved.
function gate(...argv) {
  try {
    return { code: 0, out: execFileSync(GATE, argv, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }), err: '' };
  } catch (e) {
    return { code: e.status, out: e.stdout || '', err: e.stderr || '' };
  }
}
const lastLine = (r) => r.out.trim().split('\n').pop();

ok('the latency Piece is benched, not judged, and says so by name', () => {
  const r = gate('judge', 'latency');
  assert.equal(r.code, 3, r.out);
  assert.match(lastLine(r), /^gate judge latency: refused \(the latency Piece is benched, not judged/);
});

ok('a Piece whose states need flags the app has not got names them and judges nothing', () => {
  const r = gate('judge', 'chrome');
  assert.equal(r.code, 3, r.err);
  assert.match(r.err, /state typing names typing/);
  assert.match(r.err, /state view-menu names menu/);
  assert.match(lastLine(r), /^gate judge chrome: refused \(3 of 4 states name flags the app has not got\)/);

  const files = gate('judge', 'files');
  assert.equal(files.code, 3, files.err);
  assert.match(files.err, /state library names library, sidebar/);
});

ok('a Piece nobody has judged states for is not a Piece', () => {
  const r = gate('judge', 'nosuch');
  assert.equal(r.code, 3, r.err);
  assert.match(r.err, /nosuch/);
  assert.match(lastLine(r), /^gate judge nosuch: refused/);
});

ok('the command with no Piece is a usage error rather than a verdict', () => {
  const r = gate('judge');
  assert.equal(r.code, 2, r.err);
  assert.match(r.err, /usage: tools\/gate judge/);
});

if (failures === 0) {
  console.log('judge selftest: pass');
} else {
  console.log(`judge selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
