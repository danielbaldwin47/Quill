// gdb-launch.mjs <log> <seconds> [extra argv...]: a bench-shaped launch run under gdb (ptrace_scope 1).
import path from 'node:path';
import fs from 'node:fs';
import { spawn, execFileSync } from 'node:child_process';
import { openStage, launchEnv } from '/home/diggle/repos/quill/.claude/worktrees/bench-489/tools/harness.mjs';
const ROOT = '/home/diggle/repos/quill/.claude/worktrees/bench-489';
const [log, secs, ...extra] = process.argv.slice(2);
const stage = await openStage({ root: ROOT });
try {
  stage.rules({ w: 1440, h: 900, initialFocus: true });
  const argv = ['--deterministic', '--measure', path.join(stage.tmp, 'cap.jsonl'),
    '--text', path.join(ROOT, 'dev/shots/latency/doc10k.md'), '--caret', 'end', '--focus', 'off',
    '--w', '1440', '--h', '900', '--syntax', 'off', '--style', 'off', '--spell', 'off', ...extra];
  const out = fs.openSync(log, 'w');
  const env = { ...launchEnv(process.env), DEBUGINFOD_URLS: '', QUILL_T0_NS: String(BigInt(Date.now()) * 1000000n) };
  const gdb = spawn('gdb', ['-q', '-batch', '-x', '/home/diggle/.claude/jobs/1a4897df/tmp/frames.gdb',
    '--args', path.join(ROOT, 'target/release/quill'), ...argv], { env, stdio: ['ignore', out, out] });
  await new Promise((r) => setTimeout(r, Number(secs) * 1000));
  try { execFileSync('pkill', ['-f', `target/release/quill --deterministic --measure ${stage.tmp}`]); } catch { /* gone */ }
  await new Promise((r) => setTimeout(r, 1000));
  gdb.kill('SIGKILL');
} finally {
  stage.close();
}
