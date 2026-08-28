// Which build of the JavaScript app a number or a shot belongs to: one hash, for both of the
// things that have to say so.
//
//   import { appFiles, hashApp } from '../../tools/fingerprint.mjs'
//
// `legacy/tools/latency.mjs` stamps every bench it takes with this, and `tools/gate oracle` stamps
// every frozen shot with it. The two are only worth comparing — is this the build those numbers
// came from? — if they are the same hash over the same files in the same order, so the hash lives
// here and both import it; neither owns a copy, as with the regimes in tools/regimes.mjs.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

// The app's own source under `appDir`, sorted, named relative to `root` — which is how they are
// keyed in the hash, so a file that moves changes it. `fonts/` is left out: those are bytes the
// app loads, not the app, and they are megabytes to read for a build that never moves them.
export function appFiles(root, appDir = 'legacy/app') {
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

export function hashApp(root, appDir = 'legacy/app') {
  const files = appFiles(root, appDir);
  const h = crypto.createHash('sha256');
  for (const f of files) {
    h.update(`${f}:${crypto.createHash('sha256').update(fs.readFileSync(path.join(root, f))).digest('hex')}\n`);
  }
  return { files: files.length, sha256: h.digest('hex').slice(0, 16) };
}
