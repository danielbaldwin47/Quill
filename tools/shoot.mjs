// A judged shot of ours with no critic: the release binary shot at a Piece's judged states, cut the
// way the judge cuts them, and each compared byte for byte with the latest round that judged it.
//
//   tools/gate shoot <piece>              every judged state of the Piece
//   tools/gate shoot <piece> <state>...   only the states named
//   tools/gate shoot --pieces a,b         every judged state of each Piece named, on one stage
//   tools/gate shoot --all                every Piece with judged states, on one stage
//
// For looking before spending a critic. A session that has moved the Focus rule wants to see its
// own sentence crop, and to know whether the paragraph crop moved at all, before `tools/gate judge`
// puts a critic on every state; six of the eight sessions in the week to 2026-09-01 wrote this
// script themselves in the job's tmp directory, reading harness.mjs and judge.mjs to do it. The
// checks, the build, the stage and the cutting are the judge's own (`preflight` and `shootState` in
// tools/judge.mjs), so a shot taken here is the shot the judge would take.
//
// SEVERAL PIECES ARE ONE STAGE. A session that has touched the type wants every Piece, and a shell
// loop over `gate shoot <piece>` opens and removes a headless output per Piece: that loop hung
// Hyprland on 2026-09-02. So `--pieces` and `--all` are one build and one stage, with the checks
// and the states run per Piece as they are for one, the way `tools/gate bench --all` is one launch
// per regime on one stage. A Piece name, `--all` and `--pieces` each say which Pieces to shoot, so
// only one of them may be given, and a state is named for one Piece, so states go with a name only.
//
// THE LINES AND THE EXIT CODE. One line per state on stdout — where the shot is, and whether its
// pixels are the ones the latest round judged, which is what `tools/gate judge` reads to carry a
// verdict forward — then `gate shoot <piece>: shot N states (M unchanged since their last round)`,
// exit 0; a run of several Pieces prints that per Piece and ends in `gate shoot: shot N states over
// P Pieces (M unchanged since their last round)`. `gate shoot <piece>: refused (<why>)` and exit 3,
// with the trail on stderr, for every reason the judge would refuse, and for a state the Piece has
// not got; in a run of several, a refused Piece is refused by its own line after the others have
// printed, and the run ends in `gate shoot: refused (...)`, exit 3. Nothing here is evidence: the
// shots go under target/gate/shoot/<piece>/ rather than shots/, no pair is made, and no round is
// written.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { assertState, secondShot } from './assert-state.mjs';
import { APP_ID, Refusal, chosenList, openStage } from './harness.mjs';
import { Refused, buildOurs, carriedFrom, checkStates, openLog, opensAt, say, shootState, spill } from './judge.mjs';
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

// The Pieces `--all` shoots: every Piece with judged states but latency, which is a bench run and
// shoots nothing.
export function shootablePieces(states) {
  return Object.keys(states.pieces || {}).filter((p) => p !== 'latency');
}

// A Piece's own last line, and the run's: the same shape, so the two read as one.
export function shotLine(shot, unchanged, pieces = 0) {
  const states = `${shot} state${shot === 1 ? '' : 's'}`;
  const over = pieces ? ` over ${pieces} Piece${pieces === 1 ? '' : 's'}` : '';
  return `${states}${over} (${unchanged} unchanged since their last round)`;
}

function usage(where = process.stderr) {
  where.write(`usage: tools/gate shoot <piece> [state ...] | --pieces a,b | --all [--settings <path>]

  The Pieces with judged states are the keys of "pieces" in
  shots/oracle/states.json, and a Piece's states are the keys under it.
  What the command does: tools/gate --help

  --pieces   these Pieces, by name, separated by commas, on one stage
  --all      every Piece with judged states, on one stage
  --settings is ours' settings file for the run, as tools/gate judge takes it.

A Piece name, --all and --pieces each say which Pieces to shoot, so only one of them may be given;
states are named for one Piece, so they go with a name only.
`);
}

// The refused ending, for one Piece or, with no Piece, for the run.
function refuse(piece, why) {
  spill();
  console.log(`gate shoot${piece ? ` ${piece}` : ''}: refused (${why})`);
  return 3;
}

// Everything one Piece needs settled before the build, as the judge settles it (`checkStates`), or
// the `Refused` saying what is missing. A state the Piece has not got is refused here too, before
// the build, because a typo should cost a second and not a minute.
function planPiece(root, piece, names, settingsFile) {
  if (piece === 'latency') throw new Refused('the latency Piece is a bench run and shoots nothing');
  let resolved;
  try { resolved = resolveStates(readStates(root), piece); }
  catch (e) {
    say(`gate shoot: ${e.message}`);
    throw new Refused(`${piece} is not a Piece with judged states`);
  }
  const unknown = names.filter((n) => !resolved.some((s) => s.name === n));
  if (unknown.length) throw new Refused(`${unknown.join(', ')}: not a judged state of ${piece}`);
  const plan = checkStates(root, piece, settingsFile, { command: 'shoot' });
  return { want: plan.resolved.filter((s) => !names.length || names.includes(s.name)), cropping: plan.cropping, resolved: plan.resolved };
}

