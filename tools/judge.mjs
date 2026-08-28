// The Ticket tier's blind judging, in one command: ours shot at every judged state of a Piece,
// paired with the Parity oracle, put to a fresh critic, revealed, and written down.
//
//   tools/gate judge <piece>              judge one Piece and write the round
//   tools/gate judge <piece> --note "..." ... recording what changed this round
//
// An agent closing a ticket that names a Piece runs this and pastes its lines; the owner can rerun
// it when a verdict looks wrong, because everything it decides is on disk: the pair the critic saw
// (`shots/blind/<piece>/<state>/`), the shot it took of ours, the frozen opponent it was paired
// with, and the round it wrote.
//
// THE LINE AND THE EXIT CODE. `gate judge <piece>: ours|theirs, round <N>`, and:
//
//   0  ours — every judged state of the Piece was picked blind
//   1  theirs — at least one was not
//   2  theirs, and this Piece had been won against this opponent before ("a Piece once won is never
//      lost", docs/agents/gate.md); the line above the verdict says which round won it
//   3  nothing was judged — a Piece with no judged states, a state naming a flag the app has not
//      got, an opponent that is not frozen, or a run that broke before a verdict
//
// The 0/1/2 are the verdict and are this command's own; `check` and `oracle` use their codes for
// pass and fail, which is why everything that is *not* a verdict here is gathered under one code
// rather than borrowing theirs.
//
// A PIECE IS JUDGED WHOLE. The Piece is ours only when every one of its states is. Every refusal is
// therefore checked for every state before the first window opens: a run that shoots two of three
// states and then finds the third unshootable has spent a critic on a verdict it cannot use.
//
// WHAT THE CRITIC IS AND IS NOT. One fresh `claude -p` session per pair, Opus at high effort, with
// `tools/critic.md`'s prompt and nothing else: it runs in a scratch directory outside the checkout
// holding only `A.png` and `B.png`, with customizations off, allowed `Read` and `magick` and no
// other tool. What that buys, exactly: it has no `Bash` it could run `tools/blind.mjs reveal` with,
// no `Grep` or `Glob` to find the repository with, and — customizations off — no `CLAUDE.md` to
// learn from that Quill is being judged against a JavaScript app in `legacy/`, or that a directory
// of frozen opponents exists to compare against. `Read` itself is not jailed to that directory, so
// the last step is still asked for rather than enforced: a critic that guessed the checkout's path
// could read it. That is the blindness the gauntlet had, and no less.
//
// OURS WILL LOSE. The Pieces are still to be ported; a shot of today's app against the JavaScript
// app that won its gauntlet is a loss, and that is the point of having the pipeline before the
// Pieces rather than after.
import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { pair, pairDir, reveal } from './blind.mjs';
import { APP_ID, compositorAvailable, openStage, quillArgv } from './harness.mjs';
import { fingerprint, freezeReason, readStates, resolveStates, unservable } from './oracle.mjs';

/// The opponent this command judges against while `legacy/` exists, and the word the round records
/// it under. The switch to the iA reference belongs to the retirement ticket
/// (`docs/architecture.md`, "Repo migration"), not here.
const OPPONENT = 'oracle';

/// What the opponent is called in prose — on the progress page, and in the line that says a won
/// Piece has been lost. One string, so the page and the command cannot disagree.
export const OPPONENTS = { oracle: 'Parity oracle' };

/// The binary a judged shot is of. Release rather than debug: the Gate judges what the owner would
/// install, and a debug build's frame timings and font rasterisation are not the shipped ones.
const BINARY = 'target/release/quill';

/// How long a critic is given. A critic reads two 2880x1800 images at least twice each and crops
/// into them; ten minutes is far past what that has taken and short enough that a session which has
/// stopped answering does not hold a Piece open all night.
const CRITIC_TIMEOUT_MS = 10 * 60 * 1000;

// ---------- what produced the shots ----------

function sha(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }

