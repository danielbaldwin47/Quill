// phases.mjs <bench.log>: the time between consecutive phase marks, one row per regime.
import fs from 'node:fs';
const marks = fs.readFileSync(process.argv[2], 'utf8').split('\n')
  .map((l) => /^@(\d+) (.*)$/.exec(l)).filter(Boolean).map((m) => ({ t: +m[1], what: m[2] }));
const s = (a, b) => ((b - a) / 1000).toFixed(2);
let row = null;
const rows = [];
let prev = null;
const pre = [];
for (const m of marks) {
  if (m.what.startsWith('launch ')) { row = { name: m.what.slice(7), start: m.t, parts: [] }; rows.push(row); if (prev) row.gap = s(prev.t, m.t); prev = m; continue; }
  if (row) row.parts.push(`${m.what} +${s(prev.t, m.t)}`); else pre.push(`${m.what} @${s(0, m.t)}`);
  if (row && m.what === 'result written') row.total = s(row.start, m.t);
  prev = m;
}
console.log(`before the first regime: ${pre.join(', ')}`);
for (const r of rows) console.log(`${r.name} total ${r.total}s${r.gap ? ` (gap before launch ${r.gap}s)` : ''}\n   ${r.parts.join(', ')}`);
const last = marks[marks.length - 1];
console.log(`end: ${last.what} @${s(0, last.t)}`);
