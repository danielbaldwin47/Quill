// around.mjs <dir> <regime> [n]: the n worst keys, each with every probe line and key around it.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const [dir, name, n = 2] = process.argv.slice(2);
const log = fs.readFileSync(path.join(dir, 'bench.log'), 'utf8');
const file = [...log.matchAll(/wrote (dev\/shots\/latency\/bench-(\S+?)-\d{8}T\d{6}\.json)/g)].find((m) => m[2] === name)[1];
const r = JSON.parse(fs.readFileSync(path.join(ROOT, file), 'utf8'));
const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8').trim().split('\n')
  .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
const s = (us) => ((us - exec) / 1e3).toFixed(1);
const top = r.samples_ms.map((v, i) => ({ v, i })).sort((a, b) => b.v - a.v).slice(0, Number(n));
for (const { v, i } of top) {
  // The capture's measured keys: the 25 warm-up keys lead. Find the capture key whose present - evdev matches best.
  const keys = cap.slice(25);
  const k = keys.map((c, j) => ({ c, j, d: Math.abs((c.present_us / 1000 - c.evdev_ms) - v) })).filter((x) => Math.abs(x.j - i) <= 2).sort((a, b) => a.d - b.d)[0];
  const lo = k.c.handler_us - 120000; const hi = k.c.present_us + 20000;
  console.log(`== ${name} sample ${i} ${v} ms = capture key ${k.j} (handled ${s(k.c.handler_us)} ms, presented ${s(k.c.present_us)} ms, frame ${k.c.frame})`);
  const ev = [
    ...probe.filter((p) => p.t >= lo && p.t <= hi).map((p) => ({ t: p.t, what: p.what })),
    ...keys.map((c, j) => ({ c, j })).filter(({ c }) => c.handler_us >= lo && c.handler_us <= hi)
      .map(({ c, j }) => ({ t: c.handler_us, what: `KEY ${j} -> frame ${c.frame} presented ${s(c.present_us)}` })),
  ].sort((a, b) => a.t - b.t);
  for (const e of ev) console.log(`  ${s(e.t)} ${e.what}`);
}
