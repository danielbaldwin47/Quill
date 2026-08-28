// A blind A/B pair for a critic, and the key that says which was which.
//
//   node tools/blind.mjs pair <piece> <state> <ours.png> <theirs.png>
//   node tools/blind.mjs reveal <piece> <state>
//
// `tools/gate judge` imports these rather than running them, but the two commands stay because the
// owner rerunning a verdict by hand is the reason blind judging is trustworthy at all: the pair on
// disk can be looked at, and the key can be asked for afterwards.
//
// WHAT MAKES IT BLIND. The pair directory is emptied first and then holds exactly `A.png` and
// `B.png` — no key, no note, no name of a Piece or an app, nothing whose presence or absence could
// be read as an answer. The two files are written as bytes in A-then-B order rather than copied in
// ours-then-theirs order, so even the modification times are in the order a reader would guess
// rather than the order that would tell them something. Which letter is ours is a coin flip from
// the system's random source, and the answer is written outside the repository entirely: a key
// inside it would be one `git status` away from the critic's own working directory.
//
// WHERE THE KEY LIVES. `$XDG_STATE_HOME/quill/blind-keys/`, which is where `docs/architecture.md`
// puts it, beside the app's own state. It used to point at a job directory belonging to a session
// that has since been deleted, which meant every reveal was a file-not-found.
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

// Where a key is kept. `BLIND_KEY_DIR` is for a test that must not write into the owner's state,
// and for nothing else.
export function keyDir() {
  if (process.env.BLIND_KEY_DIR) return process.env.BLIND_KEY_DIR;
  const state = process.env.XDG_STATE_HOME || path.join(os.homedir(), '.local', 'state');
  return path.join(state, 'quill', 'blind-keys');
}

// Where a Piece's state is paired. One directory per judged state, because one Piece is several
// pairs and a critic is shown one of them.
export function pairDir(piece, state) {
  return path.join('shots/blind', piece, state);
}

function keyFile(piece, state) {
  return path.join(keyDir(), `${piece}-${state}.json`);
}

// Writes the pair and the key, and answers with the two paths and which letter is ours.
//
// The directory is emptied rather than overwritten: a pair left by an earlier round with a state
// that has since been renamed would otherwise sit beside this one, and a critic told to read
// `A.png` and `B.png` would be reading them out of a directory that has four files in it.
export function pair(piece, state, ours, theirs) {
  const dir = pairDir(piece, state);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });

  const oursIsA = crypto.randomInt(2) === 0;
  const bytes = { ours: fs.readFileSync(ours), theirs: fs.readFileSync(theirs) };
  fs.writeFileSync(path.join(dir, 'A.png'), oursIsA ? bytes.ours : bytes.theirs);
  fs.writeFileSync(path.join(dir, 'B.png'), oursIsA ? bytes.theirs : bytes.ours);

  const key = { piece, state, ours: oursIsA ? 'A' : 'B', oursFile: ours, theirsFile: theirs, at: new Date().toISOString() };
  fs.mkdirSync(keyDir(), { recursive: true });
  fs.writeFileSync(keyFile(piece, state), `${JSON.stringify(key, null, 2)}\n`);
  return { dir, A: path.join(dir, 'A.png'), B: path.join(dir, 'B.png'), ours: key.ours };
}

// Reads the key back. A pair with no key is not a pair that can be scored, and saying which file
// is missing is the only useful thing to say about it.
export function reveal(piece, state) {
  const file = keyFile(piece, state);
  if (!fs.existsSync(file)) throw new Error(`no key for ${piece}/${state} at ${file}`);
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function usage(where = process.stderr) {
  where.write(`usage: node tools/blind.mjs pair <piece> <state> <ours.png> <theirs.png>
       node tools/blind.mjs reveal <piece> <state>

  Pairs land in shots/blind/<piece>/<state>/ as A.png and B.png; the key is kept
  outside the repository, under $XDG_STATE_HOME/quill/blind-keys/.
`);
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  const [cmd, piece, state, ours, theirs] = process.argv.slice(2);
  try {
    if (cmd === 'pair' && piece && state && ours && theirs) console.log(JSON.stringify(pair(piece, state, ours, theirs)));
    else if (cmd === 'reveal' && piece && state) console.log(JSON.stringify(reveal(piece, state)));
    else { usage(); process.exitCode = 2; }
  } catch (e) {
    process.stderr.write(`blind: ${e.message}\n`);
    process.exitCode = 1;
  }
}
