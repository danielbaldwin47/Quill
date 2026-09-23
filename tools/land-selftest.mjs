// Does `tools/land` still end in the line its --help promises? — the land tool's own test.
//
//   node tools/land-selftest.mjs
//
// Landing is irreversible — a merge, a deleted branch, a comment on a ticket —
// so the tool is driven here in --dry-run, which runs the read-only steps it
// decides by and prints the rest as `would:` lines. `gh`, `git` and the main
// checkout's own `tools/context-report` are faked on PATH: each scenario is a
// directory of the exact bytes the real commands would print, and the fake
// `git rev-parse --git-common-dir` points the tool at a scratch root, so
// nothing here touches this repository. Every expectation is read back out of
// the scenario that produced it — the branch name out of the `gh pr view` row,
// the discarded files out of the `git status` rows — rather than written twice.
//
// The cases are the landing sequence's own history (docs/agents/context.md
// § What the steps cost when skipped): a green PR, a PR that is not this tool's
// to land, the worktree with untracked bench files that refused `git worktree
// remove` at every landing, the worktree whose shot clashes with the main
// checkout's (tools/assets.mjs), the remote branch `--delete-branch` had already
// deleted, and the ticket `tools/context-report` can find no session for.

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const LAND = path.join(ROOT, 'tools/land');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases++;
  try {
    body();
    console.log(`land selftest: ${name} ok`);
  } catch (e) {
    failures++;
    console.log(`land selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

// One scenario: a scratch directory holding what the fakes print, a scratch
// main checkout for the tool to find, and `gh`, `git` and `tools/context-report`
// written into it. `scene` names the files by the command that reads them.
let scenes = 0;
function scene(files) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `land-selftest-${process.pid}-${scenes++}-`));
  const main = path.join(dir, 'root');
  fs.mkdirSync(path.join(main, 'tools'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'bin'));
  for (const [name, text] of Object.entries(files)) fs.writeFileSync(path.join(dir, name), text);

  // The fakes answer only the calls tools/land makes in --dry-run, and ignore
  // the `-q` template and the `-C` directory: what they print is the scenario.
  const write = (file, text) => {
    fs.writeFileSync(file, text);
    fs.chmodSync(file, 0o755);
  };
  write(path.join(dir, 'bin/gh'), `#!/usr/bin/env bash
if [ "$1" = pr ] && [ "$2" = view ]; then cat "$LAND_FAKE/pr.tsv"; exit 0; fi
echo "fake gh: $*" >&2; exit 0
`);
  write(path.join(dir, 'bin/git'), `#!/usr/bin/env bash
args=("$@")
if [ "\${args[0]}" = -C ]; then args=("\${args[@]:2}"); fi
case "\${args[0]} \${args[1]}" in
  "rev-parse --path-format=absolute") echo "$LAND_FAKE/root/.git" ;;
  "worktree list") cat "$LAND_FAKE/worktrees.txt" ;;
  "status --porcelain") cat "$LAND_FAKE/status.txt" ;;
  "branch --list") cat "$LAND_FAKE/local.txt" ;;
  "ls-remote --heads") cat "$LAND_FAKE/remote.txt" ;;
  *) echo "fake git: \${args[*]}" >&2 ;;
esac
exit 0
`);
  write(path.join(main, 'tools/context-report'), `#!/usr/bin/env bash
if [ ! -s "$LAND_FAKE/report.txt" ]; then exit 1; fi
cat "$LAND_FAKE/report.txt"
`);
  // `node tools/assets.mjs check <worktree>`: the clash line and exit 1 when the
  // scenario has one, the clean line otherwise.
  write(path.join(main, 'tools/assets.mjs'), `import fs from 'node:fs';