/// Which build of ours a round is of: the commit it was taken at, and the binary itself, because a
/// checkout can be dirty and a commit is then only half the answer.
export function build(root) {
  let git = null;
  try { git = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); } catch { /* not a checkout */ }
  return { git, binary: sha(fs.readFileSync(path.join(root, BINARY))).slice(0, 16) };
}

// ---------- the rounds already recorded ----------

/// Every round recorded for a Piece, oldest first.
export function rounds(root, piece) {
  const dir = path.join(root, 'progress/rounds');
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir)
    .filter((f) => f.startsWith(`${piece}-r`) && f.endsWith('.json'))
    .map((f) => JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')))
    .filter((r) => r.piece === piece)
    .sort((a, b) => a.round - b.round);
}

/// The round this run is: one past the highest already recorded, whoever recorded it.
export function nextRound(recorded) {
  return recorded.reduce((n, r) => Math.max(n, Number(r.round) || 0), 0) + 1;
}

/// The round that won this Piece against a native opponent, or null.
///
/// "A Piece once won is never lost" reads only rounds that carry `opponent`. The rounds already in
/// `progress/rounds/` are the JavaScript app's gauntlet against iA Writer — the reference era, the
/// thing the native app is being built to match — and a native Piece that has never been judged has
/// not been won, however many r1 verdicts sit beside it saying "ours".
export function wonBefore(recorded) {
  return recorded.filter((r) => r.opponent && r.winner === 'ours').pop() || null;
}

// ---------- the critic ----------

/// The prompt for one pair: `tools/critic.md` below its `---`, with its fields filled.
///
/// Everything above the `---` is written for whoever reads the file and is never sent, which is why
/// that half can explain itself at length without spending a critic's context on it.
export function criticPrompt(template, { title, judge }) {
  const body = template.split(/\n---\n/).slice(1).join('\n---\n').trim();
  if (!body) throw new Error('tools/critic.md has no prompt under its --- line');
  const filled = body.replaceAll('{{title}}', title).replaceAll('{{judge}}', judge);
  // A field the template grew that nothing here fills would reach the critic as `{{...}}`, which is
  // a question mark in the middle of the one thing a critic is told.
  const unfilled = filled.match(/\{\{\w+\}\}/g);
  if (unfilled) throw new Error(`tools/critic.md asks for ${[...new Set(unfilled)].join(', ')}, which this command does not fill`);
  return filled;
}

/// The critic's answer, out of whatever it said on the way to it.
///
/// The last fenced JSON object wins: a critic that reasons in prose and then answers has written
/// the answer last, and a critic that quotes the schema back before filling it in has written the
/// example first. Every key the round needs is checked here rather than where it is read, so a
/// malformed answer names itself instead of becoming an empty string in a round file.
export function criticAnswer(text) {
  const blocks = [...String(text).matchAll(/```json\s*([\s\S]*?)```/g)].map((m) => m[1]);
  const raw = blocks.length ? blocks[blocks.length - 1] : text;
  let answer;
  try { answer = JSON.parse(raw); } catch (e) { throw new Error(`the critic's answer is not JSON: ${e.message}`); }
  if (answer.pick !== 'A' && answer.pick !== 'B') throw new Error(`the critic picked ${JSON.stringify(answer.pick)}, which is neither A nor B`);
  for (const key of ['gapA', 'gapB', 'verdict']) {
    if (typeof answer[key] !== 'string' || !answer[key].trim()) throw new Error(`the critic's answer has no ${key}`);
  }
  return {
    pick: answer.pick,
    margin: answer.margin === 'slight' ? 'slight' : 'clear',
    gapA: answer.gapA,
    gapB: answer.gapB,
    verdict: answer.verdict,
    sameViewport: answer.sameViewport !== false,
    secondary: Array.isArray(answer.secondary) ? answer.secondary.filter((s) => typeof s === 'string') : [],
  };
}

