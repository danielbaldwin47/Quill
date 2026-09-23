// keyless.mjs <dir> <regime>: every presented frame that carried no key, with its time after exec.
import fs from 'node:fs';
import path from 'node:path';
const [dir, name] = process.argv.slice(2);
const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8').trim().split('\n')
  .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
const carried = new Set(cap.map((c) => c.frame));
const frames = probe.filter((p) => /^f \d+$/.test(p.what)).map((p) => ({ t: p.t, n: +p.what.slice(2) }));
const keyless = frames.filter((f) => !carried.has(f.n));
console.log(`${name}: ${frames.length} frames, ${keyless.length} keyless; key 0 handled at ${((cap[25].handler_us - exec) / 1e3).toFixed(0)} ms`);
console.log(keyless.map((f) => `${f.n}@${((f.t - exec) / 1e3).toFixed(0)}`).join(' '));
