// Prepare a blind A/B pair for a critic, or reveal it.
// node tools/blind.mjs pair <piece> <ours.png> <theirs.png>   -> writes shots/blind/<piece>/A.png B.png (random order); key kept outside the repo
// node tools/blind.mjs reveal <piece>                          -> prints which of A/B is ours
import fs from 'node:fs'; import path from 'node:path'; import crypto from 'node:crypto';
const [cmd, piece, ours, theirs] = process.argv.slice(2);
const keyDir = process.env.BLIND_KEY_DIR || '/home/diggle/.claude/jobs/e9d93e91/tmp/blind-keys';
fs.mkdirSync(keyDir, { recursive: true });
const dir = path.join('shots/blind', piece); fs.mkdirSync(dir, { recursive: true });
if (cmd === 'pair') {
  const oursIsA = crypto.randomInt(2) === 0;
  fs.copyFileSync(ours, path.join(dir, oursIsA ? 'A.png' : 'B.png')); fs.copyFileSync(theirs, path.join(dir, oursIsA ? 'B.png' : 'A.png'));
  fs.writeFileSync(path.join(keyDir, piece + '.json'), JSON.stringify({ ours: oursIsA ? 'A' : 'B', oursFile: ours, theirsFile: theirs, at: new Date().toISOString() }));
  console.log(JSON.stringify({ dir, A: path.join(dir, 'A.png'), B: path.join(dir, 'B.png') }));
} else if (cmd === 'reveal') {
  console.log(fs.readFileSync(path.join(keyDir, piece + '.json'), 'utf8'));
} else { console.error('usage: blind.mjs pair|reveal ...'); process.exit(1); }
