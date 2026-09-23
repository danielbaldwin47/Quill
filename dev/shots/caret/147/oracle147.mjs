// THROWAWAY (#147): shoots the Parity oracle at the ticket's "jump" pair.
//
// The frozen oracle under `dev/shots/oracle/caret/` holds the three judged states
// and nothing else, and #147's jump pair is not one of them — so the two
// judged views pair with the frozen shots and this one has to be taken. It
// writes into the evidence directory rather than `dev/shots/oracle/`, so no oracle
// is re-frozen and `tools/gate oracle caret` still says `unchanged`.
//
//   node dev/shots/caret/147/oracle147.mjs
//
// The passage and the offsets are the ours side's, byte for byte: the same
// `dev/shots/caret/147/jump.md`, the same offset 10, at mono 20 px with the chrome
// off, so the two sides differ in nothing but which app drew them.

import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { shootArgv } from '../../../../tools/oracle.mjs';

// dev/shots/caret/147/ -> the repo root.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const PORT = 4319;

const flags = {
  text: 'dev/shots/caret/147/jump.md', w: 1440, h: 900, scale: 2, theme: 'light',
  font: 'mono', size: 20, focus: 'off', typewriter: false, chrome: 'off',
  caret: 10, select: null, scroll: 0, nocaret: false, active: true,
  typing: false, menu: null,
};

const states = [
  { name: 'jump-caret', flags: { ...flags } },
  { name: 'jump-select', flags: { ...flags, caret: 14, select: [10, 14] } },
];

const busy = (port) => new Promise((res) => {
  const s = net.connect(port, '127.0.0.1');
  s.on('connect', () => { s.destroy(); res(true); });
  s.on('error', () => res(false));
});

const server = spawn('node', [path.join(root, 'dev/legacy/tools/serve.mjs'), String(PORT)],
  { cwd: root, stdio: 'ignore' });
for (let i = 0; i < 100; i++) {
  if (await busy(PORT)) break;
  await new Promise((r) => setTimeout(r, 50));
}
try {
  fs.mkdirSync(path.join(root, 'dev/shots/caret/147'), { recursive: true });
  for (const s of states) {
    const out = `dev/shots/caret/147/oracle-${s.name}.png`;
    execFileSync('node', [path.join(root, 'dev/legacy/tools/shoot.mjs'),
      ...shootArgv(root, s.flags, out, `http://localhost:${PORT}/`)],
    { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] });
    console.log(`shot ${out}`);
  }
} finally {
  server.kill();
}
