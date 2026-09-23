// probe.mjs <dir>: each regime's worst keys beside the probe's stalls, slow frames and marks.
import fs from 'node:fs';
import path from 'node:path';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const dir = process.argv[2];
const log = fs.readFileSync(path.join(dir, 'bench.log'), 'utf8');
const results = [...log.matchAll(/wrote (dev\/shots\/latency\/bench-(\S+?)-\d{8}T\d{6}\.json)/g)];
for (const [, file, name] of results) {
  const r = JSON.parse(fs.readFileSync(path.join(ROOT, file), 'utf8'));
  const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8').trim().split('\n')
    .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
  const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
  const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
  const s = (us) => ((us - exec) / 1e6).toFixed(3);
  const keys = cap.slice(25);
  console.log(`== ${name}: key 0 handled at ${s(keys[0].handler_us)} s, last warm-up key at ${s(cap[24].handler_us)} s`);
  const top = r.samples_ms.map((v, i) => ({ v, i })).sort((a, b) => b.v - a.v).slice(0, 4);
  for (const { v, i } of top) {
    const k = keys[i];
    const near = probe.filter((p) => p.t >= k.handler_us - 40000 && p.t <= k.present_us + 5000 && !p.what.startsWith('exec'));
    console.log(`  key ${i} ${v} ms at ${s(k.handler_us)} s: ${near.map((p) => `${p.what} @${s(p.t)}`).join('; ')}`);
  }
  const busy = probe.filter((p) => !p.what.startsWith('exec') && p.t - exec < 6e6);
  const summary = busy.filter((p) => /^(stall|frame)/.test(p.what)).map((p) => `${s(p.t)} ${p.what.replace(/ update.*/, '')}`);
  console.log(`  stalls/slow frames in the first 6 s (${summary.length}): ${summary.join(' | ')}`);
  const marks = busy.filter((p) => !/^(stall|frame)/.test(p.what));
  if (marks.length) console.log(`  marks: ${marks.length}, first ${s(marks[0].t)} ${marks[0].what}, last ${s(marks.at(-1).t)} ${marks.at(-1).what}`);
}
