// Does `tools/gate shoot` refuse what it should, before it builds, and keep out of shots/? — the
// looked-at shot's own test.
//
//   node tools/shoot-selftest.mjs
//
// The command's real work — the build, the stage, the shot — is the judge's, shared through
// `preflight` and `shootState` and held by tools/judge-selftest.mjs. What is this command's own is
// checked here: where its files go (never under shots/, which is a round's), what it says beside a
// shot, and the refusals it gives before a window opens. No window, no build.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { readStates } from './oracle.mjs';
import { OUT, shootablePieces, shotLine, shotPaths, standing } from './shoot.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const GATE = path.join(ROOT, 'tools', 'gate');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`shoot selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`shoot selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

function gate(...argv) {
  try {
    return { code: 0, out: execFileSync(GATE, argv, { cwd: ROOT, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }) };
  } catch (e) {
    return { code: e.status, out: e.stdout || '' };
  }
}
const lastLine = (r) => r.out.trim().split('\n').pop();

ok('a looked-at shot lives under target/, and a crop state cuts both sides beside it', () => {
  assert.ok(OUT.startsWith('target/'), OUT);
  const whole = shotPaths('focus', 'sentence', null);
  assert.equal(whole.shot, 'target/gate/shoot/focus/sentence-ours.png');
  assert.equal(whole.ours, whole.shot, 'a Parity state is paired whole');
  assert.equal(whole.theirs, 'shots/oracle/focus/sentence.png', 'against the frozen oracle, which is read and never written');
  const cut = shotPaths('focus', 'sentence', { crop: [0, 0, 1, 1] });
  assert.equal(cut.ours, 'target/gate/shoot/focus/sentence-ours-crop.png');
  assert.equal(cut.theirs, 'target/gate/shoot/focus/sentence-theirs-crop.png');
  for (const p of [whole.shot, whole.lit, cut.ours, cut.theirs]) assert.ok(!p.startsWith('shots/'), `${p} would land among a round's evidence`);
});

ok('the line beside a shot says what a judge would do with it', () => {
  const prior = { round: 4, state: { winner: 'ours', margin: 'slight' } };
  assert.match(standing(prior, { round: 5 }), /round 4 judged \(ours, slight\); a judge would carry/);
  assert.match(standing(null, { round: 5 }), /not the pixels round 5 judged; a judge would ask a critic/);
  assert.match(standing(null, null), /no round has judged this state; a judge would ask a critic/);
});

ok('--all is every Piece with judged states but latency, which is a bench run', () => {
  const states = readStates(ROOT);
  const all = shootablePieces(states);
  assert.ok(all.length, 'states.json names Pieces to shoot');
  assert.deepEqual([...all, 'latency'].sort(), Object.keys(states.pieces).sort());
});

ok("a Piece's last line and the run's are one shape, counted in states and Pieces", () => {
  assert.equal(shotLine(1, 0), '1 state (0 unchanged since their last round)');
  assert.equal(shotLine(3, 2), '3 states (2 unchanged since their last round)');
  assert.equal(shotLine(1, 1, 1), '1 state over 1 Piece (1 unchanged since their last round)');
  assert.equal(shotLine(7, 4, 3), '7 states over 3 Pieces (4 unchanged since their last round)');
});

ok('a run of several Pieces refuses each by its line, then the run by the count, before anything is built', () => {
  const r = gate('shoot', '--pieces', 'nosuchpiece,latency');
  assert.equal(r.code, 3, r.out);
  const lines = r.out.trim().split('\n');
  assert.match(lines.at(-3), /^gate shoot nosuchpiece: refused \(nosuchpiece is not a Piece with judged states\)$/);
  assert.match(lines.at(-2), /^gate shoot latency: refused \(.*shoots nothing\)$/);
  assert.equal(lines.at(-1), 'gate shoot: refused (2 of 2 Pieces refused: nosuchpiece, latency)');
});

ok('a state named with --all or --pieces is refused, since a state is named for one Piece', () => {
  const withAll = gate('shoot', '--all', 'focus');
  assert.equal(withAll.code, 3, withAll.out);
  assert.equal(lastLine(withAll), 'gate shoot: refused (states are named for one Piece, and focus came with --all)');
  const withPieces = gate('shoot', 'sentence', '--pieces', 'focus');
  assert.equal(withPieces.code, 3, withPieces.out);
  assert.equal(lastLine(withPieces), 'gate shoot: refused (states are named for one Piece, and sentence came with --pieces)');
});

ok('--all and --pieces together, and a --pieces with no names, are refused as command lines', () => {
  const both = gate('shoot', '--all', '--pieces', 'focus');
  assert.equal(both.code, 3, both.out);
  assert.equal(both.out, '', 'a command line refused before the log opens prints nothing on stdout');
  const empty = gate('shoot', '--pieces', '');
  assert.equal(empty.code, 3, empty.out);
});

ok('a Piece nobody has judged states for, and the one that shoots nothing, are refused', () => {
  const none = gate('shoot', 'nosuchpiece');
  assert.equal(none.code, 3, none.out);
  assert.match(lastLine(none), /^gate shoot nosuchpiece: refused \(nosuchpiece is not a Piece with judged states\)/);
  const latency = gate('shoot', 'latency');
  assert.equal(latency.code, 3, latency.out);
  assert.match(lastLine(latency), /^gate shoot latency: refused \(.*shoots nothing\)/);
});

ok('a state the Piece has not got is refused by name, before anything is built', () => {
  const r = gate('shoot', 'focus', 'sentence', 'nosuchstate');
  assert.equal(r.code, 3, r.out);
  assert.match(lastLine(r), /^gate shoot focus: refused \(nosuchstate: not a judged state of focus\)/);
});

ok('a command line this cannot read is refused, and --help is not', () => {
  const flag = gate('shoot', 'focus', '--nosuch');
  assert.equal(flag.code, 3, flag.out);
  const help = gate('shoot', '--help');
  assert.equal(help.code, 0, help.out);
  assert.match(help.out, /^usage: tools\/gate shoot/);
});

console.log(`shoot selftest: ${cases - failures} of ${cases} ok`);
process.exit(failures === 0 ? 0 : 1);
