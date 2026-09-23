// Do `tools/assets.mjs link` and `sync` move the judging evidence between a worktree
// and the main checkout? — the evidence tool's own test.
//
//   node tools/assets-selftest.mjs
//
// Two things are held. The tool's test for evidence agrees with this repository's
// .gitignore, path by path, over every file the repository tracks under the
// evidence roots and a set of paths the rule has to tell apart (a blind pair is
// ignored for its own reason and is not evidence, so none is probed) — so a pattern
// added to one and not the other goes red here. And on a scratch repository with
// this .gitignore, a worktree gets a link to each piece of evidence it lacks and
// nothing over a file it has, and `sync` copies back what the worktree wrote itself,
// leaves the links, and never overwrites the main checkout's copy. No window.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isEvidence, link, sync } from './assets.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`assets selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`assets selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

const git = (dir, ...args) => execFileSync('git', ['-C', dir, ...args], { encoding: 'utf8' });

// Whether git itself ignores `rel` in `dir`, tracked or not: check-ignore with
// --no-index reads the patterns alone.
function ignored(dir, rels) {
  let out = '';
  try {
    out = execFileSync('git', ['-C', dir, 'check-ignore', '--no-index', '--stdin'], {
      input: rels.join('\n'),
      encoding: 'utf8',
    });
  } catch (e) {
    out = e.stdout ?? '';
  }
  return new Set(out.split('\n').filter(Boolean));
}

ok('the rule agrees with .gitignore', () => {
  const tracked = git(ROOT, 'ls-files', '--', 'dev/shots', 'dev/progress/diagnostics', 'dev/ref/ia').split('\n').filter(Boolean);
  const probes = [
    'dev/shots/chrome/r99-bars-ours.png',
    'dev/shots/latency/r9-bench.json',
    'dev/shots/oracle/chrome/bars.png',
    'dev/progress/diagnostics/ticket-1/round-1/records.tar.gz',
    'dev/progress/diagnostics/ticket-1/round-1/notes.md',
    'dev/ref/ia/shots/mac-native/mac-native-99-new.png',
    'dev/ref/ia/shots/still.webp',
    'dev/ref/ia/shots/narrow-344.json',
    'dev/ref/ia/templates/any/template.css',
    'dev/ref/ia/sources/any.txt',
    'dev/ref/ia/fonts/Duo/iAWriterDuoV.ttf',
    'dev/ref/ia/mac-native/NOTES.md',
  ];
  const all = [...tracked, ...probes];
  const byGit = ignored(ROOT, all);
  const wrong = all.filter((rel) => isEvidence(rel) !== byGit.has(rel));
  assert.deepEqual(wrong, [], 'isEvidence and .gitignore disagree on these');
});

const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'assets-selftest-'));
try {
  const main = path.join(scratch, 'main');
  const wt = path.join(scratch, 'wt');
  fs.mkdirSync(main);
  git(main, 'init', '-q', '-b', 'main');
  fs.copyFileSync(path.join(ROOT, '.gitignore'), path.join(main, '.gitignore'));
  const put = (dir, rel, body) => {
    fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
    fs.writeFileSync(path.join(dir, rel), body);
  };
  put(main, 'dev/shots/oracle/chrome/bars.png', 'oracle');
  put(main, 'dev/shots/chrome/state.json', '{}');
  git(main, 'add', '.');
  git(main, '-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-qm', 'seed');
  put(main, 'dev/shots/chrome/r1-bars-ours.png', 'round one');
  put(main, 'dev/ref/ia/shots/mac-native/cap.png', 'capture');
  put(main, 'dev/shots/blind/chrome/bars/A.png', 'a blind copy');
  git(main, 'worktree', 'add', '-q', wt);

  ok('link gives a worktree the evidence it lacks, and only that', () => {
    const r = link(wt);
    assert.equal(r.linked, 2);
    for (const rel of ['dev/shots/chrome/r1-bars-ours.png', 'dev/ref/ia/shots/mac-native/cap.png']) {
      assert.ok(fs.lstatSync(path.join(wt, rel)).isSymbolicLink(), `${rel} is a link`);
      assert.equal(fs.readlinkSync(path.join(wt, rel)), path.join(main, rel));
    }
    assert.ok(!fs.lstatSync(path.join(wt, 'dev/shots/oracle/chrome/bars.png')).isSymbolicLink(), 'a tracked shot stays the checkout\'s');
    assert.ok(!fs.existsSync(path.join(wt, 'dev/shots/blind/chrome/bars/A.png')), 'a blind pair is not evidence');
    assert.equal(git(wt, 'status', '--porcelain'), '', 'the links leave the worktree clean');
    assert.equal(link(wt).linked, 0, 'a second link finds nothing to do');
  });

  ok('sync copies what the worktree wrote, and nothing else', () => {
    put(wt, 'dev/shots/chrome/r2-bars-ours.png', 'round two');
    put(wt, 'dev/shots/chrome/r1-theirs-crop.png', 'the worktree\'s');
    put(main, 'dev/shots/chrome/r1-theirs-crop.png', 'the main checkout\'s');
    const r = sync(wt);
    assert.equal(r.copied, 1);
    assert.deepEqual(r.held, ['dev/shots/chrome/r1-theirs-crop.png']);
    assert.equal(fs.readFileSync(path.join(main, 'dev/shots/chrome/r2-bars-ours.png'), 'utf8'), 'round two');
    assert.equal(fs.readFileSync(path.join(main, 'dev/shots/chrome/r1-theirs-crop.png'), 'utf8'), 'the main checkout\'s');
  });

  ok('both do nothing in the main checkout', () => {
    assert.equal(link(main).here, true);
    assert.equal(sync(main).here, true);
  });
} finally {
  fs.rmSync(scratch, { recursive: true, force: true });
}

console.log(failures ? `assets selftest: fail (${failures} of ${cases})` : `assets selftest: pass (${cases} cases)`);
process.exit(failures ? 1 : 0);
