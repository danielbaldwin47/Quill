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

import { ASSERTIONS, SYNTAX, assertState, secondShot, validate } from './assert-state.mjs';
import { pair, pairDir, reveal } from './blind.mjs';
import { CAPTURES, cropPng, encodePng, overlaid, resolveOpponent } from './crop.mjs';
import {
  ACCENT, ACCENT_HEX, APP_ID, DIALOG_APP_ID, accentPixels, appeared, carriesAccent, classPattern, launchEnv,
  parseToplevels, pngSize, quillArgv, rulesLua, wantsLitCaret,
} from './harness.mjs';
import { VERDICT_KEYS, carriedFrom, criticAnswer, criticPrompt, opponentOf, oursArgv, refusedFlag, shotPaths } from './judge.mjs';
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
ok('every state pins Syntax highlight and an override reaches the app', () => {
  assert.equal(states.defaults.syntax, 'off');
  for (const piece of Object.keys(states.pieces)) {
    for (const state of resolveStates(states, piece)) {
      const argv = quillArgv(ROOT, state.flags);
      assert.equal(argv[argv.indexOf('--syntax') + 1], state.flags.syntax, `${piece}/${state.name}`);
    }
  }
  const argv = quillArgv(ROOT, { ...states.defaults, syntax: 'nouns,adverbs' });
  assert.equal(argv[argv.indexOf('--syntax') + 1], 'nouns,adverbs');
});

ok('every state pins Style check and an override reaches the app', () => {
  assert.equal(states.defaults.style, 'off');
  for (const piece of Object.keys(states.pieces)) {
    for (const state of resolveStates(states, piece)) {
      const argv = quillArgv(ROOT, state.flags);
      assert.equal(argv[argv.indexOf('--style') + 1], state.flags.style, `${piece}/${state.name}`);
    }
  }
  const argv = quillArgv(ROOT, { ...states.defaults, style: 'fillers,cliches' });
  assert.equal(argv[argv.indexOf('--style') + 1], 'fillers,cliches');
});

