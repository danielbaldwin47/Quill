// collide.mjs <dir> <regime>: for every keyless frame, the next key: how soon after it, and how long it took.
import fs from 'node:fs';
import path from 'node:path';
const [dir, name] = process.argv.slice(2);
const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8').trim().split('\n')
  .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
const carried = new Set(cap.map((c) => c.frame));
const frames = probe.filter((p) => /^f \d+$/.test(p.what)).map((p) => ({ t: p.t, n: +p.what.slice(2) }));
for (const f of frames.filter((x) => !carried.has(x.n))) {
  const j = cap.findIndex((c) => c.handler_us > f.t);
  if (j < 0) continue;
  const c = cap[j];
  const after = (c.handler_us - f.t) / 1000;
  if (after > 40) continue;
  console.log(`keyless frame ${f.n} at ${((f.t - exec) / 1e3).toFixed(0)} ms; ${j < 25 ? 'warm-up' : 'measured'} key ${j - 25} ${after.toFixed(1)} ms later, evdev->present ${(c.present_us / 1000 - c.evdev_ms).toFixed(2)} ms (frame ${c.frame})`);
}