/// Runs one critic on one pair and answers with what it said.
///
/// The session's working directory is a scratch directory outside the checkout holding copies of
/// `A.png` and `B.png` and nothing else, and the two tools it is allowed are the two the prompt asks
/// it to use. `--safe-mode` is the load-bearing one: it turns off this repository's own
/// instructions, and a critic that has read `CLAUDE.md` knows there is a JavaScript app in
/// `legacy/` it is probably being asked about, which is not a blind critic. The header above says
/// where that stops being enforcement and starts being instruction.
async function runCritic(prompt, A, B) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'quill-critic-'));
  try {
    fs.copyFileSync(A, path.join(dir, 'A.png'));
    fs.copyFileSync(B, path.join(dir, 'B.png'));
    const argv = [
      '-p', prompt,
      '--model', 'opus',
      '--effort', 'high',
      '--output-format', 'json',
      '--safe-mode',
      '--allowedTools', 'Read', 'Bash(magick:*)',
    ];
    const said = await run('claude', argv, { cwd: dir, timeout: CRITIC_TIMEOUT_MS });
    let envelope;
    try { envelope = JSON.parse(said); } catch { throw new Error(`the critic session printed no JSON envelope:\n${said.slice(0, 2000)}`); }
    if (envelope.is_error) throw new Error(`the critic session failed: ${envelope.result || envelope.subtype || 'no reason given'}`);
    return criticAnswer(envelope.result);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/// A child process's stdout, or its stderr as the reason it has none.
function run(command, argv, { cwd, timeout }) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, argv, { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    let err = '';
    const timer = setTimeout(() => { child.kill('SIGKILL'); reject(new Error(`${command} did not answer within ${Math.round(timeout / 1000)}s`)); }, timeout);
    child.stdout.on('data', (b) => { out += b; });
    child.stderr.on('data', (b) => { err += b; });
    child.on('error', (e) => { clearTimeout(timer); reject(new Error(`${command}: ${e.message}`)); });
    child.on('close', (code) => {
      clearTimeout(timer);
      if (code === 0) resolve(out);
      else reject(new Error(`${command} exited with ${code}${err.trim() ? `\n${err.trim()}` : ''}`));
    });
  });
}

// ---------- the round ----------

/// The state whose verdict the round's single-pair keys are filled from: the first one lost, and the
/// first one of all if none was.
///
/// The round file's shape is the gauntlet's, and the gauntlet judged one pair per Piece. Keeping it
/// is what lets `tools/progress.mjs` go on reading every round ever written; filling it from the
/// decisive state is what stops a Piece that lost on its third state showing the first state's
/// comfortable verdict on the progress page.
export function decisive(judged) {
  return judged.find((s) => s.winner === 'theirs') || judged[0];
}

/// The round file, in the shape every round since the gauntlet has been written in, plus the three
/// keys a native round needs that a gauntlet round did not: who the opponent was, which build of
/// ours it was, and what happened at each judged state.
export function round({ piece, number, judged, opponent, build: made, oracle, note, at }) {
  const head = decisive(judged);
  return {
    piece,
    round: number,
    winner: judged.every((s) => s.winner === 'ours') ? 'ours' : 'theirs',
    margin: head.margin,
    gap: head.gap,
    gapTheirs: head.gapTheirs,
    verdict: head.verdict,
    oursShot: head.ours,
    theirsShot: head.theirs,
    refSource: oracle,
    builderNote: note || '',
    secondary: head.secondary,
    opponent,
    build: made,
    states: judged,
    at,
  };
}

// ---------- the command ----------

function usage(where = process.stderr) {
  where.write(`usage: tools/gate judge <piece> [--note <text>]

  The Pieces with judged states are the keys of "pieces" in
  shots/oracle/states.json. What the command does: tools/gate --help
`);
}

/// Says something on the way to the verdict. Everything here is stderr: the owner reads the last
/// line on stdout, and the agent who has to fix something reads this above it.
function say(line) { process.stderr.write(`${line}\n`); }

