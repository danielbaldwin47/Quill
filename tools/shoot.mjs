// A judged shot of ours with no critic: the release binary shot at a Piece's judged states, cut the
// way the judge cuts them, and each compared byte for byte with the latest round that judged it.
//
//   tools/gate shoot <piece>              every judged state of the Piece
//   tools/gate shoot <piece> <state>...   only the states named
//
// For looking before spending a critic. A session that has moved the Focus rule wants to see its
// own sentence crop, and to know whether the paragraph crop moved at all, before `tools/gate judge`
// puts a critic on every state; six of the eight sessions in the week to 2026-09-01 wrote this
// script themselves in the job's tmp directory, reading harness.mjs and judge.mjs to do it. The
// checks, the build, the stage and the cutting are the judge's own (`preflight` and `shootState` in
// tools/judge.mjs), so a shot taken here is the shot the judge would take.
//
// THE LINES AND THE EXIT CODE. One line per state on stdout — where the shot is, and whether its
// pixels are the ones the latest round judged, which is what `tools/gate judge` reads to carry a
// verdict forward — then `gate shoot <piece>: shot N states (M unchanged since their last round)`,
// exit 0. `gate shoot <piece>: refused (<why>)` and exit 3, with the trail on stderr, for every
// reason the judge would refuse, and for a state the Piece has not got. Nothing here is evidence:
// the shots go under target/gate/shoot/<piece>/ rather than shots/, no pair is made, and no round
// is written.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { assertState } from './assert-state.mjs';
import { APP_ID, openStage } from './harness.mjs';
import { Refused, carriedFrom, openLog, preflight, say, shootState, spill } from './judge.mjs';
import { readStates, resolveStates } from './oracle.mjs';
import { rounds } from './rounds.mjs';

// Where a looked-at shot goes: under the build directory, never under shots/, which is a round's.
export const OUT = 'target/gate/shoot';

// The files one state's shot is made of, named as the judge names a round's so a crop reads as one.
// `theirs` for a Parity state is the frozen oracle itself, read and never written; for a Design
// oracle state it is the crop cut fresh beside ours.
export function shotPaths(piece, state, cut) {
  const stem = path.join(OUT, piece, state);
  return {
    shot: `${stem}-ours.png`,
    lit: `${stem}-ours-lit.png`,
    ours: cut ? `${stem}-ours-crop.png` : `${stem}-ours.png`,
    theirs: cut ? `${stem}-theirs-crop.png` : path.join('shots/oracle', piece, `${state}.png`),
  };
}

// What one paired state's line says after its path: which round last judged these exact pixels,
// or which round the pixels have moved since, or that none has.
export function standing(prior, last) {
  if (prior) return `the pixels round ${prior.round} judged (${prior.state.winner}, ${prior.state.margin}); a judge would carry that verdict`;
  if (last) return `not the pixels round ${last.round} judged; a judge would ask a critic`;
  return 'no round has judged this state; a judge would ask a critic';
}

function usage(where = process.stderr) {
  where.write(`usage: tools/gate shoot <piece> [state ...] [--settings <path>]

  The Pieces with judged states are the keys of "pieces" in
  shots/oracle/states.json, and a Piece's states are the keys under it.
  What the command does: tools/gate --help

  --settings is ours' settings file for the run, as tools/gate judge takes it.
`);
}

function refuse(piece, why) {
  spill();
  console.log(`gate shoot ${piece}: refused (${why})`);
  return 3;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);

  let piece = null;
  const names = [];
  let settingsFile = null;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--settings') {
      settingsFile = argv[++i];
      if (settingsFile === undefined) { process.stderr.write('gate shoot: --settings takes the settings file to open ours with\n'); usage(); return 3; }
    } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`gate shoot: ${a}: not a flag this command has\n`); usage(); return 3; }
    else if (piece === null) piece = a;
    else names.push(a);
  }
  if (piece === null) { usage(); return 3; }
  openLog(root, `shoot-${piece}`);
  if (piece === 'latency') return refuse(piece, 'the latency Piece is a bench run and shoots nothing');

  // A state the Piece has not got is refused before the build the checks end in, because a typo
  // should cost a second and not a minute.
  let resolved;
  try { resolved = resolveStates(readStates(root), piece); }
  catch (e) {
    say(`gate shoot: ${e.message}`);
    return refuse(piece, `${piece} is not a Piece with judged states`);
  }
  const unknown = names.filter((n) => !resolved.some((s) => s.name === n));
  if (unknown.length) return refuse(piece, `${unknown.join(', ')}: not a judged state of ${piece}`);

  let plan;
  try { plan = preflight(root, piece, settingsFile, { command: 'shoot' }); }
  catch (e) {
    if (e instanceof Refused) return refuse(piece, e.why);
    throw e;
  }
  const want = plan.resolved.filter((s) => !names.length || names.includes(s.name));
  const recorded = rounds(root, piece);
  fs.mkdirSync(path.join(root, OUT, piece), { recursive: true });

  const lines = [];
  let unchanged = 0;
  const stage = await openStage({ root, appId: APP_ID });
  try {
    for (const s of want) {
      const cut = plan.cropping.get(s.name);
      const paths = shotPaths(piece, s.name, cut);
      say(`gate shoot ${piece}: shooting ${s.name}`);
      const { ours, theirs } = await shootState(stage, root, s, settingsFile, cut, paths);

      // An asserted state is measured rather than compared, as the judge measures it (ADR 0017).
      if (s.assert) {
        await shootState(stage, root, s, settingsFile, null, { ...paths, shot: paths.lit }, { active: true });
        const answer = assertState(s.assert, {
          lit: fs.readFileSync(path.join(root, paths.lit)),
          dim: fs.readFileSync(path.join(root, paths.shot)),
        });
        lines.push(`gate shoot ${piece}: ${s.name} -> ${ours}: asserted, ${answer.ours ? 'ours' : 'theirs'} (${answer.why})`);
        continue;
      }

      const prior = carriedFrom(root, recorded, s.name, ours, theirs);
      const last = [...recorded].reverse().find((r) => (r.states || []).some((x) => x.name === s.name && x.pick));
      if (prior) unchanged += 1;
      lines.push(`gate shoot ${piece}: ${s.name} -> ${ours}: ${standing(prior, last)}`);
    }
  } finally {
    stage.close();
  }

  for (const line of lines) { say(line); console.log(line); }
  console.log(`gate shoot ${piece}: shot ${want.length} state${want.length === 1 ? '' : 's'} (${unchanged} unchanged since their last round)`);
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (e) {
    say(`gate shoot: ${e.message}`);
    process.exitCode = refuse(process.argv[2] || '?', 'the run broke before a shot');
  }
}
