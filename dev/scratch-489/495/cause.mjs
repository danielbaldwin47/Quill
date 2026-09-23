// cause.mjs <dir> [until ms]: every keyless frame up to `until`, with the probe mark just before it,
// and the next key's latency when that key lands within a refresh after it.
import fs from 'node:fs';
import path from 'node:path';
const [dir, until = 6000] = process.argv.slice(2);
for (const f of fs.readdirSync(dir).filter((x) => x.startsWith('probe-'))) {
  const name = f.slice(6, -4);
  const probe = fs.readFileSync(path.join(dir, f), 'utf8').trim().split('\n')
    .map((l) => { const [t, ...rest] = l.split(' '); return { t: +t, what: rest.join(' ') }; });
  const exec = +probe.find((p) => p.what.startsWith('exec_us')).what.split(' ')[1];
  const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l));
  const carried = new Set(cap.map((c) => c.frame));
  const ms = (us) => ((us - exec) / 1e3).toFixed(0);
  const rows = [];
  let prevFrame = null;
  for (const p of probe) {
    if (!/^f \d+$/.test(p.what)) continue;
    const n = +p.what.slice(2);
    if (!carried.has(n) && p.t - exec < until * 1000) {
      const why = probe.filter((q) => q.t <= p.t && q.t >= (prevFrame ?? 0) && !/^(f |frame|stall|exec)/.test(q.what)).map((q) => q.what.replace(/ \(parent.*\)/, '').replace('GtkScrollbar vertical.right.overlay-indicator', 'indicator'));
      const j = cap.findIndex((c) => c.handler_us > p.t);
      const hit = j >= 0 && cap[j].handler_us - p.t < 16700 ? ` -> ${j < 25 ? 'warm-up' : 'measured'} key ${j - 25} ${(cap[j].present_us / 1000 - cap[j].evdev_ms).toFixed(1)} ms` : '';
      rows.push(`${n}@${ms(p.t)} [${[...new Set(why)].join(', ') || '?'}]${hit}`);
    }
    prevFrame = p.t;
  }
  console.log(`== ${name}: key 0 at ${ms(cap[25].handler_us)} ms\n  ${rows.join('\n  ')}`);
}
