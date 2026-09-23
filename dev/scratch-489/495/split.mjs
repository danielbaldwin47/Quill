// split.mjs <dir> <regime> <key>...: a key's time split: evdev -> handler -> present.
import fs from 'node:fs';
import path from 'node:path';
const [dir, name, ...ks] = process.argv.slice(2);
const cap = fs.readFileSync(path.join(dir, `capture-${name}.jsonl`), 'utf8').trim().split('\n').map((l) => JSON.parse(l)).slice(25);
const probe = fs.readFileSync(path.join(dir, `probe-${name}.txt`), 'utf8');
const exec = +/\d+ exec_us (\d+)/.exec(probe)[1];
for (const k of ks.map(Number)) for (const i of [k - 1, k, k + 1]) {
  const c = cap[i];
  console.log(`${name} key ${i} at ${((c.handler_us - exec) / 1e6).toFixed(3)} s: evdev->handler ${(c.handler_us / 1000 - c.evdev_ms).toFixed(2)} ms, handler->present ${((c.present_us - c.handler_us) / 1000).toFixed(2)} ms, frame ${c.frame}, refresh ${c.refresh_us}`);
}
