// regime-worst.mjs <bench.log> <regime> [root]: a regime's launch fields, worst keys and machine state, and
// every earlier committed result for it (worst and the key it was on).
import fs from 'node:fs';
import path from 'node:path';
const [log, regime, root = '/home/diggle/repos/quill/.claude/worktrees/bench-launch-495'] = process.argv.slice(2);
const file = [...fs.readFileSync(log, 'utf8').matchAll(/wrote (dev\/shots\/latency\/bench-(\S+?)-\d{8}T\d{6}\.json)/g)].find((m) => m[2] === regime)[1];
const r = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8'));
const top = (s) => s.map((v, i) => `${v}@${i}`).sort((a, b) => parseFloat(b) - parseFloat(a));
console.log(file, JSON.stringify(r.launch));
console.log('worst', top(r.samples_ms).slice(0, 8).join(' '));
console.log('load', r.fingerprint.machine.load1, r.fingerprint.machine.busiest_processes.slice(0, 3).join(' | '));
const dir = path.join(root, 'dev/shots/latency');
const earlier = fs.readdirSync(dir).filter((f) => f.startsWith(`bench-${regime}-`) && f.endsWith('.json') && !f.includes('capture') && path.join('dev/shots/latency', f) !== file);
for (const f of earlier.sort().slice(-8)) {
  const e = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
  if (e.samples_ms) console.log('  earlier', f, 'worst', top(e.samples_ms).slice(0, 3).join(' '));
}