ok('a state becomes the native flags that state means', () => {
  const caret = flagsOf('caret');
  const argv = quillArgv(ROOT, caret.selection);
  const flag = (name) => argv[argv.indexOf(name) + 1];
  assert.ok(argv.includes('--deterministic'), 'every judged shot is of a determined frame');
  assert.equal(flag('--w'), '1440');
  assert.equal(flag('--h'), '900');
  assert.equal(flag('--theme'), 'light');
  // Mono rather than the defaults' Duo, because this state names a `mac-native` crop as its
  // opponent and such a state is shot on a grid the capture can be compared against cell for cell
  // (ADR 0015). The state itself says nothing about the face: `resolveStates` does.
  assert.equal(flag('--font'), 'mono');
  assert.equal(flag('--step'), '5');
  assert.equal(flag('--focus'), 'off');
  assert.equal(flag('--syntax'), 'off');
  assert.equal(flag('--style'), 'off');
  // The caret Piece is judged bare (#139), so its states override the defaults' chrome; that
  // override reaching the command line is the half of this case the defaults cannot show.
  assert.equal(flag('--chrome'), 'off');
  assert.equal(flag('--text'), path.join(ROOT, 'ref/sample.md'));
  // Bytes on the way in and bytes on the way out: the native flags take the form states.json
  // writes, which is what the oracle has to convert away from and this does not.
  assert.equal(flag('--caret'), '36');
  assert.equal(flag('--select'), '18,36');
  assert.ok(!argv.includes('--typewriter'));
  assert.ok(!argv.includes('--nocaret'));
  // Neither of the two flags that are not the app's ever reaches its command line.
  assert.ok(!argv.includes('--scale') && !argv.includes('--active'));

  const empty = quillArgv(ROOT, flagsOf('chrome').empty);
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

  // The three flags of the files Piece. `--library` is joined to the root the way `--text` is,
  // because ours is launched from wherever the harness is standing; the other two are the state's
  // own words. The passage is one of the fixture's own documents, so the page and the sidebar show
  // the same one.
  const files = flagsOf('files');
  const lib = quillArgv(ROOT, files.library);
  assert.equal(lib[lib.indexOf('--library') + 1], path.join(ROOT, files.library.library));
  assert.ok(lib.includes('--sidebar'));
  assert.ok(!lib.includes('--search'), 'a Library at rest is not a narrowed one');
  assert.equal(lib[lib.indexOf('--text') + 1], path.join(ROOT, files.library.text));
  const narrowed = quillArgv(ROOT, files.search);
  assert.equal(narrowed[narrowed.indexOf('--search') + 1], files.search.search);
  assert.ok(narrowed.includes('--sidebar'), 'a query narrows a pane that is open');
  assert.ok(!quillArgv(ROOT, chrome.bars).includes('--sidebar'), 'the Piece that is not files opens no pane');

  // The Preview's two flags. `--preview` is named only by a state that wants the pane, because a
  // pane's being open is never remembered and every other state is shot without one; `--template`
  // is named by all of them off the defaults, because it pins the whole `[template]` table and the
  // shape of a heading is the shape of one whatever Piece the shot is of.
  const preview = flagsOf('preview');
  const beside = quillArgv(ROOT, preview.split);
  assert.equal(beside[beside.indexOf('--preview') + 1], 'split');
  assert.equal(beside[beside.indexOf('--template') + 1], 'modern');
  const whole = quillArgv(ROOT, preview.full);
  assert.equal(whole[whole.indexOf('--preview') + 1], 'full');
  const plain = quillArgv(ROOT, flagsOf('page').light);
  assert.ok(!plain.includes('--preview'), 'a state that says nothing about the pane opens with none');
  assert.equal(plain[plain.indexOf('--template') + 1], 'modern', 'and is pinned to the defaults Template all the same');

  // The Export dialog's flag, named only by the state that wants a dialog: nothing else opens one,
  // and a still cannot pull an expander open, so the flag is what opens it with its Options showing.
  const opened = quillArgv(ROOT, flagsOf('export').dialog);
  assert.equal(opened[opened.indexOf('--export-dialog') + 1], 'pdf');
  assert.ok(opened.includes('--nocaret'), 'the keyboard is the dialog\'s while it is up, so the Editor draws no caret');
  assert.ok(!plain.includes('--export-dialog'), 'a state that says nothing about a dialog opens none');
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

  // The Export dialog maps under a class of its own, and is ruled apart from the window it stands
  // over: it wants the stage's workspace and Omarchy's compositing undone, and it must not be given
  // ours' size and position, because where the compositor puts it is what `export/dialog` measures.
  assert.equal(classPattern(DIALOG_APP_ID), '^quill$');
  assert.match(focused, /local D = "\^quill\$"/);
  assert.equal(focused.match(/dialog\(\{/g).length, 3);
  assert.match(focused, /dialog\(\{ name = "quill-gate-dialog-workspace", workspace = "7 silent" \}\)/);
  assert.ok(!/dialog\(\{[^}]*\b(size|move) =/.test(focused), 'the dialog is placed by the compositor, not by a rule');
});

ok('a dialog\'s own buffer goes over the page\'s, which is the one frame a writer sees', () => {
  // Two toplevels, two `grim -T` captures, one frame: the paste is where they become it. Painted
  // rather than captured, for the reason every fixture here is.
  const page = Buffer.alloc(8 * 6 * 3, 10);
  const sheet = Buffer.alloc(3 * 2 * 3, 200);
  const shot = decodePng(overlaid(
    encodePng({ w: 8, h: 6, ch: 3, data: page }),
    encodePng({ w: 3, h: 2, ch: 3, data: sheet }),
    [4, 3],
  ));
  assert.deepEqual([shot.w, shot.h], [8, 6], 'the frame is the page\'s size, whatever the dialog\'s is');
  const at = (x, y) => shot.data[((y * shot.w) + x) * shot.ch];
  assert.equal(at(4, 3), 200, 'the dialog lands where the compositor put it');
  assert.equal(at(6, 4), 200);
  assert.equal(at(3, 3), 10, 'and the page is left either side of it');
  assert.equal(at(4, 2), 10);

  // A dialog hanging off the edge is clipped rather than throwing: the compositor can place one
  // anywhere, and a shot that refused would be refusing the placement it is there to measure.
  const over = decodePng(overlaid(
    encodePng({ w: 8, h: 6, ch: 3, data: Buffer.alloc(8 * 6 * 3, 10) }),
    encodePng({ w: 3, h: 2, ch: 3, data: sheet }),
    [7, 5],
  ));
  assert.equal(over.data[((5 * 8) + 7) * 3], 200);
  assert.deepEqual([over.w, over.h], [8, 6]);
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
  const judged = JSON.parse(fs.readFileSync(path.join(ROOT, 'shots/oracle/states.json'), 'utf8')).pieces.type;
  assert.deepEqual(lost.states.map((s) => s.name), Object.keys(judged));
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
  const made = synthetic({ capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] }, { font: 'quattro', step: 6, theme: 'dark' });
  const [s] = resolveStates(made, 'synthetic');
  // `opponent` says who judges the state, not what it is shot at, so it must never reach the flags:
  // `unservable` would call it a flag no tool serves and refuse the Piece before a window opened.
  assert.deepEqual(unservable(made.defaults, s.flags), []);
  assert.deepEqual(s.opponent, { capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] });
  // Mono at the defaults' step whatever the state itself says, so the two grids compare cell for
  // cell; everything that is not the type is still the state's own.
  assert.equal(s.flags.font, 'mono');
  assert.equal(s.flags.step, states.defaults.step);
  assert.equal(s.flags.theme, 'dark');
  // And a state with no opponent is exactly what it was.
  const plain = resolveStates(states, 'type').find((t) => t.name === 'quattro');
  assert.equal(plain.opponent, null);
  assert.equal(plain.flags.font, 'quattro');
});

ok('a round names the opponent its states were judged against, and says so when they differ', () => {
  const design = { capture: CAPTURE, crop: [0, 0, 8, 8], ours: [0, 0, 8, 8] };
  const ghost = { kind: 'ghost', alpha: 0.3 };
  assert.equal(opponentOf([{ opponent: null }, { opponent: null }]), 'oracle');
  assert.equal(opponentOf([{ opponent: design }, { opponent: design }]), 'mac-native');
  // A Piece part-way through: #165-#168 move one row at a time, so this is what the ledger says
  // for every round between the first row moving and the last.
  assert.equal(opponentOf([{ opponent: null }, { opponent: design }]), 'mixed');
  // A Piece with no opponent at all, and one holding an asserted state beside a paired one — which
  // is the caret from #147 on: a crop, a Parity pair and an assertion in the one round (ADR 0017).
  assert.equal(opponentOf([{ assert: ghost }, { assert: ghost }]), 'asserted');
  assert.equal(opponentOf([{ opponent: design }, { opponent: null }, { assert: ghost }]), 'mixed');
  assert.equal(opponentOf([{ opponent: design }, { assert: ghost }]), 'mixed');
  // And every word one of them returns is one the progress page can caption.
  for (const word of ['oracle', 'mac-native', 'mixed', 'asserted']) {
    assert.ok(OPPONENTS[word], `a round recording ${word} would be captioned by its own key`);
    assert.equal(opponentName({ opponent: word }), OPPONENTS[word]);
  }
});

// A ground and a bar that are deliberately **not** the app's palette.
//
// The rule solves its alpha out of the two shots and never compares a colour against one written
// down, so it must hold for any ground and any bar the bar-finder can see. Painting `theme.rs`'s
// own hexes here would test one palette and go quietly stale the day it moves — which is how
// `tools/keys-assert.mjs`'s constants came to disagree with the app. These two only have to satisfy
// `readBar`: chroma at least 40, and more blue than green.
const GROUND = [240, 238, 235];
const BAR = [10, 100, 200];

// A shot with nothing in it but ground and one bar: the two frames the ghost rule reads.
function painted({ w = 80, h = 40, paper = GROUND, bar = BAR, at: [x0, y0, bw, bh] = [20, 10, 6, 20] } = {}) {
  const data = Buffer.alloc(w * h * 3);
  for (let i = 0; i < w * h; i += 1) for (let c = 0; c < 3; c += 1) data[i * 3 + c] = paper[c];
  for (let y = y0; y < y0 + bh; y += 1) {
    for (let x = x0; x < x0 + bw; x += 1) for (let c = 0; c < 3; c += 1) data[(y * w + x) * 3 + c] = bar[c];
  }
  return encodePng({ w, h, ch: 3, data });
}

// The bar as it is painted at `alpha` over the ground, which is what a ghost is.
const over = (alpha, bar = BAR, paper = GROUND) => bar.map((v, c) => Math.round(v * alpha + paper[c] * (1 - alpha)));

ok('the ghost is measured off ours own pixels, and the alpha is solved rather than looked up', () => {
  const lit = painted();
  const held = assertState({ kind: 'ghost', alpha: 0.3 }, { lit, dim: painted({ bar: over(0.3) }) });
  assert.equal(held.ours, true, held.why);
  // Solved, not asserted against a hex: the answer carries the alpha it read, and it is the one the
  // state asked for. This repo has twice had a pinned colour outlive the palette it was copied from.
  assert.ok(Math.abs(held.alpha - 0.3) <= 0.02, `solved ${held.alpha} for a ghost painted at 0.3`);

  // A ghost at the wrong alpha fails, and the neighbouring rungs of the ladder cannot pass for 0.3.
  for (const wrong of [0.2, 0.25, 0.35, 0.5]) {
    const got = assertState({ kind: 'ghost', alpha: 0.3 }, { lit, dim: painted({ bar: over(wrong) }) });
    assert.equal(got.ours, false, `a ghost at ${wrong} passed for 0.3: ${got.why}`);
  }

  // A bar that moves when the window deactivates is the defect this catches, whatever its alpha.
  const moved = assertState({ kind: 'ghost', alpha: 0.3 }, { lit, dim: painted({ bar: over(0.3), at: [23, 10, 6, 20] }) });
  assert.equal(moved.ours, false, moved.why);
  assert.match(moved.why, /moves when the window deactivates/);

  // No caret at all is its own answer and never a silent pass.
  const gone = assertState({ kind: 'ghost', alpha: 0.3 }, { lit, dim: painted({ bar: [247, 247, 247] }) });
  assert.equal(gone.ours, false, gone.why);
  assert.match(gone.why, /no caret/);

  // An assertion nobody wrote, and an alpha that is not one, are refused rather than guessed at —
  // and by the same call `tools/gate judge` makes over every asserted state before it shoots.
  assert.throws(() => assertState({ kind: 'shimmer' }, { lit, dim: lit }), /the assertion is "shimmer"/);
  assert.throws(() => validate({ kind: 'shimmer' }), /the assertion is "shimmer"/);
  for (const alpha of [3, 0, 1, '0.3', undefined]) {
    assert.throws(() => validate({ kind: 'ghost', alpha }), /between 0 and 1/, `an alpha of ${alpha} was taken`);
  }
  assert.deepEqual(Object.keys(ASSERTIONS), ['ghost', 'folded', 'split', 'full', 'pdf-split', 'pdf-full', 'dialog', 'syntax']);
});

ok('the ghost refuses to read a bar it cannot see the ground beside, and never guesses one', () => {
  const spec = { kind: 'ghost', alpha: 0.3 };

  // A bar standing on ink has no clear run either side, and neither shot shows what is under it.
  // Solving an alpha against the wrong ground would answer confidently and wrongly, so it does not.
  // The ink goes on the row the ground is read at — the bar's middle, y 19 of a bar spanning 10..29.
  const litInk = smudged(painted(), 17, 19);
  const dimInk = smudged(painted({ bar: over(0.3) }), 17, 19);
  const unclear = assertState(spec, { lit: litInk, dim: dimInk });
  assert.equal(unclear.ours, false, unclear.why);
  assert.match(unclear.why, /does not stand in clear ground/);

  // A bar the ground barely separates from cannot carry an alpha on that channel, and a bar it
  // separates from on no channel at all is refused rather than divided by nearly nothing.
  const grey = [200, 200, 200];
  const flat = assertState(spec, {
    lit: painted({ paper: grey, bar: grey }),
    dim: painted({ paper: grey, bar: grey }),
  });
  assert.equal(flat.ours, false, flat.why);
  assert.match(flat.why, /no caret/, 'a bar the same colour as its ground is not a bar this can see');

  // Two shots of different sizes are not two shots of one state.
  const odd = assertState(spec, { lit: painted(), dim: painted({ w: 81 }) });
  assert.equal(odd.ours, false, odd.why);
  assert.match(odd.why, /80x40 and the ghosted one 81x40/);

  // Two runs of colour is either a second caret or something this was never meant to measure.
  const twice = painted();
  const doubled = smudged(painted({ bar: over(0.3) }), 40, 20, over(0.3));
  const two = assertState(spec, { lit: twice, dim: doubled });
  assert.equal(two.ours, false, two.why);
  assert.match(two.why, /not one run of colour/);
});

// ---------- Syntax highlight ----------

// Real GTK captures, their command-line provenance and the protected rectangles are recorded in
// tools/syntax-fixture/README.md. Variants replace pixels in those captures, never app internals.
const syntaxFixture = (name) => fs.readFileSync(path.join(ROOT, 'tools/syntax-fixture', name));
const syntaxPair = (name) => ({ dim: syntaxFixture(`${name}-ours.png`), lit: syntaxFixture(`${name}-ours-lit.png`) });
const syntaxRule = (name) => states.pieces.syntax[name].assert;
// Since #319 three of the five states name a `mac-native` opponent instead of a rule, and a state
// is answered one way (ADR 0017). Their pairs are still shot, and the rules they carried are still
// the rules to exercise the production one with, so they are derived from the two rules that are
// still registered rather than written out again: `live`'s is all five Categories on the light
// ground with the Live arm, so dropping that arm is what `all-light` held and adding the Focus arm
// is what `focus-sentence` held. Derived, so a Category added to the Piece reaches all four.
const syntaxAllLight = (() => { const { live, ...rest } = syntaxRule('live'); return rest; })();
// The focused sentence of `passage-syntax.md` — the passage the Design oracle's own focus frame was
// shot on, and now ours — holds no conjunction, so four Categories are all a Focus Sentence shot of
// it can show. That is the capture's own reading: CAPTURE-ORIGINAL-MBP.md § Dim wins over colour
// lists the focused row as carrying noun, verb, adjective and adverb, on both grounds.
const syntaxFocus = { ...syntaxAllLight, expected: syntaxAllLight.expected.filter((c) => c !== 'conjunctions'), focus: true };
const syntaxDark = { ...syntaxAllLight, theme: 'dark' };
// The Design oracle's light noun, which is the pigment every variant below paints with: the one
// Category that is in `all-light`'s five and not in `adjectives-adverbs`' two. Read out of the
// production table rather than written down again, so a repalette reaches the variants too.
const NOUN = SYNTAX.light.colours.nouns;
function alteredSyntax(pair, change) {
  const page = decodePng(pair.dim);
  const source = decodePng(pair.lit);
  for (let y = 0; y < page.h; y += 1) {
    for (let x = 0; x < page.w; x += 1) {
      const i = (y * page.w + x) * page.ch;
      const j = (y * source.w + x) * source.ch;
      const rgb = [...page.data.subarray(i, i + 3)];
      const off = [...source.data.subarray(j, j + 3)];
      const replacement = change({ x, y, rgb, off });
      if (replacement) page.data.set(replacement, i);
    }
  }
  return { ...pair, dim: encodePng(page) };
}
const rgbIs = (a, b) => a.every((v, i) => v === b[i]);

// One pixel of an encoded shot, for a variant that moves ink rather than recolouring it. The shot
// is decoded once and kept, because the caller asks per pixel.
const decoded = new Map();
function readPixel(png, x, y) {
  if (!decoded.has(png)) decoded.set(png, decodePng(png));
  const image = decoded.get(png);
  const i = (y * image.w + x) * image.ch;
  return [...image.data.subarray(i, i + 3)];
}

ok('all five Syntax states hold on real captures and the companion pins only master off', () => {
  assert.deepEqual(Object.keys(states.pieces.syntax), ['all-light', 'all-dark', 'adjectives-adverbs', 'focus-sentence', 'live']);
  // Which of the five carry a rule and which a `mac-native` opponent, and that no state carries
  // both — the judge refuses that pairing, and this is where the Piece's own shape is held to it.
  const rules = { 'all-light': syntaxAllLight, 'all-dark': syntaxDark, 'adjectives-adverbs': syntaxRule('adjectives-adverbs'), 'focus-sentence': syntaxFocus, live: syntaxRule('live') };
  assert.deepEqual(Object.keys(states.pieces.syntax).filter((n) => states.pieces.syntax[n].opponent),
    ['all-light', 'all-dark', 'focus-sentence']);
  const began = performance.now();
  for (const state of resolveStates(states, 'syntax')) {
    const rule = rules[state.name];
    assert.equal(Boolean(state.assert) && Boolean(state.opponent), false, `${state.name} is answered one way`);
    if (state.assert) {
      assert.deepEqual(state.assert, rule);
      const second = secondShot(state.assert, state);
      assert.deepEqual(second, { state: { ...state, flags: { ...state.flags, syntax: 'off' } }, options: {} });
    }
    assert.equal(state.flags.theme, rule.theme);
    assert.equal(state.flags.chrome, 'off');
    // A state whose flag names Categories shows exactly those. A state on `on` shows all five,
    // except under Focus Sentence, where only the bright sentence is coloured and what it holds is
    // the passage's business rather than the flag's.
    assert.ok(rule.expected.every((c) => syntaxAllLight.expected.includes(c)), `${state.name} colours only the five`);
    if (state.flags.syntax !== 'on') assert.deepEqual(rule.expected, state.flags.syntax.split(','));
    else if (!rule.focus) assert.deepEqual(rule.expected, syntaxAllLight.expected);
    assert.equal(Boolean(rule.focus), state.flags.focus === 'sentence');
    assert.equal(Boolean(rule.live), state.flags.live);
    const got = assertState(rule, syntaxPair(state.name));
    assert.equal(got.ours, true, `${state.name}: ${got.why}`);
    if (rule.focus) assert.ok(got.brightRows.length > 0);
    if (rule.live) assert.ok(got.headingPixels >= 32);
  }
  console.log(`judge selftest: syntax five real 2880x1800 pairs measured in ${(performance.now() - began).toFixed(0)} ms`);
});

ok('the actual syntax-off shot fails the all-light rule', () => {
  const pair = syntaxPair('all-light');
  const got = assertState(syntaxAllLight, { ...pair, dim: pair.lit });
  assert.equal(got.ours, false);
  assert.match(got.why, /nouns has 0 opaque pixels/);
});

ok('Syntax rejects missing and unexpected Categories, wrong pigments and displaced colour', () => {
  const pair = syntaxPair('all-light');
  // Removing a whole Category's opaque cores must fail even though its antialiased edge remains.
  const missing = alteredSyntax(pair, ({ rgb, off }) => rgbIs(rgb, NOUN) ? off : null);
  assert.match(assertState(syntaxAllLight, missing).why, /nouns has 0 opaque pixels/);
  const unexpected = assertState(syntaxRule('adjectives-adverbs'), pair);
  assert.equal(unexpected.ours, false);
  assert.match(unexpected.why, /unexpected|non-prose ink/);
  let inserted = false;
  const oneNoun = alteredSyntax(syntaxPair('adjectives-adverbs'), ({ rgb, off }) => {
    if (!inserted && rgbIs(rgb, [25, 25, 25]) && rgbIs(off, rgb)) {
      inserted = true;
      return NOUN;
    }
    return null;
  });
  assert.equal(inserted, true);
  assert.match(assertState(syntaxRule('adjectives-adverbs'), oneNoun).why, /unexpected nouns/);
  for (const pigment of [[4, 250, 100], NOUN]) {
    const wrong = alteredSyntax(pair, ({ x, y }) => x === 10 && y === 100 ? pigment : null);
    assert.equal(assertState(syntaxAllLight, wrong).ours, false);
  }
  const odd = assertState(syntaxAllLight, { ...pair, lit: painted() });
  assert.equal(odd.ours, false);
  assert.match(odd.why, /differ in size/);
});

ok('Syntax Focus rejects coloured dim glyphs and Live requires colour on the scaled heading', () => {
  const focus = syntaxPair('focus-sentence');
  let leaked = false;
  const wrongFocus = alteredSyntax(focus, ({ rgb }) => {
    if (!leaked && rgbIs(rgb, [198, 196, 194])) { leaked = true; return NOUN; }
    return null;
  });
  assert.equal(leaked, true, 'the captured sentence has dim glyphs to challenge');
  assert.equal(assertState(syntaxFocus, wrongFocus).ours, false);
  leaked = false;
  const faintLeak = alteredSyntax(focus, ({ rgb }) => {
    if (!leaked && rgbIs(rgb, [198, 196, 194])) { leaked = true; return [237, 208, 198]; }
    return null;
  });
  assert.match(assertState(syntaxFocus, faintLeak).why, /outside the bright rows/);
  const live = syntaxPair('live');
  const { heading } = assertState(syntaxRule('live'), live);
  const plainHeading = alteredSyntax(live, ({ y, off }) => y >= heading.top && y <= heading.bottom ? off : null);
  const got = assertState(syntaxRule('live'), plainHeading);
  assert.equal(got.ours, false);
  assert.match(got.why, /Live heading has 0 Category pixels/);
  const unscaled = assertState(syntaxRule('live'), syntaxPair('all-light'));
  assert.equal(unscaled.ours, false);
  assert.match(unscaled.why, /no heading taller/);
});

ok('Syntax refuses invalid configuration before a shot opens', () => {
  const spec = syntaxAllLight;
  for (const patch of [
    { theme: 'sepia' }, { expected: [] }, { expected: ['nouns', 'nouns'] },
    { expected: ['pronouns'] }, { expected: 'nouns' }, { focus: 'sentence' },
    { live: 1 }, { extra: true }, { protected: [[0, 0, 0, 4]] }, { protected: [[0, 0, 1]] },
  ]) assert.throws(() => validate({ ...spec, ...patch }), /syntax/);
});

ok('Syntax preserves captured marker, code and URL pixels, and a coloured protected glyph fails', () => {
  const pair = { dim: syntaxFixture('protection-on.png'), lit: syntaxFixture('protection-off.png') };
  // Device-pixel rectangles read off protection-off.png: heading #; link destination; inline
  // code on its two wrapped rows; autolink; fenced code (including markers); indented code. Each
  // is 88 px higher than the rectangles #317 measured, which is the page top #241 made a constant
  // (docs/design.md row Page top): the pair was re-shot for #319 and every band moved by it.
  const protectedRegions = [
    [570, 77, 32, 45], [1100, 362, 645, 62], [1906, 362, 320, 62],
    [620, 444, 214, 54], [620, 587, 675, 62], [592, 727, 1695, 225],
    [592, 1012, 1695, 90],
  ];
  const spec = { ...syntaxAllLight, protected: protectedRegions };
  const good = assertState(spec, pair);
  assert.equal(good.ours, true, good.why);
  // The defect `protection-shifted.png` preserved — a full retag after a worker result moved the
  // indented code and the prose under it — put back on the current capture rather than read out of
  // that file. The file is two palettes and one page top old: #319 measured the Categories off the
  // Design oracle, so its pixels are colours the rule no longer knows, and the production rule
  // rejects it at the first of them rather than at the moved block, which is the wrong red. The
  // defect is geometry, so it survives the move; the capture did not.
  const moved = alteredSyntax(pair, ({ x, y }) => {
    const [rx, ry, w, h] = protectedRegions.at(-1);
    return x >= rx && x < rx + w && y >= ry && y < ry + h ? readPixel(pair.dim, x, y - 4) : null;
  });
  const shifted = assertState(spec, moved);
  assert.equal(shifted.ours, false);
  assert.match(shifted.why, /protected marker, code or URL pixel/);
  // Change one real opaque glyph in each protected subject. Each must independently go red,
  // including a fenced body glyph whose uncoloured ink happens to equal ordinary prose ink.
  for (const [rx, ry, w, h] of protectedRegions) {
    let changed = false;
    const wrong = alteredSyntax(pair, ({ x, y, rgb }) => {
      if (!changed && x >= rx && x < rx + w && y >= ry && y < ry + h
          && (rgbIs(rgb, [25, 25, 25]) || rgbIs(rgb, [181, 179, 176]))) {
        changed = true;
        return NOUN;
      }
      return null;
    });
    assert.equal(changed, true, `protected rectangle ${rx},${ry} contains a captured glyph`);
    const got = assertState(spec, wrong);
    assert.equal(got.ours, false, got.why);
    assert.match(got.why, /protected|outside a bright source glyph/);
  }
  const outside = assertState({ ...spec, protected: [[3000, 0, 2, 2]] }, pair);
  assert.equal(outside.ours, false);
  assert.match(outside.why, /outside the shot/);
});

// ---------- Style check ----------

ok('the nine style states are judged against mac-native crops of the capture in #354', () => {
  assert.deepEqual(Object.keys(states.pieces.style), ['on-light', 'on-dark', 'fillers-light',
    'focus-light', 'focus-dark', 'syntax-light', 'syntax-dark', 'select-light', 'select-dark']);
  for (const state of resolveStates(states, 'style')) {
    // The passage is the one the engine's fixture test and the capture use, so the three cannot
    // drift, and every state is answered by a crop of the frame the capture shot for it rather
    // than by arithmetic over ours: the mark has a Design oracle now (#367, ADR 0015).
    assert.equal(state.flags.text, 'ref/style.md', state.name);
    assert.equal(state.assert ?? null, null, `${state.name} is judged, not asserted`);
    assert.match(state.opponent.capture, /^mac-native-24-(light|dark)-style-/, state.name);
    assert.equal(state.opponent.capture.includes(state.flags.theme), true,
      `${state.name} is judged against its own ground`);
    for (const rect of [state.opponent.crop, state.opponent.ours]) {
      assert.equal(rect.length, 4, state.name);
      assert.equal(rect.every((n) => Number.isInteger(n) && n >= 0), true, state.name);
    }
    // Both rectangles are the same size, because the two crops are read side by side.
    assert.deepEqual(state.opponent.crop.slice(2), state.opponent.ours.slice(2), state.name);
  }
});

// ---------- the fold ----------

// The leading and the row the fold's fixtures are painted on, in the pixels of a shot.
//
// Two rows of one paragraph start `PITCH` apart, and a row of ink is `ROW` of it: the numbers only
// have to hold that shape, because the rule reads the pitch off the page it is given rather than
// from anything written down.
const PITCH = 12;
const ROW = 6;

// A page painted as blocks of ink: `{ top, left, rows, height }` each, and the caret's bar on the
// first row of block `caret`.
//
// The blocks are the shape `live/folded` is measured on — a heading, the paragraph the caret is in,
// and a list whose bullets stand on the body column — and nothing else is on the page, because the
// rule reads blocks of ink and has no opinion about what the ink says.
function page(blocks, caret = 1, { w = 90, h = 220, paper = GROUND } = {}) {
  const data = Buffer.alloc(w * h * 3);
  for (let i = 0; i < w * h; i += 1) for (let c = 0; c < 3; c += 1) data[i * 3 + c] = paper[c];
  const paint = (x0, y0, wide, tall, colour) => {
    for (let y = y0; y < y0 + tall; y += 1) {
      for (let x = x0; x < x0 + wide; x += 1) for (let c = 0; c < 3; c += 1) data[(y * w + x) * 3 + c] = colour[c];
    }
  };
  blocks.forEach((block, i) => {
    const tall = block.height ?? ROW;
    for (let r = 0; r < block.rows; r += 1) {
      paint(block.left, block.top + r * PITCH, 40, tall, [20, 20, 20]);
      if (i === caret && r === 0) paint(block.left + 10, block.top, 2, tall, BAR);
    }
  });
  return encodePng({ w, h, ch: 3, data });
}

// The page with the markers on it: the heading's `#` hung in the gutter at column 14, the caret's
// paragraph and the list's bullets both beginning on the body column at 20.
const SOURCE = [
  { top: 10, left: 14, rows: 1, height: 10 },
  { top: 34, left: 20, rows: 3 },
  { top: 82, left: 20, rows: 2 },
];

// The same page folded: nothing in the gutter, the heading's ink 1.6 times as tall and its row
// taller with it, and the list still beginning on the body column — its bullets are folded to the
// paper and the furniture #274 draws stands in the cells they kept.
const FOLDED = [
  { top: 10, left: 20, rows: 1, height: 16 },
  { top: 40, left: 20, rows: 3 },
  { top: 88, left: 20, rows: 2 },
];

// One block of a fixture with `change` written over it.
const with_ = (blocks, i, change) => blocks.map((block, at) => (at === i ? { ...block, ...change } : block));

ok('the fold is measured off ours own pixels: the caret block untouched, the markers gone, the heading on the ladder', () => {
  const spec = { kind: 'folded', scale: 1.6 };
  const lit = page(SOURCE);
  const held = assertState(spec, { lit, dim: page(FOLDED) });
  assert.equal(held.ours, true, held.why);
  // Solved, not asserted against a length: the answer carries the ladder it read off the two shots.
  assert.ok(Math.abs(held.scale - 1.6) <= 0.1, `read ${held.scale} for a heading painted at 1.6`);

  // A marker left standing in a folded block is the defect this catches, either where it hangs...
  const hung = assertState(spec, { lit, dim: page(with_(FOLDED, 0, { left: 14 })) });
  assert.equal(hung.ours, false, hung.why);
  assert.match(hung.why, /still hanging in the gutter/);

  // ...or where a fold closed a marker's cells up instead of folding it to its ground, which shows
  // as the block's words moving off the column they were laid out on.
  const bullet = assertState(spec, { lit, dim: page(with_(FOLDED, 2, { left: 26 })) });
  assert.equal(bullet.ours, false, bullet.why);
  assert.match(bullet.why, /the fold moved a block’s words/);

  // A heading left at body height fails the ladder, and so does one at the rung below it.
  const flat = assertState(spec, { lit, dim: page(with_(FOLDED, 0, { height: 10 })) });
  assert.equal(flat.ours, false, flat.why);
  assert.match(flat.why, /rung of the Live ladder/);
  const wrong = assertState(spec, { lit, dim: page(with_(FOLDED, 0, { height: 14 })) });
  assert.equal(wrong.ours, false, wrong.why);

  // The block the caret is in is the writer's to edit, so a fold that changed it is a defect
  // wherever the page put it.
  const moved = assertState(spec, { lit, dim: page(with_(FOLDED, 1, { left: 24 })) });
  assert.equal(moved.ours, false, moved.why);

  // A rung of the ladder that is not one is refused rather than guessed at, by the call
  // `tools/gate judge` makes over every asserted state before it shoots.
  for (const scale of [1, 0.5, '1.6', undefined]) {
    assert.throws(() => validate({ kind: 'folded', scale }), /rung of the Live ladder/, `a scale of ${scale} was taken`);
  }
});

ok('the fold refuses a pair it cannot read a fold out of, and never passes one silently', () => {
  const spec = { kind: 'folded', scale: 1.6 };

  // Two shots of different sizes are not two shots of one state.
  const odd = assertState(spec, { lit: page(SOURCE), dim: page(FOLDED, 1, { w: 91 }) });
  assert.equal(odd.ours, false, odd.why);
  assert.match(odd.why, /is 91x220 and the source one 90x220/);

  // A source page with nothing hung in the gutter is a page this cannot measure a fold against:
  // saying so is the honest answer, and passing it would be a rule that holds for any two shots.
  const flat = assertState(spec, { lit: page(with_(SOURCE, 0, { left: 20 })), dim: page(FOLDED) });
  assert.equal(flat.ours, false, flat.why);
  assert.match(flat.why, /nothing hangs in the gutter/);

  // A fold that took a whole block off the page is not a fold.
  const gone = assertState(spec, { lit: page(SOURCE), dim: page(FOLDED.slice(0, 2)) });
  assert.equal(gone.ours, false, gone.why);
  assert.match(gone.why, /how many blocks the page has/);

  // A page with no caret on it names no open block, and is refused rather than measured.
  const blind = assertState(spec, { lit: page(SOURCE), dim: page(FOLDED, -1) });
  assert.equal(blind.ours, false, blind.why);
  assert.match(blind.why, /no caret/);
});

// One more mark painted into an encoded shot, at `x`, `y`: ink by default, which is what a caret
// standing on a glyph looks like to the ground reader.
function smudged(png, x, y, colour = [20, 20, 20]) {
  const { w, h, ch, data } = decodePng(png);
  for (let i = 0; i < 4; i += 1) {
    for (let c = 0; c < 3; c += 1) data[((y * w) + x + i) * ch + c] = colour[c];
  }
  return encodePng({ w, h, ch, data });
}

// ---------- the Preview pane, measured off its own pixels ----------

// Two papers and an ink that are deliberately **not** the app's palette, for the reason the ghost's
// ground is not: both Preview rules read every colour they compare out of the shot, so they have to
// hold for whatever pair of grounds a Template hands them. Painting `theme.rs`'s hexes here would
// test one palette and go quietly stale the day a Template moves.
const EDITOR_PAPER = [30, 30, 30];
const PAGE_PAPER = [16, 16, 16];
const GLYPH = [200, 200, 200];

// A window divided at `divider`: the Editor's paper left of it, the rendered page's from it to the
// edge, a heading's run of ink `ink` wide centred on `centre`, and a paragraph in each pane below.
//
// The paragraph matters twice over. It is wider than the heading, so a rule that read the pane's
// whole ink rather than the first block of it would measure the wrong run; and it puts ink down the
// columns either side of the divider, so the divider is found by a majority down each column rather
// than by the first row that happens to change colour.
function paned({ w = 400, h = 200, divider = 200, left = EDITOR_PAPER, right = PAGE_PAPER, centre = 300, ink = 120 } = {}) {
  const data = Buffer.alloc(w * h * 3);
  const put = (x, y, rgb) => { for (let c = 0; c < 3; c += 1) data[((y * w) + x) * 3 + c] = rgb[c]; };
  for (let y = 0; y < h; y += 1) for (let x = 0; x < w; x += 1) put(x, y, x < divider ? left : right);
  const run = (y0, y1, x0, x1) => { for (let y = y0; y < y1; y += 1) for (let x = x0; x < x1; x += 1) put(x, y, GLYPH); };
  if (ink > 0) {
    run(20, 32, Math.round(centre - ink / 2), Math.round(centre + ink / 2));
    run(50, 58, divider + 20, w - 20);
    if (divider > 40) run(50, 58, 20, divider - 20);
  }
  return encodePng({ w, h, ch: 3, data });
}

// One sheet of paper, which is what Full is: the same painter with nothing left of the divider.
const sheet = (opts = {}) => paned({ divider: 0, centre: 200, ...opts });

// Where a heading centred in the pane right of `divider` has its ink, for a 400 px window.
const inPane = (divider) => (divider + 399) / 2 + 0.5;

ok('Split is measured off the divider, the two papers and the heading over the rendered page', () => {
  const spec = { kind: 'split' };
  const held = assertState(spec, { dim: paned() });
  assert.equal(held.ours, true, held.why);
  assert.equal(held.divider, 200, 'the divider is the boundary between the last left column and the first right one');
  assert.deepEqual(held.papers, ['#1e1e1e', '#101010'], 'both papers are read off the shot, not compared against a hex');
  assert.match(held.why, /50\.0 % across the window/);

  // A divider dragged off the middle is the defect this catches, at either side and with the
  // heading still centred in the pane it made — so the reading names the divider and not the page.
  for (const at of [160, 240]) {
    const got = assertState(spec, { dim: paned({ divider: at, centre: inPane(at) }) });
    assert.equal(got.ours, false, `a divider at ${at} of 400 passed for half the window: ${got.why}`);
    assert.match(got.why, /the divider is \d+\.\d px off the window's half/);
  }
  // And the 3 px it is read to is real at both ends of itself: the glyph rounding gets through and
  // the pixel after it does not.
  assert.equal(assertState(spec, { dim: paned({ divider: 197, centre: inPane(197) }) }).ours, true);
  assert.equal(assertState(spec, { dim: paned({ divider: 196, centre: inPane(196) }) }).ours, false);

  // A pane showing the Editor's own ground rather than a rendered page has no two papers to divide.
  const one = assertState(spec, { dim: paned({ right: EDITOR_PAPER }) });
  assert.equal(one.ours, false, one.why);
  assert.match(one.why, /both halves of the window are #1e1e1e/);

  // A heading left where the Editor would put it — on the left edge of its measure — is the other
  // half of the rule, and it fails with the divider exactly where it should be.
  const flush = assertState(spec, { dim: paned({ centre: 260 }) });
  assert.equal(flush.ours, false, flush.why);
  assert.match(flush.why, /the heading is 40\.0 px off the centre of the pane it is drawn in/);

  // An empty pane is its own answer and never a silent pass.
  const bare = assertState(spec, { dim: paned({ ink: 0 }) });
  assert.equal(bare.ours, false, bare.why);
  assert.match(bare.why, /nothing was rendered on it/);

  // The rule takes nothing but its kind, and an entry that sets something nobody reads is refused
  // by the same call `tools/gate judge` makes over every asserted state before it shoots.
  assert.throws(() => validate({ kind: 'split', alpha: 0.3 }), /the split assertion takes nothing but its kind, and this one names alpha/);
});

ok('Full is one paper across the window with the heading centred in it', () => {
  const spec = { kind: 'full' };
  const held = assertState(spec, { dim: sheet() });
  assert.equal(held.ours, true, held.why);
  assert.equal(held.paper, '#101010');
  assert.match(held.why, /one paper, #101010, across all 400x200 of the window/);

  // A pane still beside the page is a Full that did not hide the Editor's scroller.
  const halved = assertState(spec, { dim: paned() });
  assert.equal(halved.ours, false, halved.why);
  assert.match(halved.why, /the window is not one paper: #1e1e1e down its left edge against #101010 at the right/);

  // A heading off the window's centre fails, and the reading says by how much.
  const flush = assertState(spec, { dim: sheet({ centre: 160 }) });
  assert.equal(flush.ours, false, flush.why);
  assert.match(flush.why, /40\.0 px out and past the 3 px/);

  // A blank window is not a rendered page.
  const bare = assertState(spec, { dim: sheet({ ink: 0 }) });
  assert.equal(bare.ours, false, bare.why);
  assert.match(bare.why, /from edge to edge: nothing was rendered on it/);

  assert.throws(() => validate({ kind: 'full', paper: '#ffffff' }), /the full assertion takes nothing but its kind, and this one names paper/);
});

// ---------- the page column, measured off its own pixels ----------

// A surround, a paper and an ink that are again **not** the app's palette, and are picked for the
// two thresholds the rules stand on rather than for the look. The paper is 30 off the surround,
// inside the 40 a pixel has to clear to count as ink, so a page edge is an edge and not a glyph the
// way `#ffffff` on `#f7f7f7` is not one; the glyph is 190 off the paper, well past it. And the
// surround is far lighter than the Editor's paper, which is what a dark theme's window looks like
// with a light column in half of it.
const PDF_SURROUND = [200, 200, 200];
const PDF_PAPER = [230, 230, 230];
const PDF_GLYPH = [40, 40, 40];

// A column of pages: the Editor's paper left of `divider`, the surround right of it, `pages` pages
// `pageW` wide centred on `centre` with `top0` px of air over the first and `gap` between each
// pair, and a run of ink `ink` wide on each.
//
// Everything the two rules read is a parameter, because every one of them is a defect somebody
// could ship: a divider dragged off the middle, a column wearing the theme instead of its own
// surround, a page filling the pane, a page off the pane's centre, a gap between pages that is not
// the gap over the first, and a column with no pages drawn on it at all.
function pdfColumn({
  w = 400, h = 400, divider = 0, editor = EDITOR_PAPER, surround = PDF_SURROUND, paper = PDF_PAPER,
  centre = null, pageW = 360, top0 = 20, gap = 20, pageH = 170, pages = 2, ink = 60,
} = {}) {
  const data = Buffer.alloc(w * h * 3);
  const put = (x, y, rgb) => { for (let c = 0; c < 3; c += 1) data[((y * w) + x) * 3 + c] = rgb[c]; };
  const fill = (x0, x1, y0, y1, rgb) => {
    for (let y = Math.max(0, y0); y < Math.min(h, y1); y += 1) {
      for (let x = Math.max(0, x0); x < Math.min(w, x1); x += 1) put(x, y, rgb);
    }
  };
  fill(0, divider, 0, h, editor);
  fill(divider, w, 0, h, surround);
  const middle = centre === null ? (divider + w - 1) / 2 : centre;
  const left = Math.round(middle - (pageW - 1) / 2);
  for (let at = 0; at < pages; at += 1) {
    const top = top0 + at * (pageH + gap);
    fill(left, left + pageW, top, top + pageH, paper);
    if (ink > 0) fill(Math.round(middle - ink / 2), Math.round(middle + ink / 2), top + 30, top + 42, PDF_GLYPH);
  }
  return encodePng({ w, h, ch: 3, data });
}

ok('PDF Split is the divider, a lighter surround than the Editor, and a page standing in the column', () => {
  const spec = { kind: 'pdf-split' };
  const held = assertState(spec, { dim: pdfColumn({ divider: 200, pageW: 160 }) });
  assert.equal(held.ours, true, held.why);
  assert.equal(held.divider, 200, 'the divider is the boundary between the Editor and the surround');
  assert.deepEqual(held.grounds, ['#1e1e1e', '#c8c8c8', '#e6e6e6'],
    'the Editor, the surround and the paper are all read off the shot, not compared against a hex');
  assert.deepEqual(held.page, [220, 20, 160, 360], 'the page is the pane less a gutter each side');

  // A divider dragged off the middle is a defect, at either side, with the page still centred in
  // the pane it made — so the reading names the divider and not the page. The 3 px it is read to is
  // real at both ends of itself.
  for (const at of [160, 240]) {
    const got = assertState(spec, { dim: pdfColumn({ divider: at, pageW: 400 - at - 40 }) });
    assert.equal(got.ours, false, `a divider at ${at} of 400 passed for half the window: ${got.why}`);
    assert.match(got.why, /the divider is \d+\.\d px off the window's half/);
  }
  assert.equal(assertState(spec, { dim: pdfColumn({ divider: 197, pageW: 160 }) }).ours, true);
  assert.equal(assertState(spec, { dim: pdfColumn({ divider: 196, pageW: 160 }) }).ours, false);

  // A column wearing the theme rather than its own surround: the pages are drawn in the Template's
  // light palette whatever the theme, so the ground under them is never darker than the Editor's.
  const dark = assertState(spec, { dim: pdfColumn({ divider: 200, pageW: 160, surround: [10, 10, 10] }) });
  assert.equal(dark.ours, false, dark.why);
  assert.match(dark.why, /no lighter than the Editor's own paper/);

  // A page run out to the pane's own edge, with no gutter left on that side to stand in.
  const edgeToEdge = assertState(spec, { dim: pdfColumn({ divider: 200, pageW: 180, centre: 309.5 }) });
  assert.equal(edgeToEdge.ours, false, edgeToEdge.why);
  assert.match(edgeToEdge.why, /reaches the edge of the pane/);

  // A page laid out against the window rather than the half it was put in.
  const offset = assertState(spec, { dim: pdfColumn({ divider: 200, pageW: 160, centre: 280 }) });
  assert.equal(offset.ours, false, offset.why);
  assert.match(offset.why, /19\.0 px off the centre of the pane it stands in/);

  // A pane with nothing drawn on it, and a window whose air over the pages is the Editor's own
  // ground, are each their own answer and never a silent pass.
  const empty = assertState(spec, { dim: pdfColumn({ divider: 200, pages: 0 }) });
  assert.equal(empty.ours, false, empty.why);
  assert.match(empty.why, /no page is standing on the surround/);
  const one = assertState(spec, { dim: pdfColumn({ divider: 200, pageW: 160, surround: EDITOR_PAPER }) });
  assert.equal(one.ours, false, one.why);
  assert.match(one.why, /the ground the Editor is made of/);

  assert.throws(() => validate({ kind: 'pdf-split', paper: '#ffffff' }),
    /the pdf-split assertion takes nothing but its kind, and this one names paper/);
});

ok('PDF Full is the page centred in the window with the column\'s gap over it and between its pages', () => {
  const spec = { kind: 'pdf-full' };
  const held = assertState(spec, { dim: pdfColumn() });
  assert.equal(held.ours, true, held.why);
  assert.deepEqual(held.grounds, ['#c8c8c8', '#e6e6e6']);
  assert.equal(held.gap, 20, 'the air over the first page is the gap the column stacks with');
  assert.match(held.why, /2 pages with 20 px between them/);

  // A page off the window's centre, at the 3 px this is read to and the pixel past it.
  assert.equal(assertState(spec, { dim: pdfColumn({ centre: 202.5 }) }).ours, true);
  const off = assertState(spec, { dim: pdfColumn({ centre: 203.5 }) });
  assert.equal(off.ours, false, off.why);
  assert.match(off.why, /4\.0 px off the window's centre/);

  // A page filling the window is a column that lost its gutters, and a first page flush with the
  // top of the column is one that lost its air.
  const edgeToEdge = assertState(spec, { dim: pdfColumn({ pageW: 400 }) });
  assert.equal(edgeToEdge.ours, false, edgeToEdge.why);
  assert.match(edgeToEdge.why, /reaches the edge of the window/);
  const flush = assertState(spec, { dim: pdfColumn({ top0: 0 }) });
  assert.equal(flush.ours, false, flush.why);
  assert.match(flush.why, /none of the gap the column stacks its pages with over it/);

  // The pages are stacked with one gap: a gap between two of them that is not the gap over the
  // first is a column that stacked them by something else. Read to the same 3 px.
  assert.equal(assertState(spec, { dim: pdfColumn({ gap: 23 }) }).ours, true);
  const wide = assertState(spec, { dim: pdfColumn({ gap: 24 }) });
  assert.equal(wide.ours, false, wide.why);
  assert.match(wide.why, /a gap of 24 px between two pages against the 20 px over the first/);

  // A blank page, and a window with no page on it at all.
  const bare = assertState(spec, { dim: pdfColumn({ ink: 0 }) });
  assert.equal(bare.ours, false, bare.why);
  assert.match(bare.why, /the page is blank/);
  const empty = assertState(spec, { dim: pdfColumn({ pages: 0 }) });
  assert.equal(empty.ours, false, empty.why);
  assert.match(empty.why, /from edge to edge: no page is standing on the surround/);

  assert.throws(() => validate({ kind: 'pdf-full', gap: 20 }),
    /the pdf-full assertion takes nothing but its kind, and this one names gap/);
});

// ---------- the Export dialog, measured off its own pixels ----------

// The app's dimmed Editor and its undimmed pane surround, deliberately not the
// app's palette: the rule reads both from the pair, so it has to hold for
// whatever grounds a theme hands it. The surround is twice the dim here; the
// rule must solve the relationship rather than copy that fixture arithmetic.
const DIALOG_DIM = [70, 70, 70];
const DIALOG_SURROUND = [140, 140, 140];

// The paper the pages behind the dialog are drawn on, which is the one thing in the shot that says
// the pane is showing them.
//
// Lighter than [`DIALOG_SURROUND`], because the page's white is what separates the pane from the
// surround around it. The Editor starts as [`DIALOG_SURROUND`] in the reference and becomes
// [`DIALOG_DIM`] only in the dialog shot.
const DIALOG_PAPER = [250, 250, 250];

// A window of page paper with a dialog standing on it, and the PDF pane behind it.
//
// `off` moves the dialog's centre off the window's, `rows` is how many bands of ink it carries —
// four is a shut dialog, a dozen an open one — and `dw`/`dh` are its size. The bands are drawn with
// air between them, which is what makes them bands: one row of a dialog is a run of inked rows.
// `paper` is the page standing in the right-hand half, which is the pane in PDF Split; `null` is
// the pane away or the Web sheet's own paper filling it, which is the state's other defect.
function dialogShot({
  w = 400, h = 300, dw = 160, dh = 220, off = [0, 0], rows = 12,
  editor = DIALOG_DIM, surround = DIALOG_SURROUND,
  sheet = DIALOG_SURROUND, paper = DIALOG_PAPER,
} = {}) {
  const data = Buffer.alloc(w * h * 3);
  const put = (x, y, rgb) => { for (let c = 0; c < 3; c += 1) data[((y * w) + x) * 3 + c] = rgb[c]; };
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) put(x, y, x < (w >> 1) ? editor : surround);
  }
  // The pane fills the right half, and the page stands in it with a gutter each side and air above
  // and below, exactly as the column stacks one.
  if (paper !== null) {
    for (let y = 10; y < h - 10; y += 1) {
      for (let x = (w >> 1) + 20; x < w - 20; x += 1) put(x, y, paper);
    }
  }
  if (sheet !== null) {
    const left = Math.round((w - dw) / 2 + off[0]);
    const top = Math.round((h - dh) / 2 + off[1]);
    for (let y = top; y < top + dh; y += 1) for (let x = left; x < left + dw; x += 1) put(x, y, sheet);
    for (let i = 0; i < rows; i += 1) {
      const y0 = top + 10 + i * 14;
      for (let y = y0; y < y0 + 6; y += 1) {
        for (let x = left + 8; x < left + dw - 8; x += 1) put(x, y, GLYPH);
      }
    }
  }
  return encodePng({ w, h, ch: 3, data });
}

ok('the Export dialog dims only the Editor over an unchanged pane, centred with its expander open', () => {
  const spec = { kind: 'dialog' };
  const second = secondShot(spec, {
    flags: { export: 'pdf', preview: 'pdf-split', theme: 'light' },
  });
  assert.equal(second.state.flags.export, null, 'the reference opens no dialog');
  assert.equal(second.state.flags.preview, 'pdf-split', 'the reference keeps the same pane');
  assert.equal(second.state.flags.theme, 'light', 'the reference keeps every unrelated flag');
  const reference = dialogShot({ editor: DIALOG_SURROUND, sheet: null });
  const held = assertState(spec, { dim: dialogShot(), lit: reference });
  assert.equal(held.ours, true, held.why);
  assert.deepEqual(held.grounds, ['#464646', '#8c8c8c'], 'the Editor dim and pane surround are read off the shot');
  assert.deepEqual(held.dialog, [120, 40, 160, 220], 'the dialog is where its own ground runs');
  assert.equal(held.bands, 12, 'and one band per row of it');
  assert.deepEqual(held.pane, ['#fafafa', '#8c8c8c'], 'the page beside it and the surround it stands on');

  const paneDimmed = assertState(spec, {
    dim: dialogShot({ surround: DIALOG_DIM, sheet: DIALOG_SURROUND }),
    lit: reference,
  });
  assert.equal(paneDimmed.ours, false, paneDimmed.why);
  assert.match(paneDimmed.why, /pane surround .* changed from/);

  const editorClear = assertState(spec, {
    dim: dialogShot({ editor: DIALOG_SURROUND }),
    lit: reference,
  });
  assert.equal(editorClear.ours, false, editorClear.why);
  assert.match(editorClear.why, /Editor is not darker than/);

  // The dialog drives the pane, so a shot with nothing in the pane beside it is the defect this
  // state is now for: the pane away, or the Web sheet where the pages should be, reads as one
  // ground right of the dialog either way.
  const alone = assertState(spec, { dim: dialogShot({ paper: null }), lit: reference });
  assert.equal(alone.ours, false, alone.why);
  assert.match(alone.why, /no page is standing in the pane beside it/);

  // Nor is one darker than what it stands on: the pages are drawn in the Template's light palette
  // whatever the theme is wearing, which is what puts them against the surround at all.
  const dark = assertState(spec, { dim: dialogShot({ paper: [8, 8, 8] }), lit: reference });
  assert.equal(dark.ours, false, dark.why);
  assert.match(dark.why, /no lighter than the .* it stands on/);

  // A dialog dragged off the window's centre is the defect this catches, in either direction.
  for (const nudged of [[20, 0], [0, -20]]) {
    const got = assertState(spec, { dim: dialogShot({ off: nudged }), lit: reference });
    assert.equal(got.ours, false, `a dialog ${nudged} off centre passed: ${got.why}`);
    assert.match(got.why, /px off the window's centre/);
  }
  // And the 8 px it is read to is real at both ends of itself.
  assert.equal(assertState(spec, { dim: dialogShot({ off: [8, 0] }), lit: reference }).ours, true);
  assert.equal(assertState(spec, { dim: dialogShot({ off: [9, 0] }), lit: reference }).ours, false);

  // A shut expander is the state's whole subject: the dialog is there, centred, and carrying only
  // the file name, the folder, the Options label and the Export button.
  const shut = assertState(spec, { dim: dialogShot({ rows: 4 }), lit: reference });
  assert.equal(shut.ours, false, shut.why);
  assert.match(shut.why, /carries 4 bands of ink and an open expander carries at least 8, so the Options are shut/);

  // No dialog at all is its own answer and never a silent pass.
  const none = assertState(spec, { dim: dialogShot({ sheet: null }), lit: reference });
  assert.equal(none.ours, false, none.why);
  assert.match(none.why, /no dialog stands over the page/);

  // A second ground that runs to an edge is not a dialog standing over a page — it is the page
  // gone, which is what a dialog opened full-window would look like. Both edges, and each alone:
  // the box is walked out from the window's centre to the first column that is not its ground,
  // so a dialog that reaches the pane's far edge is refused as surely as one that reaches the
  // Editor's. Short, so that over the pane's right quarter the page still outweighs the surround
  // the dialog is drawn in and the surround reading the rule makes first is the reference's.
  for (const [dw, off] of [[400, [0, 0]], [300, [-50, 0]], [300, [50, 0]]]) {
    const filled = assertState(spec, {
      dim: dialogShot({ dw, dh: 40, off, rows: 2 }),
      lit: reference,
    });
    assert.equal(filled.ours, false, filled.why);
    assert.match(filled.why, /it reaches an edge, so it is not a dialog standing over the page/);
  }

  assert.throws(() => validate({ kind: 'dialog', rows: 12 }), /the dialog assertion takes nothing but its kind, and this one names rows/);
});

// ---------- the caret a judged shot proves it took focus by ----------

// One colour out of `quill-engine/src/theme.rs`'s own palette table.
//
// Every fixture below is painted in these rather than in a hex written here. The harness refuses a
// shot for missing one exact colour, so the single failure that would refuse every shot on the
// machine is the palette moving out from under it — which is exactly what happened to
// `tools/keys-assert.mjs`, whose accent still reads `#00b5ff` against `theme.rs`'s `#00bfff`. Read
// off the rows on every commit, that cannot happen quietly.
const THEME = fs.readFileSync(path.join(ROOT, 'quill-engine', 'src', 'theme.rs'), 'utf8');
function role(scheme, name) {
  const row = new RegExp(`\\(Scheme::${scheme}, Role::${name}, "#([0-9a-f]{6})"\\)`).exec(THEME);
  assert.ok(row, `theme.rs names no ${name} for ${scheme}`);
  return [0, 2, 4].map((i) => parseInt(row[1].slice(i, i + 2), 16));
}

// `Caret::alpha`'s `GHOST`, taken from the judged state that is measured on it rather than typed
// again here — the same number the ghost rule above is given.
const GHOST = resolveStates(states, 'caret').find((s) => s.name === 'unfocused').assert.alpha;

ok('the accent the harness looks for is the accent the app paints', () => {
  for (const scheme of ['Light', 'Dark']) {
    assert.deepEqual(role(scheme, 'Accent'), [ACCENT.r, ACCENT.g, ACCENT.b],
      `tools/harness.mjs looks for ${ACCENT_HEX}, which is not what theme.rs paints on ${scheme}`);
  }
});

ok('a shot proves its caret was lit by the accent in it, and a ghosted one carries none', () => {
  for (const scheme of ['Light', 'Dark']) {
    const paper = role(scheme, 'Paper');
    const accent = role(scheme, 'Accent');

    const lit = painted({ paper, bar: accent });
    assert.equal(carriesAccent(lit), true, `a ${scheme} bar at full alpha is the accent, exactly`);
    assert.equal(accentPixels(decodePng(lit)), 120, 'the whole bar counts, and nothing else does');

    // The frame the race produces, and every rung between it and the bar. A composite is the
    // accent's own hue under an alpha, so nothing short of full alpha may read as lit — the ghost
    // least of all. `theme-r2`'s lost `theme/dark` measured 444 px of `#124b5e`, which is this at
    // `GHOST` over the dark paper give or take the compositor's own rounding, and no accent at all;
    // its `theme/light` and both of `theme-r3`'s measured 444 px of the accent.
    for (const alpha of [GHOST, 0.1, 0.5, 0.9, 0.99]) {
      const dim = painted({ paper, bar: over(alpha, accent, paper) });
      assert.equal(carriesAccent(dim), false, `the ${scheme} accent at alpha ${alpha} read as lit`);
    }

    // A page with no bar on it at all is not lit either — the empty frame #166 lost `page/empty` to
    // is caught by the same question, and never by a bar-finder that would report "no bar" and stop.
    assert.equal(carriesAccent(painted({ paper, bar: paper })), false, `a bare ${scheme} page is not lit`);
  }
});

ok('every judged state that draws a determined caret is held to one, and no other state is', () => {
  const wants = {};
  for (const piece of Object.keys(states.pieces)) {
    for (const s of resolveStates(states, piece)) {
      // `active` is read the way `tools/judge.mjs` reads it when it calls `shoot`, so the two
      // cannot drift into disagreeing about which states this is asked of.
      wants[`${piece}/${s.name}`] = wantsLitCaret(quillArgv(ROOT, s.flags), { active: s.flags.active !== false });
    }
  }
  // The four ways out a judged state has, and nothing else: focus parked elsewhere, `--nocaret`,
  // a selection, which paints a band where the bar would be, and a Full pane — `--preview full` or
  // `--preview pdf-full` — which puts the rendered page or the page column where the Editor's
  // scroller was and leaves no Editor on the glass to draw a bar. Every other state draws the bar,
  // both Split states — which keep their Editor — included.
  // The two Focus states take the `--nocaret` way out for the reason `theme/dark` does: they are
  // crops of the Design oracle, whose own captures carry no bar, and the state is about which
  // words are dim rather than where the caret is (#113). The two `files` states take it because the
  // Parity oracle measures the bar of a Library-opened document at one of two places depending on
  // when it is asked, one shot in three, and those states are about the sidebar beside the page.
  // `export/dialog` takes it for a reason of its own: the dialog is a surface over the page and the
  // keyboard is the dialog's while it is up, so the Editor under it draws the ghost by rights.
  // Every `style` state takes it, and for the Piece's own reason: the marks are read as lines of
  // ink, and a bar standing in a line of prose joins its band to the strikes' — what the caret is
  // made of is the caret Piece's rows, not this one's. The two Focus states still place the caret,
  // because that is what Focus scopes its sentence from; they only decline to draw it.
  // The two all-Category Syntax states that took a `mac-native` opponent in #319 take it because
  // their captures carry no bar to pair against — measured, not assumed: zero accent pixels in
  // both `308-original-mbp-{light,dark}-syntax-all`. Their sibling `syntax/focus-sentence` draws
  // one, because its capture does, at the same word: 430 accent pixels standing after `more`.
  const exempt = Object.entries(wants).filter(([, held]) => !held).map(([name]) => name).sort();
  assert.deepEqual(exempt, [
    'caret/selection', 'caret/unfocused', 'export/dialog', 'files/library', 'files/search',
    'focus/paragraph', 'focus/sentence',
    'markup/blocks', 'markup/gutters', 'markup/wrapped', 'preview/full', 'preview/pdf-full',
    'style/fillers-light', 'style/focus-dark', 'style/focus-light', 'style/on-dark',
    'style/on-light', 'style/select-dark', 'style/select-light', 'style/syntax-dark',
    'style/syntax-light',
    'syntax/all-dark', 'syntax/all-light',
    'theme/dark', 'theme/light', 'type/mono',
  ]);
  // #197 came out of `theme/dark`, which has since gone `--nocaret` (#198) so that its marks can be
  // read with no bar among them. The rule it left behind is held by the states that still draw one.
  assert.equal(wants['caret/caret'], true, 'the caret Piece is held to the bar it is about');
  assert.equal(wants['chrome/empty'], true, 'an empty Document still draws a caret, and #166 lost it');

  // The way out no judged state takes: a Live launch has no `--deterministic`, so its
  // caret is meant to be dark half the time and there is no lit frame to insist on. `tools/gate
  // keys` is the one caller that opens ours that way.
  const live = quillArgv(ROOT, resolveStates(states, 'page').find((s) => s.name === 'light').flags, { live: true });
  assert.equal(wantsLitCaret(live), false, 'a blinking caret cannot be held to a lit frame');
});

ok('a state that names an assertion carries no such flag, and wants no frozen opponent', () => {
  const made = {
    defaults: { ...states.defaults, w: 200, h: 100, scale: 2 },
    pieces: { synthetic: { lone: { active: false, assert: { kind: 'ghost', alpha: 0.3 } } } },
  };
  const [s] = resolveStates(made, 'synthetic');
  // Like `opponent`, it says how the state is answered rather than what it is shot at, so it must
  // never reach the flags — `unservable` would refuse the Piece before a window opened.
  assert.deepEqual(unservable(made.defaults, s.flags), []);
  assert.deepEqual(s.assert, { kind: 'ghost', alpha: 0.3 });
  assert.equal(s.opponent, null);
  // And unlike `opponent` it does not force Mono: there is no second app's grid to line up with.
  assert.equal(s.flags.font, states.defaults.font);
  assert.equal(s.flags.active, false);
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

// ---------- the same pixels are not judged twice ----------

ok('a round names its shots under shots/<piece>/, whole for a Parity state and cut for a Design oracle one', () => {
  const whole = shotPaths('focus', 4, 'sentence', null);
  assert.equal(whole.shot, 'shots/focus/r4-sentence-ours.png');
  assert.equal(whole.ours, whole.shot);
  assert.equal(whole.theirs, 'shots/oracle/focus/sentence.png');
  assert.equal(whole.lit, 'shots/focus/r4-sentence-ours-lit.png');
  const cut = shotPaths('focus', 4, 'sentence', { crop: [0, 0, 1, 1] });
  assert.equal(cut.shot, whole.shot, 'the whole shot stays beside its crop');
  assert.equal(cut.ours, 'shots/focus/r4-sentence-ours-crop.png');
  assert.equal(cut.theirs, 'shots/focus/r4-sentence-theirs-crop.png');
});

ok('a state carries the latest verdict on the same bytes, both sides, and nothing else', () => {
  inTemp((root) => {
    const write = (file, bytes) => { fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true }); fs.writeFileSync(path.join(root, file), bytes); return file; };
    const ours3 = write('shots/focus/r3-sentence-ours.png', 'ours-a');
    const ours4 = write('shots/focus/r4-sentence-ours.png', 'ours-a');
    const ours5 = write('shots/focus/r5-sentence-ours.png', 'ours-b');
    const theirs = write('shots/oracle/focus/sentence.png', 'theirs-a');
    const now = write('shots/focus/r6-sentence-ours.png', 'ours-a');
    const verdict = (n, ours, extra = {}) => ({
      piece: 'focus', round: n, opponent: 'oracle', winner: 'ours',
      states: [{ name: 'sentence', ours, theirs, blind: 'shots/blind/focus/sentence', oursWas: 'A', pick: 'A', winner: 'ours', margin: n === 3 ? 'clear' : 'slight', sameViewport: true, gap: `g${n}`, gapTheirs: `t${n}`, verdict: `v${n}`, secondary: [], ...extra }],
    });
    const r3 = verdict(3, ours3);
    // Round 4 carried round 3: its state is round 3's verdict under round 4's own shot.
    const r4 = { ...verdict(4, ours4), states: [{ ...r3.states[0], ours: ours4, carried: 3 }] };
    const r5 = verdict(5, ours5);

    const fromLatest = carriedFrom(root, [r3, r4], 'sentence', now, theirs);
    assert.equal(fromLatest.round, 3, 'a carried verdict points at the round whose critic looked, not the round that carried it');
    assert.equal(fromLatest.state.verdict, 'v3');
    assert.deepEqual(Object.keys(fromLatest.state).filter((k) => VERDICT_KEYS.includes(k)).sort(), [...VERDICT_KEYS].sort(), 'every key a verdict is made of is there to copy');

    assert.equal(carriedFrom(root, [r3, r4, r5], 'sentence', now, theirs).round, 3, 'pixels that moved and moved back are the pixels round 3 judged, whatever round 5 saw');
    assert.equal(carriedFrom(root, [r3, r4, r5], 'sentence', ours5, theirs).round, 5, 'and the latest round to have judged these bytes is the one carried');
    assert.equal(carriedFrom(root, [r3], 'sentence', ours5, theirs), null, 'ours moved');
    assert.equal(carriedFrom(root, [r3], 'sentence', now, write('shots/oracle/focus/other.png', 'theirs-b')), null, 'the opponent moved');
    assert.equal(carriedFrom(root, [r3], 'paragraph', now, theirs), null, 'another state');
    assert.equal(carriedFrom(root, [{ ...r3, states: [{ ...r3.states[0], pick: undefined, margin: 'asserted' }] }], 'sentence', now, theirs), null, 'an assertion is arithmetic and is run again');
    assert.equal(carriedFrom(root, [{ piece: 'focus', round: 1, winner: 'ours' }], 'sentence', now, theirs), null, 'a gauntlet round has no states to carry');
    assert.equal(carriedFrom(root, [r3], 'sentence', 'shots/focus/nosuch.png', theirs), null, 'a file that is not there is not the same bytes');
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

// `saturation_stress` is recorded and not scored, and that exempts it from the budget alone: a
// whole run is still fourteen regimes, and every one of them still accounts for its keys.
ok('an unscored regime is still one of the fourteen, and still has to account for its keys', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-judge-unscored-'));
  const rows = regimes().map((r) => (r.scored === false
    ? { regime: r.name, mean_ms: 24.83, worst_ms: 33.89, p50_ms: 25.08, p99_ms: 31.2, cold_ms: 149, scored: false, pass: null }
    : { regime: r.name, mean_ms: 2, worst_ms: 8, p50_ms: 2, p99_ms: 7, cold_ms: 120, pass: true }));
  const body = (extra) => ({
    ran: '--all', headline: 'prose_end_of_draft', regimes: rows,
    regimes_not_run: [], regimes_unaccounted_for: [],
    regimes_not_scored: regimes().filter((r) => r.scored === false).map((r) => r.name),
    pass: true, lines: ['gate bench --all: pass'], ...extra,
  });

  const eleven = path.join(tmp, 'summary-20260901T000001.json');
  fs.writeFileSync(eleven, JSON.stringify(body({ regimes: rows.filter((r) => r.scored !== false) })));
  const short = gate('judge', 'latency', '--summary', eleven);
  assert.equal(short.code, 3, short.out);
  assert.match(lastLine(short), /^gate judge latency: refused \(.* is not a whole run — saturation_stress missing\)/,
    'thirteen scored regimes without the fourteenth recorded is not a whole run');

  const stray = path.join(tmp, 'summary-20260901T000002.json');
  fs.writeFileSync(stray, JSON.stringify(body({ regimes_unaccounted_for: ['saturation_stress'], pass: false })));
  const unaccounted = gate('judge', 'latency', '--summary', stray);
  assert.equal(unaccounted.code, 3, unaccounted.out);
  assert.match(lastLine(unaccounted), /^gate judge latency: refused \(.* could not account for every keystroke in saturation_stress\)/,
    'not scored is not the same as not counted');

  // A run the bench refused because the pointer left the window (#327) is whole and accounted for,
  // and still not a verdict: the keys after the leave were paced by the chrome's fade.
  const paced = path.join(tmp, 'summary-20260901T000003.json');
  fs.writeFileSync(paced, JSON.stringify(body({ regimes_the_pointer_left: ['revision'], pass: false })));
  const left = gate('judge', 'latency', '--summary', paced);
  fs.rmSync(tmp, { recursive: true, force: true });
  assert.equal(left.code, 3, left.out);
  assert.match(lastLine(left), /^gate judge latency: refused \(.* had the pointer leave the window during revision\)/,
    'accounted for is not the same as measured');

  // The whole body as written — fourteen regimes, saturation over every bar and marked unscored — is
  // deliberately not run: judge would take a verdict from it and write a round, and writing a round
  // into the ledger is not something a test may do. That it would is `tools/bench-selftest.mjs`'s
  // to check, in `latencyVerdict`.
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
    'a whole run of fourteen inside every bar is still not evidence when it came off the panel');

  // The same body without the mark is deliberately not run here. It is whole, accounted for and
  // inside every bar, so judge would take a verdict from it and write a round — and writing a round
  // into the ledger is not something a test may do. That it would is the point: the mark is the
  // only thing standing between a panel run and the latency Piece.
});

// `chrome` was this case until the tools learnt `typing` and `menu`, and `files` until they learnt
// `library`, `sidebar` and `search`. No state in states.json names a flag the defaults lack now, and
// the defaults are the whole of the rule — so the flag is invented and put in front of the command
// through QUILL_STATES, which is also what keeps this case off whichever Piece is waiting on a spec
// this month. The refusal is `checkStates`'s, before the build and before a window.
function unservableStates(file) {
  const states = JSON.parse(fs.readFileSync(path.join(ROOT, 'shots/oracle/states.json'), 'utf8'));
  states.pieces.type = { duo: { chrome: 'off', sepia: true }, quattro: { chrome: 'off', grain: 3 } };
  fs.writeFileSync(file, JSON.stringify(states));
  return { QUILL_STATES: file };
}

ok('a Piece whose states need flags the app has not got names them and judges nothing', () => {
  const file = path.join(os.tmpdir(), `quill-judge-selftest-unservable-${process.pid}.json`);
  try {
    const r = gate('judge', 'type', unservableStates(file));
    assert.equal(r.code, 3, r.err);
    assert.match(r.err, /state duo names sepia/);
    assert.match(r.err, /state quattro names grain/);
    assert.match(lastLine(r), /^gate judge type: refused \(2 of 2 states name flags the app has not got\)/);
  } finally {
    fs.rmSync(file, { force: true });
  }
});

ok('a refused run is one line on stdout, and what it said is on stderr and in its log', () => {
  const file = path.join(os.tmpdir(), `quill-judge-selftest-log-${process.pid}.json`);
  const log = path.join(ROOT, 'target/gate/judge-type.log');
  fs.rmSync(log, { force: true });
  try {
    const r = gate('judge', 'type', unservableStates(file));
    assert.deepEqual(r.out.trim().split('\n').length, 1, `stdout was more than the owner's line:\n${r.out}`);
    assert.match(r.err, /state duo names sepia/);
    assert.match(fs.readFileSync(log, 'utf8'), /state duo names sepia/);
  } finally {
    fs.rmSync(file, { force: true });
  }
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
