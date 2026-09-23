// launch495.mjs <bench.log>...: per regime, the launch fields and the four worst keys by index.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-launch-495';
for (const log of process.argv.slice(2)) {
  for (const m of fs.readFileSync(log, 'utf8').matchAll(/wrote (dev\/shots\/latency\/bench-\S+?-\d{8}T\d{6}\.json)/g)) {
    const r = JSON.parse(fs.readFileSync(path.join(ROOT, m[1]), 'utf8'));
    const top = r.samples_ms.map((v, i) => `${v}@${i}`).sort((a, b) => parseFloat(b) - parseFloat(a)).slice(0, 4);
    const l = r.launch?.[0] ?? {};
    console.log(`  ${r.regime.padEnd(20)} settled ${l.settled_ms}, quiet ${l.quiet_ms} ms, ${l.quiet_before_first_key_ms} ms before key 0, top-up ${l.top_up_keys}; worst ${top.join(' ')}`);
  }
}
