// subsample.mjs <run log>...: mean, p99 and worst of each regime's result, over key-count windows.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const pct = (a, p) => a[Math.min(a.length - 1, Math.max(0, Math.ceil(p * a.length) - 1))];
const st = (xs) => { const a = [...xs].sort((x, y) => x - y); return { mean: a.reduce((s, x) => s + x, 0) / a.length, p99: pct(a, 0.99), worst: a[a.length - 1] }; };
const windows = { 'all 300': [0, 300], 'first 100': [0, 100], 'first 150': [0, 150], 'last 150': [150, 300], 'first 200': [0, 200] };
const by = {};
for (const log of process.argv.slice(2)) {
  for (const m of fs.readFileSync(log, 'utf8').matchAll(/wrote (dev\/shots\/latency\/bench-(\S+?)-\d{8}T\d{6}\.json)/g)) {
    const r = JSON.parse(fs.readFileSync(path.join(ROOT, m[1]), 'utf8'));
    for (const [w, [a, b]] of Object.entries(windows)) {
      const s = r.samples_ms.slice(a, b);
      if (s.length < b - a) continue;
      ((by[r.regime] ??= {})[w] ??= []).push(st(s));
    }
  }
}
const rng = (xs, k) => { const v = xs.map((x) => x[k]); return `${Math.min(...v).toFixed(2)}–${Math.max(...v).toFixed(2)}`; };
for (const [regime, ws] of Object.entries(by)) {
  console.log(regime);
  for (const [w, xs] of Object.entries(ws)) console.log(`  ${w.padEnd(10)} n=${xs.length}  mean ${rng(xs, 'mean')}  p99 ${rng(xs, 'p99')}  worst ${rng(xs, 'worst')}`);
}
