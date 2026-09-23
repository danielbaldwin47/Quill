// The judging evidence that lives beside the repository rather than in it.
//
//   node tools/assets.mjs link [<worktree>]
//   node tools/assets.mjs sync <worktree>
//
// Round shots, the Design oracle's full captures, iA's stills and the diagnostics
// archives are ignored by git (.gitignore § Judging evidence) and kept in the main
// checkout only; a clone has none of them, and the backup is named in dev/README.md
// § Judging evidence. What the Gate needs to run — the frozen oracle shots, the
// captures states.json crops, the shots the selftests read — stays committed.
//
// A worktree is made from git, so it starts without the evidence. `link` gives it a
// symlink to every piece of evidence the main checkout holds and the worktree lacks,
// so a judge in the worktree finds the rounds it carries from; `tools/gate` runs it
// before every command that shoots or judges. `sync` is the other direction:
// every piece of evidence the worktree wrote itself, a real file rather than one of
// the links, is copied into the main checkout, and `tools/land` runs it before the
// worktree is removed. A file the main checkout already holds is never overwritten.
//
// A tool writing evidence goes through `replaceFile`, which renames a finished file
// over the path: a link is replaced rather than written through, so a worktree's
// shot stays the worktree's until it lands. A **clash** is a file the worktree
// wrote whose name the main checkout already holds with other bytes — two worktrees
// judging one Piece both wrote `r12` — and `sync` names it and leaves the main
// checkout's copy. Neither copy can mislead a later round: a verdict is carried on
// the hashes its round recorded, not on whatever file sits at the path
// (judge.mjs `carriedFrom`).
//
// Each ends in one line: `assets link: ...` or `assets sync: ...`.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// The same files .gitignore § Judging evidence names, as one test on a repo path.
export const EVIDENCE = [
  /^dev\/shots\/(?!oracle\/|blind\/|_).*\.(png|webp)$/,
  /^dev\/progress\/diagnostics\/.*\.(gz|tar)$/,
  /^dev\/ref\/ia\/shots\/.*\.(png|webp)$/,
  /^dev\/ref\/ia\/(templates|sources)\//,
];
const ROOTS = ['dev/shots', 'dev/progress/diagnostics', 'dev/ref/ia'];

export function isEvidence(rel) {
  return EVIDENCE.some((re) => re.test(rel));
}

// The main checkout of the repository `dir` belongs to: the parent of the common git
// directory, the same for every worktree.
export function mainCheckout(dir) {
  const common = execFileSync('git', ['-C', dir, 'rev-parse', '--path-format=absolute', '--git-common-dir'], {
    encoding: 'utf8',
  }).trim();
  return path.dirname(common);
}

// The evidence a checkout holds as ignored files, as repo paths.
export function evidence(dir) {
  const out = execFileSync(
    'git',
    ['-C', dir, 'ls-files', '-z', '--others', '--ignored', '--exclude-standard', '--', ...ROOTS],
    { encoding: 'utf8', maxBuffer: 64 << 20 },
  );
  return out.split('\0').filter((rel) => rel && isEvidence(rel));
}

function exists(file) {
  try {
    fs.lstatSync(file);
    return true;
  } catch {
    return false;
  }
}

export function link(worktree) {
  const main = mainCheckout(worktree);
  if (path.resolve(main) === path.resolve(worktree)) return { main, linked: 0, here: true };
  let linked = 0;
  for (const rel of evidence(main)) {
    const at = path.join(worktree, rel);
    if (exists(at)) continue;
    fs.mkdirSync(path.dirname(at), { recursive: true });
    fs.symlinkSync(path.join(main, rel), at);
    linked++;
  }
  return { main, linked, here: false };
}

// Writes `data` to `file` by renaming a finished copy over it, so a symlink at
// `file` is replaced rather than written through, and a reader never sees half a file.
// The temporary name is not evidence, so one a crash leaves behind shows in `git
// status` rather than being copied into the main checkout by `sync`.
export function replaceFile(file, data) {
  const tmp = `${file}.${process.pid}.tmp`;
  try {
    fs.writeFileSync(tmp, data);
    fs.renameSync(tmp, file);
  } catch (e) {
    fs.rmSync(tmp, { force: true });
    throw e;
  }
}

// The evidence the worktree wrote itself, sorted into what the main checkout lacks
// (`fresh`) and what it holds with other bytes (`clashes`).
export function unsynced(worktree) {
  const main = mainCheckout(worktree);
  if (path.resolve(main) === path.resolve(worktree)) return { main, fresh: [], clashes: [], here: true };
  const fresh = [];
  const clashes = [];
  for (const rel of evidence(worktree)) {
    const from = path.join(worktree, rel);
    if (fs.lstatSync(from).isSymbolicLink()) continue;
    const to = path.join(main, rel);
    if (!exists(to)) fresh.push(rel);
    else if (!fs.readFileSync(to).equals(fs.readFileSync(from))) clashes.push(rel);
  }
  return { main, fresh, clashes, here: false };
}

export function sync(worktree) {
  const r = unsynced(worktree);
  for (const rel of r.fresh) {
    const to = path.join(r.main, rel);
    fs.mkdirSync(path.dirname(to), { recursive: true });
    fs.copyFileSync(path.join(worktree, rel), to);
  }
  return r;
}

const IN_MAIN = 'the main checkout holds the evidence itself';

function main(argv) {
  const [cmd, dir] = argv;
  if (cmd === 'link') {
    const r = link(path.resolve(dir ?? process.cwd()));
    console.log(r.here ? `assets link: ${IN_MAIN}` : `assets link: ${r.linked} linked from ${r.main}`);
    return 0;
  }
  if (cmd === 'sync' && dir) {
    const r = sync(path.resolve(dir));
    const left = r.clashes.length ? `; ${r.clashes.length} left, the main checkout holding other bytes: ${r.clashes.join(', ')}` : '';
    console.log(r.here ? `assets sync: ${IN_MAIN}` : `assets sync: ${r.fresh.length} copied into ${r.main}${left}`);
    return 0;
  }
  console.error('usage: node tools/assets.mjs link [<worktree>] | sync <worktree>');
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exit(main(process.argv.slice(2)));
