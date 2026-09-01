// The critic, replayed over rounds already on disk: the pairs the recorded verdicts were given, put
// to fresh critics again, and the answers counted against what was recorded.
//
//   node tools/critic-replay.mjs <piece> [--rounds N] [--effort low|medium|high] [--prompt <file>]
//
// For changing the critic without changing what it decides. tools/critic.md, the effort a critic
// runs at, and what it is told to crop are each a lever on the five to six minutes a critic takes
// (2026-09-01: 26–30 tool calls of ImageMagick per critic), and none of them is pulled on a live
// round. The states of the last N rounds (3 unless --rounds says) are re-paired with fresh letters,
// put to critics built the new way — --effort for the effort, --prompt for a candidate prompt file
// in tools/critic.md's shape — and the picks compared with the winners on disk. A replay of the
// unchanged critic is the one measurement of how noisy it is: agreeing with itself nine times in
// ten says what a `slight` margin is worth.
//
// THE LINE AND THE EXIT CODE. One line per state, then `critic replay <piece>: N of M agree
// (rounds r4–r6)`, exit 0 whatever N is — agreement is a number to read, not a Gate condition —
// and `critic replay <piece>: refused (<why>)`, exit 3, when there was nothing to replay. Nothing
// is written under progress/ or shots/: every pair is made in a scratch directory with its own coin
// flip, and the critics run at once the way a round's do.
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { criticPrompt, runCritic } from './judge.mjs';
import { rounds } from './rounds.mjs';

// A pair in a scratch directory: the two files under fresh letters, and which letter is ours.
export function scratchPair(root, ours, theirs) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-replay-'));
  const oursIsA = crypto.randomInt(2) === 0;
  fs.copyFileSync(path.join(root, ours), path.join(dir, oursIsA ? 'A.png' : 'B.png'));
  fs.copyFileSync(path.join(root, theirs), path.join(dir, oursIsA ? 'B.png' : 'A.png'));
  return { dir, A: path.join(dir, 'A.png'), B: path.join(dir, 'B.png'), ours: oursIsA ? 'A' : 'B' };
}

// The states a replay can put to a critic: those a critic judged (an assertion has no pair) whose
// two files are still on disk. A carried state is the same pair as the round it carries, and is
// replayed once, at the round that judged it — so the last N rounds are the last N in which a
// critic looked at something, and a round of nothing but carried verdicts is not one of them.
export function replayable(root, recorded, count) {
  const looked = (r) => r.opponent && Array.isArray(r.states) && r.states.some((s) => s.pick && !s.carried);
  const chosen = recorded.filter(looked).slice(-count);
  const tasks = [];
  for (const r of chosen) {
    for (const s of r.states) {
      if (!s.pick || !s.ours || !s.theirs || s.carried) continue;
      if (!fs.existsSync(path.join(root, s.ours)) || !fs.existsSync(path.join(root, s.theirs))) continue;
      tasks.push({ round: r.round, state: s });
    }
  }
  return { rounds: chosen.map((r) => r.round), tasks };
}

function usage(where = process.stderr) {
  where.write(`usage: node tools/critic-replay.mjs <piece> [--rounds N] [--effort low|medium|high] [--prompt <file>]

  Replays the critic over the last N recorded rounds of a Piece (3 by
  default) and counts how often it agrees with the verdicts on disk. --effort
  and --prompt build the critic the candidate way; without them the replay
  measures the critic as it is.
`);
}

function refuse(piece, why) {
  console.log(`critic replay ${piece}: refused (${why})`);
  return 3;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  let piece = null;
  let count = 3;
  let effort = 'high';
  let promptFile = 'tools/critic.md';
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--rounds') {
      count = Number(argv[++i]);
      if (!Number.isInteger(count) || count < 1) { process.stderr.write('critic replay: --rounds takes a count\n'); usage(); return 3; }
    } else if (a === '--effort') {
      effort = argv[++i];
      if (!['low', 'medium', 'high'].includes(effort)) { process.stderr.write('critic replay: --effort is low, medium or high\n'); usage(); return 3; }
    } else if (a === '--prompt') {
      promptFile = argv[++i];
      if (promptFile === undefined) { process.stderr.write('critic replay: --prompt takes a file\n'); usage(); return 3; }
    } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`critic replay: ${a}: not a flag this command has\n`); usage(); return 3; }
    else if (piece === null) piece = a;
    else { process.stderr.write('critic replay: one Piece at a time\n'); usage(); return 3; }
  }
  if (piece === null) { usage(); return 3; }

  const brief = JSON.parse(fs.readFileSync(path.join(root, 'progress/state.json'), 'utf8')).pieces.find((p) => p.id === piece);
  if (!brief?.judge) return refuse(piece, 'the Piece has no judging brief in progress/state.json');
  if (!fs.existsSync(path.resolve(root, promptFile))) return refuse(piece, `${promptFile} is not a file to read`);
  const prompt = criticPrompt(fs.readFileSync(path.resolve(root, promptFile), 'utf8'), { title: brief.title, judge: brief.judge });

  const { rounds: chosen, tasks } = replayable(root, rounds(root, piece), count);
  if (!tasks.length) return refuse(piece, chosen.length ? 'the rounds chosen hold no pair a critic judged that is still on disk' : 'no native round has been recorded for this Piece');

  const pairs = tasks.map((t) => scratchPair(root, t.state.ours, t.state.theirs));
  let answers;
  try {
    answers = await Promise.allSettled(pairs.map((p) => runCritic(prompt, p.A, p.B, { effort })));
  } finally {
    for (const p of pairs) fs.rmSync(p.dir, { recursive: true, force: true });
  }

  let agreed = 0;
  let answered = 0;
  for (const [i, t] of tasks.entries()) {
    const a = answers[i];
    if (a.status === 'rejected') {
      console.log(`critic replay ${piece}: r${t.round} ${t.state.name}: no answer (${a.reason.message.split('\n')[0]})`);
      continue;
    }
    answered += 1;
    const winner = a.value.pick === pairs[i].ours ? 'ours' : 'theirs';
    const same = winner === t.state.winner;
    if (same) agreed += 1;
    console.log(`critic replay ${piece}: r${t.round} ${t.state.name}: ${same ? 'agrees' : 'DISAGREES'} (now ${winner}, ${a.value.margin}; recorded ${t.state.winner}, ${t.state.margin})`);
  }
  const span = chosen.length === 1 ? `round r${chosen[0]}` : `rounds r${chosen[0]}–r${chosen[chosen.length - 1]}`;
  const how = `${effort === 'high' ? '' : `effort ${effort}, `}${promptFile === 'tools/critic.md' ? '' : `prompt ${promptFile}, `}`;
  console.log(`critic replay ${piece}: ${agreed} of ${answered} agree (${how}${span})`);
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
