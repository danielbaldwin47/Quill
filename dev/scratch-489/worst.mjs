// worst.mjs <run log>...: each regime's four worst keys by index, and what the machine was doing.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
for (const log of process.argv.slice(2)) {
  console.log(path.basename(log));
  for (const m of fs.readFileSync(log, 'utf8').matchAll(/wrote (dev\/shots\/latency\/bench-\S+?-\d{8}T\d{6}\.json)/g)) {
    const r = JSON.parse(fs.readFileSync(path.join(ROOT, m[1]), 'utf8'));
    const top = r.samples_ms.map((v, i) => `${v}@${i}`).sort((a, b) => parseFloat(b) - parseFloat(a)).slice(0, 4);
    const f = r.fingerprint.machine;
    console.log(`  ${r.regime.padEnd(20)} ${top.join(' ')}  load ${f.load1.toFixed(2)}  ${f.busiest_processes.slice(0, 2).join(' | ')}`);
  }
}
