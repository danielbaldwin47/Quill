// THROWAWAY (#147): shoots one ad-hoc state on the Gate's own stage.
//
// `tools/gate judge` is the only shooter in the repo and it shoots a Piece's
// judged states and nothing else. #147 needs the same stage — same headless
// output, same 3200x2000 at scale 2, same `grim -T` — pointed at states that
// are not judged (the "jump" pair) and at four builds of the same state. So
// this drives `openStage`/`Stage.shoot` directly, from a job file listing the
// shots, and is reverted with the geometry patch it was written for.
//
//   node dev/shots/caret/147/shoot147.mjs <jobs.json>
//
// Each job: { bin, out, flags, active, env }, where `flags` is a resolved
// state in `dev/shots/oracle/states.json`'s own vocabulary and `env` is merged into
// the launch — which is how the shape is chosen, `QUILL_CARET_SHAPE` being an
// environment variable rather than a flag the app has got. One stage for the
// whole run: `openStage` is serial, and reopening it per shot would cost the
// owner's focus once per job.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openStage, quillArgv, APP_ID } from '../../../../tools/harness.mjs';

// dev/shots/caret/147/ -> the repo root.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const jobs = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

const stage = await openStage({ root, appId: APP_ID });
const done = [];
try {
  for (const job of jobs) {
    // `Stage.shoot` reads the binary's environment from `launchEnv(process.env)`
    // at launch, so the shape is set on this process for the duration of the
    // shot rather than passed through an argument the app has not got.
    if (job.env) for (const [k, v] of Object.entries(job.env)) process.env[k] = v;
    const argv = quillArgv(root, job.flags);
    const out = path.join(root, job.out);
    const sha = await stage.shoot({
      bin: job.bin, argv, w: job.flags.w, h: job.flags.h, out, active: job.active !== false,
    });
    done.push({ out: job.out, sha });
    console.log(`shot ${job.out} ${sha.slice(0, 12)}`);
    if (job.env) for (const k of Object.keys(job.env)) delete process.env[k];
  }
} finally {
  stage.close();
}
fs.writeFileSync(path.join(root, 'dev/shots/caret/147/shots.json'), `${JSON.stringify(done, null, 1)}\n`);
console.log(`shot147: ${done.length} shots`);
