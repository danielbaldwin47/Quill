// Does `tools/critic-replay.mjs` replay only the pairs its rounds' critics were shown? — the
// replay's own test.
//
//   node tools/critic-replay-selftest.mjs
//
// A replay puts recorded pairs to fresh critics and counts agreement with the recorded verdicts,
// so a pair that is not the one a critic saw would count a disagreement that never happened. The
// shots are evidence outside git (dev/README.md § Judging evidence): a file can be missing, or be
// another worktree's shot of the same name. `replayable` is held here on a scratch root to skip
// both, to read a round without hashes off its files as before, and to leave carried and
// asserted states out. Every hash is computed by judge.mjs's own `shotHash`, never written here.
// No critic runs; no window.

import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { replayable } from './critic-replay.mjs';
import { shotHash } from './judge.mjs';

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`critic-replay selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`critic-replay selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'critic-replay-selftest-'));
try {
  const write = (file, bytes) => {
    fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    fs.writeFileSync(path.join(root, file), bytes);
    return file;
  };
  const theirs = write('dev/shots/oracle/focus/sentence.png', 'theirs');
  const state = (name, ours, extra = {}) => ({ name, ours, theirs, pick: 'A', winner: 'ours', ...extra });
  const hashed = (name, ours) => state(name, ours, { oursHash: shotHash(root, ours), theirsHash: shotHash(root, theirs) });
  const round = (n, states) => ({ piece: 'focus', round: n, opponent: 'oracle', states });

  const kept = write('dev/shots/focus/r1-kept-ours.png', 'kept');
  const swapped = write('dev/shots/focus/r1-swapped-ours.png', 'judged');
  const gone = write('dev/shots/focus/r1-gone-ours.png', 'gone');
  const old = write('dev/shots/focus/r1-old-ours.png', 'old');
  const r1 = round(1, [
    hashed('kept', kept),
    hashed('swapped', swapped),
    hashed('gone', gone),
    state('old', old),
    state('asserted', kept, { pick: undefined, margin: 'asserted' }),
  ]);
  const r2 = round(2, [state('kept', kept, { carried: 1 }), hashed('fresh', write('dev/shots/focus/r2-fresh-ours.png', 'fresh'))]);
  write(swapped, 'another worktree');
  fs.rmSync(path.join(root, gone));
  const names = (recorded, count) => replayable(root, recorded, count).tasks.map((t) => `r${t.round} ${t.state.name}`);

  ok('a hashed pair is replayed only while its files hold the bytes the round hashed', () => {
    const got = names([r1], 1);
    assert.ok(got.includes('r1 kept'), got.join(', '));
    assert.ok(!got.includes('r1 swapped'), 'a file swapped under the round is not the pair its critic saw');
    assert.ok(!got.includes('r1 gone'), 'a file that is not there cannot be replayed');
  });

  ok('a round from before the hashes is read off its files, as before', () => {
    assert.ok(names([r1], 1).includes('r1 old'));
  });

  ok('carried and asserted states are not replayed', () => {
    const got = names([r1, r2], 2);
    assert.ok(!got.includes('r1 asserted'), got.join(', '));
    assert.ok(!got.includes('r2 kept'), 'a carried verdict is replayed at the round that judged it');
    assert.ok(got.includes('r2 fresh'), got.join(', '));
  });
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}

console.log(failures ? `critic-replay selftest: fail (${failures} of ${cases})` : `critic-replay selftest: pass (${cases} cases)`);
process.exit(failures ? 1 : 0);
