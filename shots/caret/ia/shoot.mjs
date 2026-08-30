// Shoots the caret's states on the Gate's own stage, for ADR 0013.
//
// `tools/gate judge` is the only shooter in the repo and it shoots a Piece's
// judged states and nothing else. This needs the same stage — same headless
// output, same 3200x2000 at scale 2, same `grim -T` — pointed also at the
// "jump" pair, which is not a judged state: a free caret and a selection
// opening at the same offset, so the distance between the two marks can be
// measured. So it drives `openStage`/`Stage.shoot` directly, from a job file.
//
//   node shots/caret/ia/shoot.mjs <jobs.json>
//
// Each job: { bin, out, flags, active }, where `flags` is a resolved state in
// `shots/oracle/states.json`'s own vocabulary. One stage for the whole run:
// `openStage` is serial, and reopening it per shot would cost the owner's
// focus once per job.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openStage, quillArgv, APP_ID } from '../../../tools/harness.mjs';

// shots/caret/ia/ -> the repo root.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const jobs = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

const stage = await openStage({ root, appId: APP_ID });
const done = [];
try {
  for (const job of jobs) {
    const argv = quillArgv(root, job.flags);
    const out = path.join(root, job.out);
    const sha = await stage.shoot({
      bin: job.bin, argv, w: job.flags.w, h: job.flags.h, out, active: job.active !== false,
    });
    done.push({ out: job.out, sha });
    console.log(`shot ${job.out} ${sha.slice(0, 12)}`);
  }
} finally {
  stage.close();
}
fs.writeFileSync(path.join(root, 'shots/caret/ia/shots.json'), `${JSON.stringify(done, null, 1)}\n`);
console.log(`shoot: ${done.length} shots`);
