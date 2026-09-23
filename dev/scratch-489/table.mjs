// table.mjs: each arrangement's mean, p99 and worst per regime, as ranges over its runs.
import fs from 'node:fs';
const DIR = '/home/diggle/.claude/jobs/1a4897df/tmp/runs';
const groups = {
  'current (4 runs)': ['base1', 'base2', 'base3', 'base4'],
  'one injector (2)': ['reuse1', 'reuse2'],
  'one injector + 45 ms warm-up (2)': ['comb1', 'comb2'],
  'one injector + 1.5 s wait (2)': ['timeline1', 'timeline2'],
  '150 keys (1)': ['keys150'],
};
const rng = (v) => (Math.min(...v) === Math.max(...v) ? `${v[0]}` : `${Math.min(...v)}–${Math.max(...v)}`);
for (const [name, labels] of Object.entries(groups)) {
  const by = {};
  for (const l of labels) {
    for (const m of fs.readFileSync(`${DIR}/${l}.out`, 'utf8').matchAll(/gate bench (\S+): \w+ — mean ([\d.]+) ms, worst ([\d.]+) ms, p50 [\d.]+ ms, p99 ([\d.]+) ms/g)) {
      const r = (by[m[1]] ??= { mean: [], p99: [], worst: [] });
      r.mean.push(+m[2]); r.worst.push(+m[3]); r.p99.push(+m[4]);
    }
  }
  console.log(name);
  for (const [regime, r] of Object.entries(by)) console.log(`| ${regime} | ${rng(r.mean)} | ${rng(r.p99)} | ${rng(r.worst)} |`);
}
