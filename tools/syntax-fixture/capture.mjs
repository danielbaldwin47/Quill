// Reproduce the real PNG inputs to judge-selftest's Syntax assertions after a release build.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { openStage, quillArgv } from '../harness.mjs';
import { readStates, resolveStates } from '../oracle.mjs';
import { secondShot } from '../assert-state.mjs';

const out = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(out, '../..');
const bin = path.join(root, 'target/release/quill');
const states = readStates(root);
const captures = [];
const stage = await openStage({ root });
try {
  const shoot = async (name, flags) => {
    const argv = quillArgv(root, flags);
    await stage.shoot({ bin, argv, w: flags.w, h: flags.h, out: path.join(out, name) });
    captures.push({ file: name, flags });
  };
  // Exactly five states, each with its rule's own companion; two extra protection fixtures.
  for (const state of resolveStates(states, 'syntax')) {
    await shoot(`${state.name}-ours.png`, state.flags);
    await shoot(`${state.name}-ours-lit.png`, secondShot(state.assert, state).state.flags);
  }
  for (const syntax of ['on', 'off']) {
    await shoot(`protection-${syntax}.png`, { ...states.defaults,
      text: 'tools/syntax-fixture/protection.md', chrome: 'off', caret: 0, syntax });
  }
} finally { stage.close(); }
const provenance = {
  commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  binarySha256: crypto.createHash('sha256').update(fs.readFileSync(bin)).digest('hex'),
  captures,
};
fs.writeFileSync(path.join(out, 'capture.json'), `${JSON.stringify(provenance, null, 2)}\n`);
console.log(`syntax fixture: captured ${captures.length} PNGs`);
