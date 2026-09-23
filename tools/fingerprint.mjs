// Which build of the JavaScript app a number or a shot belongs to: one hash, for both of the
// things that have to say so.
//
//   import { appFiles, hashApp } from '../../tools/fingerprint.mjs'
//
// `dev/legacy/tools/latency.mjs` stamps every bench it takes with this, and `tools/gate oracle` stamps
// every frozen shot with it. The two are only worth comparing — is this the build those numbers
// came from? — if they are the same hash over the same files in the same order, so the hash lives
// here and both import it; neither owns a copy, as with the regimes in tools/regimes.mjs.
import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

// The two smaller answers to the same question, kept here beside the big one so that a tool
// stamping something with what produced it has one place to reach for.

// The hash everything here is in.
export function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

// Where a checkout was when something was taken. Null outside one — a tarball is still a place a
// shot can be taken from, and this is a note of where rather than a thing to compare.
export function gitHead(root) {
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}

// The app's own source under `appDir`, sorted, named relative to `root` — which is how they are
// keyed in the hash, so a file that moves changes it. `fonts/` is left out: those are bytes the
// app loads, not the app, and they are megabytes to read for a build that never moves them.
export function appFiles(root, appDir = 'dev/legacy/app') {
  const files = [];
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const f = path.join(d, e.name);
      if (e.isDirectory()) { if (e.name !== 'fonts') walk(f); }
      else if (/\.(js|css|html)$/.test(e.name)) files.push(f);
    }
  };
  walk(path.join(root, appDir));
  return files.sort().map((f) => path.relative(root, f));
}

// A file is keyed without the `dev/` it gained when `legacy/` moved under `dev/`, so that move left
// every frozen oracle's fingerprint standing: the app it hashes did not change.
export function hashApp(root, appDir = 'dev/legacy/app') {
  const files = appFiles(root, appDir);
  const h = crypto.createHash('sha256');
  for (const f of files) {
    const key = f.replace(/^dev\//, '');
    h.update(`${key}:${crypto.createHash('sha256').update(fs.readFileSync(path.join(root, f))).digest('hex')}\n`);
  }
  return { files: files.length, sha256: h.digest('hex').slice(0, 16) };
}
