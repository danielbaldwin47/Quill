// The Piece a judge is about, in one read — `tools/gate brief <piece>`.
//
//   tools/gate brief caret
//
// The two files a judge reads are `dev/progress/state.json` (the Piece's brief) and
// `dev/shots/oracle/states.json` (its judged states), and both hold long single
// lines: a brief is one string, so a two-hit `grep` over state.json returned 25k
// characters (docs/agents/context.md § What the steps cost when skipped). This
// prints the one Piece's half of each instead — the brief wrapped to 100
// columns, then one block per judged state naming its flag overrides, which
// oracle arbitrates it, and whether it is typed.
//
// A pure function of two files; no window, no browser, no network.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const WIDTH = 100;

/** The line the owner reads when there is nothing to brief, and the exit `gate.md` gives it. */
function refuse(piece, why) {
  console.log(`gate brief${piece ? ` ${piece}` : ''}: refused (${why})`);
  process.exit(3);
}

/** `text` as lines no wider than WIDTH once `indent` is on the front of each. */
function wrap(text, indent) {
  const lines = [];
  let line = indent;
  for (const word of String(text).split(/\s+/).filter(Boolean)) {
    if (line !== indent && line.length + 1 + word.length > WIDTH) {
      lines.push(line);
      line = indent;
    }
    line += line === indent ? word : ` ${word}`;
  }
  if (line !== indent) lines.push(line);
  return lines;
}

/** A state's flag overrides, `key=value` in the order states.json holds them. */
function flags(state) {
  return Object.entries(state)
    .filter(([key]) => !['opponent', 'assert', 'keys'].includes(key))
    .map(([key, value]) => `${key}=${JSON.stringify(value)}`)
    .join('  ');
}

/** Which oracle arbitrates a state, and the rectangles when it is the Design oracle's. */
function opponent(piece, name, state) {
  if (state.opponent) {
    const { capture, crop, ours } = state.opponent;
    return [
      `opponent: the Design oracle, ${capture}`,
      `  crop: [${crop.join(', ')}]  ours: [${ours.join(', ')}]`,
    ];
  }
  if (state.assert) return [`opponent: none — asserted off ours' own pixels (${JSON.stringify(state.assert)})`];
  return [`opponent: the Parity oracle, dev/shots/oracle/${piece}/${name}.png`];
}

const piece = process.argv[2];
if (!piece) refuse('', 'no Piece named');

const progress = JSON.parse(fs.readFileSync(path.join(ROOT, 'dev/progress/state.json'), 'utf8'));
const states = JSON.parse(fs.readFileSync(path.join(ROOT, 'dev/shots/oracle/states.json'), 'utf8'));
const entry = progress.pieces.find((p) => p.id === piece);
const judged = states.pieces[piece];
if (!entry && !judged) refuse(piece, 'no such Piece');

if (entry) {
  console.log(`${entry.id} — ${entry.title}`);
  console.log(`what: ${entry.what}`);
  if (entry.judge) {
    console.log('judge:');
    for (const line of wrap(entry.judge, '  ')) console.log(line);
  }
} else {
  console.log(`${piece} — no entry in dev/progress/state.json`);
}

const names = Object.keys(judged ?? {});
for (const name of names) {
  const state = judged[name];
  console.log('');
  console.log(name);
  console.log(`  flags: ${flags(state) || 'none, the defaults as they stand'}`);
  for (const line of opponent(piece, name, state)) console.log(`  ${line}`);
  console.log(`  keys: ${state.keys ? `yes, ${state.keys.bursts?.length ?? 0} bursts` : 'no'}`);
}

// The typed script is the Piece's rather than a state's: `keys.<piece>` in
// states.json, its own flags and its own bursts, run by `tools/gate keys`. A
// Piece may carry a list of them, each on its own launch.
const scripts = states.keys?.[piece];
if (scripts) {
  console.log('');
  for (const script of [scripts].flat()) {
    console.log(`keys script: ${script.bursts.length} bursts, at ${flags(script.state)}`);
  }
}

console.log('');
console.log(`gate brief ${piece}: ${names.length} states`);
