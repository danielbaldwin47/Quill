// Recompute the retained diagnostic observations through the production key join.
// Usage from the repository root: node progress/diagnostics/ticket-327/analyze.mjs <extracted-directory>
import fs from 'node:fs';
import path from 'node:path';
import { align, handlerMs, latencyMs } from '../../../tools/bench-join.mjs';

const root = process.argv[2];
if (!root) throw new Error('Pass the directory containing the extracted diagnostic records.');
const results = [];
for (const directory of ['quill-gate-rliPve', 'quill-gate-JcJdlB']) {
  const read = (name) => fs.readFileSync(path.join(root, directory, name), 'utf8');
  const joined = JSON.parse(read('joined-0.json'));
  const pairs = align(joined.sent, joined.seen).pairs;
  const pair = pairs.reduce((a, b) => latencyMs(a) > latencyMs(b) ? a : b);
  const events = read('capture-0.probe.jsonl').trim().split('\n').map(JSON.parse);
  const paint = events.find((e) => e.kind === 'paint' && e.frame === pair.seen.frame);
  const stages = events.filter((e) => e.kind === 'stage'
    && e.start_us < pair.seen.present_us && e.end_us > pair.seen.handler_us);
  const accepts = events.filter((e) => e.kind === 'accept');
  const reads = (a) => events.filter((e) => e.kind === 'reread'
    && e.accepted_us === a.after_us && e.frame === a.frame);
  results.push({
    directory,
    accounted_keys: pairs.length,
    maximum: {
      pair,
      write_to_presented_ms: latencyMs(pair),
      handler_to_presented_ms: handlerMs(pair),
      paint,
      overlapping_stages: stages,
    },
    future: accepts.filter((e) => e.present_us > e.after_us)
      .map((accepted) => ({ accepted, reads: reads(accepted) })),
    revisions: accepts.flatMap((a) => reads(a).filter((e) => e.present_us !== a.present_us)),
  });
}
console.log(JSON.stringify(results, null, 2));