const clash = fs.readFileSync(process.env.LAND_FAKE + '/clash.txt', 'utf8').trim();
console.log(clash ? 'assets check: ' + clash : 'assets check: no clash, 0 new to copy');
process.exit(clash ? 1 : 0);
`);
  return { dir, main };
}

// `tools/land <pr> <ticket> --dry-run` over one scenario, as { lines, status }.
function land(sc, pr, ticket) {
  const run = spawnSync(LAND, [String(pr), String(ticket), '--dry-run'], {
    encoding: 'utf8',
    env: { ...process.env, LAND_FAKE: sc.dir, PATH: `${path.join(sc.dir, 'bin')}:${process.env.PATH}` },
  });
  return { lines: run.stdout.trimEnd().split('\n'), status: run.status, stderr: run.stderr };
}

// The line a step printed, by its name.
const step = (out, name) => out.lines.find((l) => l.startsWith(`${name}: `));
const woulds = (out) => out.lines.filter((l) => l.startsWith('would: '));

// A `gh pr view` row, and the worktree list that goes with a branch.
const row = (state, mergeable, mergeState, base, head) => `${state}\t${mergeable}\t${mergeState}\t${base}\t${head}\n`;
const worktrees = (main, wt, head) =>
  `worktree ${main}\nHEAD 1111111111111111111111111111111111111111\nbranch refs/heads/main\n\n` +
  (wt ? `worktree ${wt}\nHEAD 2222222222222222222222222222222222222222\nbranch refs/heads/${head}\n\n` : '');

const GREEN = row('OPEN', 'MERGEABLE', 'CLEAN', 'main', 'worktree-ticket-401-land');
const REPORT = 'context: peak 118k tokens, 57 tool calls, 1 subagent (peak 61k)';
const head = (pr) => pr.trimEnd().split('\t')[4];

// The scenario every case starts from: the green PR, its worktree clean, both
// branches still there, and a session context-report can price.
function base(extra = {}) {
  const files = {
    'pr.tsv': GREEN,
    'status.txt': '',
    'local.txt': `  ${head(GREEN)}\n`,
    'remote.txt': `3333333333333333333333333333333333333333\trefs/heads/${head(GREEN)}\n`,
    'report.txt': `${REPORT}\n`,
    'clash.txt': '',
    'worktrees.txt': '',
    ...extra,
  };
  const sc = scene(files);
  if (!files['worktrees.txt']) {
    fs.writeFileSync(path.join(sc.dir, 'worktrees.txt'), worktrees(sc.main, path.join(sc.dir, 'wt'), head(files['pr.tsv'])));
  }
  return sc;
}

ok('a green PR runs every step and ends in `land <PR>: done`', () => {
  const sc = base();
  const out = land(sc, 401, 402);
  assert.equal(out.lines[out.lines.length - 1], 'land 401: done');
  assert.equal(out.status, 0);
  for (const name of ['check', 'merge', 'pull', 'gate', 'worktree', 'branch', 'report']) {
    assert.ok(step(out, name), `no ${name} line in:\n${out.lines.join('\n')}`);
  }
  assert.ok(step(out, 'check').includes(head(GREEN)), 'the check line does not name the PR branch');
  assert.ok(woulds(out).some((l) => l === 'would: gh pr merge 401 --merge --delete-branch'));
  assert.ok(woulds(out).some((l) => l.includes('gh issue comment 402 --body-file')));
});

ok('a PR against anything but main is refused, and nothing is even offered', () => {
  const pr = row('OPEN', 'MERGEABLE', 'CLEAN', 'spec-356', 'worktree-ticket-401-land');
  const out = land(base({ 'pr.tsv': pr }), 401, 402);
  assert.equal(out.lines[out.lines.length - 1], `land 401: refused (PR #401 is against ${pr.trimEnd().split('\t')[3]}, not main)`);
  assert.equal(out.status, 3);
  assert.deepEqual(woulds(out), []);
});

ok('a worktree with untracked files is removed with --force, its line naming each', () => {
  const dirty = ['?? dev/shots/latency/bench-1757.json', '?? dev/shots/latency/bench-1758.json', ' M quill/src/main.rs'];
  const sc = base({ 'status.txt': `${dirty.join('\n')}\n` });
  const out = land(sc, 401, 402);
  const line = step(out, 'worktree');
  assert.ok(line.includes('--force'), `the worktree line does not pass --force: ${line}`);
  assert.ok(line.endsWith(`discarding ${dirty.map((d) => d.slice(3)).join(', ')}`), line);
  assert.ok(woulds(out).some((l) => l === `would: git -C ${sc.main} worktree remove --force ${path.join(sc.dir, 'wt')}`));
  assert.equal(out.lines[out.lines.length - 1], 'land 401: done');
});

ok('a worktree shot that clashes with the main checkout\'s is refused before the merge, naming it', () => {
  const clash = '1 differ from the main checkout\'s copy: dev/shots/chrome/r12-bars-ours.png';
  const out = land(base({ 'clash.txt': `${clash}\n` }), 401, 402);
  assert.equal(out.lines[out.lines.length - 1], `land 401: refused (${clash})`);
  assert.equal(out.status, 3);
  assert.deepEqual(woulds(out), []);
});

ok('a remote branch already deleted is not pushed at, and the land still ends done', () => {
  const sc = base({ 'remote.txt': '' });
  const out = land(sc, 401, 402);
  assert.equal(step(out, 'branch'), `branch: ${head(GREEN)}: local deleted, remote already gone`);
  assert.deepEqual(woulds(out).filter((l) => l.includes('push origin --delete')), []);
  assert.ok(woulds(out).some((l) => l === `would: git -C ${sc.main} branch -D ${head(GREEN)}`));
  assert.equal(out.lines[out.lines.length - 1], 'land 401: done');
});

ok('context-report finding no session is one line in the comment, not a failed land', () => {
  const sc = base({ 'report.txt': '' });
  const out = land(sc, 401, 402);
  const said = 'context: tools/context-report found no session for #402';
  assert.equal(step(out, 'report'), `report: #402 <- ${said}`);
  assert.equal(fs.readFileSync(path.join(sc.main, 'target/land/comment-402.md'), 'utf8'), `${said}\n`);
  assert.equal(out.lines[out.lines.length - 1], 'land 401: done');
  // And the same land with a session found posts what context-report printed.
  const found = land(base(), 401, 402);
  assert.equal(step(found, 'report'), `report: #402 <- ${REPORT}`);
});

console.log(`land selftest: ${cases - failures} of ${cases} ok`);
process.exit(failures === 0 ? 0 : 1);
