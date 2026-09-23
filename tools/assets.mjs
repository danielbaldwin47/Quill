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

export function sync(worktree) {
  const main = mainCheckout(worktree);
  if (path.resolve(main) === path.resolve(worktree)) return { main, copied: 0, held: [], here: true };
  let copied = 0;
  const held = [];
  for (const rel of evidence(worktree)) {
    const from = path.join(worktree, rel);
    if (fs.lstatSync(from).isSymbolicLink()) continue;
    const to = path.join(main, rel);
    if (exists(to)) {
      if (!fs.readFileSync(to).equals(fs.readFileSync(from))) held.push(rel);
      continue;
    }
    fs.mkdirSync(path.dirname(to), { recursive: true });
    fs.copyFileSync(from, to);
    copied++;
  }
  return { main, copied, held, here: false };
}

function main(argv) {
  const [cmd, dir] = argv;
  if (cmd === 'link') {
    const r = link(path.resolve(dir ?? process.cwd()));
    console.log(r.here ? 'assets link: the main checkout holds the evidence itself' : `assets link: ${r.linked} linked from ${r.main}`);
    return 0;
  }
  if (cmd === 'sync' && dir) {
    const r = sync(path.resolve(dir));
    if (r.here) {
      console.log('assets sync: the main checkout holds the evidence itself');
      return 0;
    }
    const kept = r.held.length ? `; ${r.held.length} differ from the main checkout's and were left: ${r.held.join(', ')}` : '';
    console.log(`assets sync: ${r.copied} copied into ${r.main}${kept}`);
    return 0;
  }
  console.error('usage: node tools/assets.mjs link [<worktree>] | sync <worktree>');
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exit(main(process.argv.slice(2)));
