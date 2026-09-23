// key0.mjs <dir> <regime> [before ms] [key]: every probe line and key from `before` ms ahead of a measured key to its presentation.
import fs from 'node:fs';
import path from 'node:path';
const [dir, name, before = 700, which = 0] = process.argv.slice(2);
const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8').trim().split('\n')
  .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
const warm = cap.length - 300;
const k = cap[warm + Number(which)];
const lo = k.handler_us - before * 1000; const hi = k.present_us + 20000;
const ms = (us) => ((us - exec) / 1e3).toFixed(1);
console.log(`${name}: ${warm} warm-up keys; key ${which} handled ${ms(k.handler_us)} presented ${ms(k.present_us)} (${(k.present_us / 1000 - k.evdev_ms).toFixed(2)} ms from evdev), frame ${k.frame}`);
const ev = [
  ...probe.filter((p) => p.t >= lo && p.t <= hi && !p.what.startsWith('exec')),
  ...cap.map((c, j) => ({ c, j })).filter(({ c }) => c.handler_us >= lo && c.handler_us <= hi)
    .map(({ c, j }) => ({ t: c.handler_us, what: `KEY ${j - warm} -> frame ${c.frame}` })),
].sort((a, b) => a.t - b.t);
let last = null;
for (const e of ev) {
  // Runs of drain marks are folded to one line each.
  if (/^drain[+-]$/.test(e.what) && last === 'drain') continue;
  last = /^drain[+-]$/.test(e.what) ? 'drain' : null;
  console.log(`  ${ms(e.t)} ${e.what.replace(/ \(parent.*\)/, '').replace(/ update.*/, '')}`);
}
