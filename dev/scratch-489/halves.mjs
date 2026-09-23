// halves.mjs <run log>...: each regime-run's mean over keys 0–149 against keys 150–299.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const mean = (a) => a.reduce((s, x) => s + x, 0) / a.length;
let up = 0, n = 0;
const diffs = [];
for (const log of process.argv.slice(2)) {
  for (const m of fs.readFileSync(log, 'utf8').matchAll(/wrote (dev\/shots\/latency\/bench-\S+?-\d{8}T\d{6}\.json)/g)) {
    const r = JSON.parse(fs.readFileSync(path.join(ROOT, m[1]), 'utf8'));
    const d = mean(r.samples_ms.slice(150, 300)) - mean(r.samples_ms.slice(0, 150));
    diffs.push(d); n += 1; if (d > 0) up += 1;
  }
}
diffs.sort((a, b) => a - b);
console.log(`second half higher in ${up} of ${n}; difference ${diffs[0].toFixed(2)} to ${diffs[n - 1].toFixed(2)} ms, median ${diffs[n >> 1].toFixed(2)}`);