/// The line the owner reads, and the code that agrees with it.
function verdict(piece, winner, number) {
  console.log(`gate judge ${piece}: ${winner}, round ${number}`);
}

/// Nothing was judged, and why. One code for all of them, because none of them is a verdict.
function refuse(piece, why) {
  console.log(`gate judge ${piece}: refused (${why})`);
  return 3;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);                       // states.json's paths are the repo's, and so are the shots'

  let piece = null;
  let note = '';
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--note') {
      note = argv[++i];
      if (note === undefined) { process.stderr.write('gate judge: --note takes the text to record\n'); usage(); return 2; }
    } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`gate judge: ${a}: not a flag this command has\n`); usage(); return 2; }
    else if (piece === null) piece = a;
    else { process.stderr.write('gate judge: one Piece at a time\n'); usage(); return 2; }
  }
  if (piece === null) { usage(); return 2; }

  try { return await judge(root, piece, note); }
  catch (e) {
    say(`gate judge ${piece}: ${e.message}`);
    return refuse(piece, 'the run broke before a verdict');
  }
}

async function judge(root, piece, note) {
  const states = readStates(root);
  let resolved;
  try { resolved = resolveStates(states, piece); }
  catch (e) {
    say(`gate judge: ${e.message}`);
    return refuse(piece, `${piece} is not a Piece with judged states`);
  }

  if (resolved.length === 0) {
    return refuse(piece, piece === 'latency'
      ? 'the latency Piece is benched, not judged — tools/gate bench'
      : 'this Piece has no judged states');
  }

  // Every refusal, for every state, before a window opens. The order is cheapest first and each one
  // names every state it applies to, so one run tells the agent the whole of what is missing.
  const blocked = resolved.map((s) => ({ ...s, cannot: unservable(states.defaults, s.flags) })).filter((s) => s.cannot.length);
  if (blocked.length) {
    for (const s of blocked) say(`gate judge ${piece}: state ${s.name} names ${s.cannot.join(', ')}`);
    say('gate judge: the app has no such flag yet; those states wait for the Chrome and File handling specs (shots/oracle/states.json)');
    return refuse(piece, `${blocked.length} of ${resolved.length} states name flags the app has not got`);
  }

  // The opponent is a directory of shots and the fingerprint of what took them. Half of that is not
  // an opponent: shots with no fingerprint beside them are shots nobody can say the provenance of.
  const oracleDir = path.join(root, 'shots/oracle', piece);
  const fingerprintFile = path.join(oracleDir, 'fingerprint.json');
  const unfrozen = resolved.filter((s) => !fs.existsSync(path.join(oracleDir, `${s.name}.png`))).map((s) => s.name);
  if (unfrozen.length || !fs.existsSync(fingerprintFile)) {
    const missing = unfrozen.length ? `for ${unfrozen.join(', ')}` : 'and has no fingerprint beside it';
    say(`gate judge ${piece}: no frozen opponent ${missing}; run tools/gate oracle ${piece}`);
    return refuse(piece, unfrozen.length
      ? `the Parity oracle is not frozen for ${unfrozen.length} of ${resolved.length} states`
      : 'the Parity oracle has no fingerprint');
  }

  // Judging against shots the oracle would no longer take is judging against the wrong opponent, and
  // it is invisible in the round afterwards. Checked here because it is four file reads, and skipped
  // once `legacy/` is gone, which is the retirement ticket's business rather than this command's.
  const frozen = JSON.parse(fs.readFileSync(fingerprintFile, 'utf8'));
  if (fs.existsSync(path.join(root, 'legacy/app'))) {
    const stale = freezeReason(frozen, fingerprint(root, resolved), resolved.map((s) => s.name));
    if (stale) {
      say(`gate judge ${piece}: the frozen opponent is out of date (${stale}); run tools/gate oracle ${piece}`);
      return refuse(piece, 'the Parity oracle is out of date');
    }
  }

  if (!compositorAvailable()) {
    say('gate judge: no Hyprland to open a window on; a judged shot needs the compositor (docs/research/native-harness.md)');
    return refuse(piece, 'there is no compositor to shoot on');
  }

  say(`gate judge ${piece}: building ${BINARY}`);
  try { execFileSync('cargo', ['build', '--release'], { cwd: root, stdio: ['ignore', 'ignore', 'inherit'] }); }
  catch { return refuse(piece, 'the binary would not build'); }

  const recorded = rounds(root, piece);
  const number = nextRound(recorded);
  const made = build(root);
  const template = fs.readFileSync(path.join(root, 'tools/critic.md'), 'utf8');
  const brief = JSON.parse(fs.readFileSync(path.join(root, 'progress/state.json'), 'utf8')).pieces.find((p) => p.id === piece);
  if (!brief?.judge) {
    say(`gate judge ${piece}: progress/state.json says nothing about what a critic judges this Piece on`);
    return refuse(piece, 'the Piece has no judging brief');
  }

  const judged = [];
  const stage = openStage({ root, appId: APP_ID });
  try {
    for (const s of resolved) {
      const ours = path.join('shots', piece, `r${number}-${s.name}-ours.png`);
      const theirs = path.join('shots/oracle', piece, `${s.name}.png`);
      say(`gate judge ${piece}: shooting ${s.name}`);
      await stage.shoot({
        bin: path.join(root, BINARY),
        argv: quillArgv(root, s.flags),
        w: s.flags.w,
        h: s.flags.h,
        out: path.join(root, ours),
        active: s.flags.active !== false,
      });

      const paired = pair(piece, s.name, ours, theirs);
      say(`gate judge ${piece}: ${s.name} paired at ${paired.dir}, asking a critic`);
      const answer = await runCritic(criticPrompt(template, { title: brief.title, judge: brief.judge }), paired.A, paired.B);

      // Revealed here, after the critic has answered and from a file the critic could not reach.
      const key = reveal(piece, s.name);
      const winner = answer.pick === key.ours ? 'ours' : 'theirs';
      say(`gate judge ${piece}: ${s.name}: ${winner} (${answer.margin})`);
      judged.push({
        name: s.name,
        ours,
        theirs,
        blind: pairDir(piece, s.name),
        oursWas: key.ours,
        pick: answer.pick,
        winner,
        margin: answer.margin,
        sameViewport: answer.sameViewport,
        gap: key.ours === 'A' ? answer.gapA : answer.gapB,
        gapTheirs: key.ours === 'A' ? answer.gapB : answer.gapA,
        verdict: answer.verdict,
        secondary: answer.secondary,
      });
    }
  } finally {
    stage.close();
  }

  const oracle = `shots/oracle/${piece}/ — the Parity oracle frozen by tools/gate oracle ${piece} from legacy/app ${frozen.app?.sha256} with legacy/tools/shoot.mjs ${frozen.shoot}`;
  const written = round({ piece, number, judged, opponent: OPPONENT, build: made, oracle, note, at: new Date().toISOString() });
  const file = path.join(root, 'progress/rounds', `${piece}-r${number}.json`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(written, null, 2)}\n`);
  say(`gate judge ${piece}: wrote ${path.relative(root, file)}`);

  if (written.winner === 'ours') { verdict(piece, 'ours', number); return 0; }

  const won = wonBefore(recorded);
  if (won) {
    console.log(`gate judge ${piece}: this Piece was won in round ${won.round} against the ${OPPONENTS[won.opponent] || won.opponent} and is lost now (docs/agents/gate.md: a Piece once won is never lost)`);
    verdict(piece, 'theirs', number);
    return 2;
  }
  verdict(piece, 'theirs', number);
  return 1;
}

// The exit code is set rather than taken: process.exit() can cut the last line short on its way down
// a pipe, and that line is the whole point of the command.
if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
