// idle-launch.mjs <probe file> <seconds> [extra argv...]: one bench-shaped launch on the stage, no keys typed.
import path from 'node:path';
import { openStage } from '/home/diggle/repos/quill/.claude/worktrees/bench-489/tools/harness.mjs';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const [probe, secs, ...extra] = process.argv.slice(2);
const stage = await openStage({ root: ROOT });
try {
  stage.rules({ w: 1440, h: 900, initialFocus: true });
  process.env.QUILL_PROBE = probe;
  const argv = ['--deterministic', '--measure', path.join(stage.tmp, 'cap.jsonl'),
    '--text', path.join(ROOT, 'dev/shots/latency/doc10k.md'), '--caret', 'end', '--focus', 'off',
    '--w', '1440', '--h', '900', '--syntax', 'off', '--style', 'off', '--spell', 'off', ...extra];
  const ours = await stage.launch(path.join(ROOT, 'target/release/quill'), argv);
  console.log(`pid ${ours.child.pid}`);
  await new Promise((r) => setTimeout(r, Number(secs) * 1000));
  stage.kill(ours.child);
} finally {
  stage.close();
}