// One Piece's states shot on the open stage: its lines, how many it shot, and how many a judge
// would carry.
async function shootPiece(stage, root, piece, { want, cropping }, settingsFile) {
  const recorded = rounds(root, piece);
  fs.mkdirSync(path.join(root, OUT, piece), { recursive: true });
  const lines = [];
  let unchanged = 0;
  for (const s of want) {
    const cut = cropping.get(s.name);
    const paths = shotPaths(piece, s.name, cut);
    say(`gate shoot ${piece}: shooting ${s.name}`);
    const { ours, theirs } = await shootState(stage, root, s, settingsFile, cut, paths);

    // An asserted state is measured rather than compared, as the judge measures it (ADR 0017): the
    // second shot is the one its own rule asks for ([`SECOND`] in tools/assert-state.mjs).
    if (s.assert) {
      const second = secondShot(s.assert, s);
      await shootState(stage, root, second.state, settingsFile, null, { ...paths, shot: paths.lit }, second.options);
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
  return { lines, shot: want.length, unchanged };
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);

  let all = false;
  let listed = null;
  const positionals = [];
  let settingsFile = null;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--settings') {
      settingsFile = argv[++i];
      if (settingsFile === undefined) { process.stderr.write('gate shoot: --settings takes the settings file to open ours with\n'); usage(); return 3; }
    } else if (a === '--all') { all = true; } else if (a === '--pieces') { listed = argv[++i] ?? ''; } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`gate shoot: ${a}: not a flag this command has\n`); usage(); return 3; }
    else positionals.push(a);
  }
  let choice;
  try {
    choice = chosenList({
      command: 'gate shoot', flag: '--pieces', listOf: 'Piece names', what: 'which Pieces to shoot',
      all, listed, universe: () => shootablePieces(readStates(root)),
    });
  } catch (e) {
    if (!(e instanceof Refusal)) throw e;
    process.stderr.write(`${e.message}\n`);
    usage();
    return 3;
  }
  // A Piece name takes the first positional and states the rest; with --all or --pieces every
  // positional is a state, and a state is named for one Piece.
  const several = choice.flag !== null;
  const piece = several ? null : positionals[0] ?? null;
  const names = several ? positionals : positionals.slice(1);
  if (!several && piece === null) { usage(); return 3; }
  const pieces = choice.names ?? [piece];
  openLog(root, several ? 'shoot' : `shoot-${piece}`);
  if (several && names.length) return refuse(null, `states are named for one Piece, and ${names.join(', ')} came with ${choice.flag}`);
  if (!pieces.length) return refuse(null, 'shots/oracle/states.json names no Piece to shoot');

  // Every Piece's checks before the build, then one build for all of them, then every Piece's
  // states asked of the binary — all before any window opens, so a refusal costs nobody the
  // display. A Piece refused at either step is refused by its line after the rest have shot; a
  // binary that would not build refuses the run.
  const plans = new Map();
  const refused = new Map();
  for (const p of pieces) {
    try { plans.set(p, planPiece(root, p, names, settingsFile)); }
    catch (e) {
      if (!(e instanceof Refused)) throw e;
      refused.set(p, e.why);
    }
  }
  if (plans.size) {
    try { buildOurs(root, { command: 'shoot', piece }); }
    catch (e) {
      if (!(e instanceof Refused)) throw e;
      return refuse(piece, e.why);
    }
    for (const [p, plan] of plans) {
      try { opensAt(root, p, plan.resolved, settingsFile, { command: 'shoot' }); }
      catch (e) {
        if (!(e instanceof Refused)) throw e;
        plans.delete(p);
        refused.set(p, e.why);
      }
    }
  }
  if (!several && refused.size) return refuse(piece, refused.get(piece));

  const done = new Map();
  if (plans.size) {
    const stage = await openStage({ root, appId: APP_ID, say });
    try {
      for (const [p, plan] of plans) done.set(p, await shootPiece(stage, root, p, plan, settingsFile));
    } finally {
      stage.close();
    }
  }

  let shot = 0;
  let unchanged = 0;
  for (const [p, r] of done) {
    for (const line of r.lines) { say(line); console.log(line); }
    console.log(`gate shoot ${p}: shot ${shotLine(r.shot, r.unchanged)}`);
    shot += r.shot;
    unchanged += r.unchanged;
  }
  if (!several) return 0;
  for (const [p, why] of refused) console.log(`gate shoot ${p}: refused (${why})`);
  if (refused.size) return refuse(null, `${refused.size} of ${pieces.length} Pieces refused: ${[...refused.keys()].join(', ')}`);
  console.log(`gate shoot: shot ${shotLine(shot, unchanged, done.size)}`);
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (e) {
    say(`gate shoot: ${e.message}`);
    const label = process.argv[2] && !process.argv[2].startsWith('-') ? process.argv[2] : null;
    process.exitCode = refuse(label, 'the run broke before a shot');
  }
}
