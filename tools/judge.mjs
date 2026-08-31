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
//   3  nothing was judged — a command line this could not read, a Piece with no judged states, a
//      state naming a flag the app has not got, an opponent that is not frozen or a crop that
//      cannot be cut, or a run that broke before a verdict
//
// The 0/1/2 are the verdict and are this command's own, which is why everything that is *not* a
// verdict is gathered under 3 — a mistyped flag included, where `check` and `oracle` would say 2.
// Sharing 2 between "this Piece has been lost" and "you typed the command wrong" would make the
// loudest thing this command can say indistinguishable from a typo.
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
//
// WHICH OPPONENT. The Parity oracle — `legacy/` frozen by `tools/gate oracle` — unless the state
// names one. A state carrying `opponent` in `shots/oracle/states.json` is judged against a crop of
// the Design oracle instead, because `docs/design.md` has taken that behaviour away from `legacy/`
// (ADR 0015): the pair is then the rectangle it names in a `ref/ia/shots/mac-native/` capture
// against the matching rectangle of ours, shot in Mono at the `defaults`' type so the two grids
// compare cell for cell. `tools/crop.mjs` is where that geometry and the cutting live.
import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { assertState, validate } from './assert-state.mjs';
import { BUDGET, ORACLE, latencyVerdict } from './bench-join.mjs';
import { pair, pairDir, reveal } from './blind.mjs';
import { CAPTURES, cropPng, resolveOpponent } from './crop.mjs';
import { gitHead, sha256 } from './fingerprint.mjs';
import { APP_ID, compositorAvailable, openStage, quillArgv } from './harness.mjs';
import { fingerprint, freezeReason, readStates, resolveStates, unservable } from './oracle.mjs';
import { regimes } from './regimes.mjs';
import { nextRound, opponentName, round, rounds, wonBefore } from './rounds.mjs';

// The opponent this command judges against while `legacy/` exists, and the word the round records
// it under. The switch to the iA reference belongs to the retirement ticket
// (`docs/architecture.md`, "Repo migration"), not here.
const OPPONENT = 'oracle';

// Who a round was judged against, read off the states rather than declared: a state carrying
// `opponent` is paired with a crop of the Design oracle instead of the Parity oracle's frozen shot
// (ADR 0015), and a Piece may hold both kinds while its rows are moved one at a time.
//
// What the word does is caption the round — `opponentName` in `tools/rounds.mjs`, which is what
// the progress page prints and what the line above a lost Piece says it was won against. What it
// does *not* do is scope "a Piece once won is never lost": `wonBefore` asks only whether a round
// carrying any `opponent` was won, so a Piece won over `legacy/` is still held to that win when its
// first row moves to a crop. That is the rule as `docs/agents/gate.md` states it, and narrowing it
// to the opponent would weaken the guard exactly as the re-judges begin (#165–#168).
// A state carrying `assert` is judged against nobody at all (ADR 0017), so it is read first: a
// Piece whose every state is asserted has no opponent to name, and one that holds an asserted
// state beside a paired one is as mixed as a Piece can be.
export function opponentOf(resolved) {
  if (resolved.every((s) => s.assert)) return 'asserted';
  if (resolved.some((s) => s.assert)) return 'mixed';
  if (resolved.every((s) => s.opponent)) return 'mac-native';
  if (resolved.some((s) => s.opponent)) return 'mixed';
  return OPPONENT;
}

// The binary a judged shot is of. Release rather than debug: the Gate judges what the owner would
// install, and a debug build's frame timings and font rasterisation are not the shipped ones.
const BINARY = 'target/release/quill';

// How long a critic is given. A critic reads two 2880x1800 images at least twice each and crops
// into them; ten minutes is far past what that has taken and short enough that a session which has
// stopped answering does not hold a Piece open all night.
const CRITIC_TIMEOUT_MS = 10 * 60 * 1000;

// How much of a failing build's output is kept. Node's own default is 1 MB, which a workspace-wide
// error set can pass; the whole of it is what the agent needs, so this is set well past that.
const BUILD_OUTPUT_MAX = 32 * 1024 * 1024;

// ---------- what produced the shots ----------

// Which build of ours a round is of: the commit it was taken at, and the binary itself, because a
// checkout can be dirty and a commit is then only half the answer. Both stamps come from
// `fingerprint.mjs`, which is where "which build is this?" is answered for the shots on the other
// side of the pair too.
export function build(root) {
  return { git: gitHead(root), binary: sha256(fs.readFileSync(path.join(root, BINARY))).slice(0, 16) };
}

// ---------- the command line ours is opened with ----------

// One judged state as ours' arguments, plus the settings file the round was asked for. That file is
// the one thing in a round that reaches ours and not the opponent: the opponent's side of every
// pair was frozen by `tools/gate oracle` before this round began, and the Parity oracle keeps its
// settings in localStorage rather than a file, so there is no command line to hand it one on.
export function oursArgv(root, flags, settingsFile) {
  const argv = quillArgv(root, flags);
  return settingsFile ? [...argv, '--settings', settingsFile] : argv;
}

// The flag ours refused, read out of what it said when it would not start, or null when it stopped
// for some other reason and the whole command line is the thing to report.
//
// There are two flag vocabularies here and they come apart on purpose. `shots/oracle/states.json`
// learns a flag the moment a tool under legacy/ can serve it, so the opponent can be frozen at a
// state; the app learns to parse it a whole spec later, when the feature lands. Between the two,
// the state is servable and unshootable at once, and only ours can say so.
export function refusedFlag(said) {
  const m = /(--[a-z-]+): not a flag Quill knows/.exec(said);
  return m ? m[1] : null;
}

// ---------- the critic ----------

// The prompt for one pair: `tools/critic.md` below its `---`, with its fields filled.
//
// Everything above the `---` is written for whoever reads the file and is never sent, which is why
// that half can explain itself at length without spending a critic's context on it.
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

// The critic's answer, out of whatever it said on the way to it.
//
// The last fenced JSON object wins: a critic that reasons in prose and then answers has written
// the answer last, and a critic that quotes the schema back before filling it in has written the
// example first. Every key the round needs is checked here rather than where it is read, so a
// malformed answer names itself instead of becoming an empty string in a round file.
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

// Runs one critic on one pair and answers with what it said.
//
// The session's working directory is a scratch directory outside the checkout holding copies of
// `A.png` and `B.png` and nothing else, and the two tools it is allowed are the two the prompt asks
// it to use. `--safe-mode` is the load-bearing one: it turns off this repository's own
// instructions, and a critic that has read `CLAUDE.md` knows there is a JavaScript app in
// `legacy/` it is probably being asked about, which is not a blind critic. The header above says
// where that stops being enforcement and starts being instruction.
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

// A child process's stdout, or its stderr as the reason it has none.
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

// ---------- the command ----------

function usage(where = process.stderr) {
  where.write(`usage: tools/gate judge <piece> [--note <text>] [--summary <file>] [--settings <path>]

  The Pieces with judged states are the keys of "pieces" in
  shots/oracle/states.json. What the command does: tools/gate --help

  --settings is ours' settings file for the run, and goes to ours only: the
  Parity oracle has no such file, and its shots are frozen before the round
  starts. The round records the path under "build".

  latency is judged on numbers rather than by a critic: it reads the newest
  shots/latency/summary-*.json that tools/gate bench --all wrote, or the one
  --summary names, and answers with arithmetic.
`);
}

// Says something on the way to the verdict. None of it is printed as it is said: the owner reads one
// line on stdout and an agent pays for every other one, so the trail goes to
// target/gate/judge-<piece>.log and is handed over only when the run ends in no verdict at all. A
// run that reached a verdict has its detail in the round it wrote.
const trail = [];
let logFile = null;
function say(line) {
  trail.push(line);
  if (!logFile) return;
  // The log is a convenience and never the thing that decides a run: a target/ that cannot be
  // written to must not turn a verdict into a stack trace. Given up on at the first refusal, so a
  // full disk is not one failed write per line; the trail itself is kept either way.
  try { fs.appendFileSync(logFile, `${line}\n`); } catch { logFile = null; }
}

// The trail, for the agent who has to fix what stopped this.
function spill() {
  if (trail.length) process.stderr.write(`${trail.join('\n')}\n`);
}

// The file the trail is written to as it is said, so a run that is still going, or one killed
// part-way, can be read from another terminal. Silent when it cannot be opened, for the reason
// say() is.
function openLog(root, piece) {
  const file = path.join(root, 'target/gate', `judge-${piece}.log`);
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, `gate judge ${piece} — ${new Date().toISOString()}\n`);
    logFile = file;
  } catch { logFile = null; }
}

// The line the owner reads, and the code that agrees with it.
function verdict(piece, winner, number) {
  console.log(`gate judge ${piece}: ${winner}, round ${number}`);
}

// Nothing was judged, and why. One code for all of them, because none of them is a verdict. The
// trail comes out here and nowhere else: this is the only ending an agent has to fix.
function refuse(piece, why) {
  spill();
  console.log(`gate judge ${piece}: refused (${why})`);
  return 3;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);                       // states.json's paths are the repo's, and so are the shots'

  let piece = null;
  let note = '';
  let summaryFile = null;
  let settingsFile = null;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--note') {
      note = argv[++i];
      if (note === undefined) { process.stderr.write('gate judge: --note takes the text to record\n'); usage(); return 3; }
    } else if (a === '--summary') {
      summaryFile = argv[++i];
      if (summaryFile === undefined) { process.stderr.write('gate judge: --summary takes the bench summary to read\n'); usage(); return 3; }
    } else if (a === '--settings') {
      settingsFile = argv[++i];
      if (settingsFile === undefined) { process.stderr.write('gate judge: --settings takes the settings file to open ours with\n'); usage(); return 3; }
    } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    else if (a.startsWith('-')) { process.stderr.write(`gate judge: ${a}: not a flag this command has\n`); usage(); return 3; }
    else if (piece === null) piece = a;
    else { process.stderr.write('gate judge: one Piece at a time\n'); usage(); return 3; }
  }
  if (piece === null) { usage(); return 3; }
  openLog(root, piece);

  try { return await judge(root, piece, note, summaryFile, settingsFile); }
  catch (e) {
    say(`gate judge ${piece}: ${e.message}`);
    return refuse(piece, 'the run broke before a verdict');
  }
}

// Where `tools/gate bench --all` leaves what this reads, and the oracle's own report beside it.
const SUMMARIES = 'shots/latency';
const ORACLE_REPORT = 'progress/latency-report.md';

// The newest summary a bench run wrote, by the stamp in its name — which sorts by time because it
// is `YYYYMMDDTHHMMSS`, and is the run's own idea of when it happened rather than the file
// system's, so a checkout does not reorder them.
function newestSummary(root) {
  const dir = path.join(root, SUMMARIES);
  if (!fs.existsSync(dir)) return null;
  const found = fs.readdirSync(dir).filter((f) => /^summary-.*\.json$/.test(f)).sort();
  return found.length ? path.join(SUMMARIES, found[found.length - 1]) : null;
}

/// The latency Piece's round: arithmetic over a bench run, recorded in the ledger's own shape.
///
/// No window, no critic and no build. What decides it is `latencyVerdict` in `tools/bench-join.mjs`
/// — the budget as the floor and the Parity oracle's own numbers as what winning means — and what
/// this does is find the run, refuse the ones that are not evidence, and write the round.
async function judgeLatency(root, note, named) {
  const file = named ? path.relative(root, path.resolve(root, named)) : newestSummary(root);
  if (!file || !fs.existsSync(path.join(root, file))) {
    say(`gate judge latency: no bench summary ${named ? `at ${named}` : `under ${SUMMARIES}/`}`);
    return refuse('latency', named
      ? `${named} is not a file to read`
      : 'there is no bench run to judge; tools/gate bench --all writes one');
  }

  let summary;
  try { summary = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8')); }
  catch (e) {
    say(`gate judge latency: ${file}: ${e.message}`);
    return refuse('latency', `${file} is not a bench summary`);
  }

  // A panel run is a real measurement and not a measurement of this. It is taken on the physical
  // display at fractional scale, which is not the output the budget or the oracle's numbers are of,
  // and `tools/gate bench --panel` says so in every line it prints and in the file it writes.
  //
  // It cannot arrive here by itself — a panel run writes `panel-summary-*.json`, which
  // `newestSummary` does not match — so this is the guard for the other way in: `--summary` with a
  // panel run named on purpose, which is easy to do by tab-completion and impossible to spot in a
  // round afterwards.
  if (summary.informational) {
    say(`gate judge latency: ${file} is a --panel run`);
    return refuse('latency', `${file} is informational: ${summary.informational}`);
  }

  // A subset run is a real measurement and not a verdict on the Piece: the rule is every regime
  // within budget, so a round written from two of them would be recording a win nobody had.
  //
  // Asked of the regimes the summary actually holds, never of `ran`: that is the line the run
  // printed for a human to read, and a verdict that turned on its exact wording would be one
  // rewording away from judging a subset as though it were the whole twelve.
  const held = new Set((summary.regimes || []).map((row) => row.regime));
  const short = (summary.regimes_not_run || []).concat(
    regimes().map((r) => r.name).filter((name) => !held.has(name)),
  );
  if (short.length) {
    say(`gate judge latency: ${file} ran ${summary.ran}, and the Piece is judged on all twelve`);
    return refuse('latency', `${file} is not a whole run — ${short.join(', ')} missing`);
  }
  if ((summary.regimes_unaccounted_for || []).length) {
    return refuse('latency', `${file} could not account for every keystroke in `
      + `${summary.regimes_unaccounted_for.join(', ')}`);
  }

  const said = latencyVerdict(summary);
  const recorded = rounds(root, 'latency');
  const number = nextRound(recorded);
  const oracle = `${ORACLE_REPORT} — the Parity oracle's own numbers, measured by `
    + `legacy/tools/latency.mjs on the JavaScript app as it won its gauntlet: ${ORACLE.mean_ms} ms mean, `
    + `${ORACLE.worst_ms} ms worst, ${ORACLE.uinput_to_presented_ms} ms uinput → presented, `
    + `${ORACLE.cold_ms} ms cold. Not a screenshot piece — the pair is a run against a report.`;

  const written = round({
    piece: 'latency',
    number,
    judged: said.states,
    opponent: OPPONENT,
    // The commit is not the build when the tree was dirty, and a round that quotes only the commit
    // invites a later reader to check out `5f1311a` and wonder why the numbers will not come back.
    // Taken from the fingerprint, which is where the fact is recorded, rather than from the
    // summary's `build`, which is the short form for a human reading the run.
    build: { ...summary.build, dirty: summary.fingerprint?.app?.tree_was_dirty ?? null },
    oracle,
    note,
    at: new Date().toISOString(),
    // The pair this round names is the whole run against the oracle's report, which is not
    // something any one regime says — see `round` in tools/rounds.mjs.
    headline: {
      margin: said.margin,
      gap: said.gap,
      gapTheirs: said.gapTheirs,
      verdict: said.verdict,
      ours: file,
      theirs: ORACLE_REPORT,
      secondary: [
        `budget: mean <= ${BUDGET.mean_ms} ms, worst <= ${BUDGET.worst_ms} ms, cold <= ${BUDGET.cold_ms} ms`,
        `won by beating the oracle at ${summary.headline}, not by clearing the budget`,
        `bench run: ${summary.ran} at ${summary.at}`,
      ],
    },
  });

  const out = path.join(root, 'progress/rounds', `latency-r${number}.json`);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, `${JSON.stringify(written, null, 2)}\n`);
  say(`gate judge latency: wrote ${path.relative(root, out)}`);

  if (written.winner === 'ours') { verdict('latency', 'ours', number); return 0; }

  const won = wonBefore(recorded);
  if (won) {
    console.log(`gate judge latency: this Piece was won in round ${won.round} against the ${opponentName(won)} and is lost now (docs/agents/gate.md: a Piece once won is never lost)`);
    verdict('latency', 'theirs', number);
    return 2;
  }
  verdict('latency', 'theirs', number);
  return 1;
}

async function judge(root, piece, note, summaryFile, settingsFile) {
  // The latency Piece is not shot and not paired: its opponent is a set of numbers, so its round is
  // arithmetic over what `tools/gate bench --all` already measured. Answered before anything below
  // opens a window or builds a binary, because none of that is needed to read two files.
  // --settings is a flag ours is opened with, and latency opens nothing here: taking it and doing
  // nothing with it would put a settings file in the round's build that never touched the numbers.
  if (piece === 'latency') {
    if (settingsFile) {
      say('gate judge latency: --settings is ours\' settings file for a shot, and the latency Piece shoots nothing');
      return refuse(piece, '--settings is not a flag the latency Piece has');
    }
    return judgeLatency(root, note, summaryFile);
  }

  const states = readStates(root);
  let resolved;
  try { resolved = resolveStates(states, piece); }
  catch (e) {
    say(`gate judge: ${e.message}`);
    return refuse(piece, `${piece} is not a Piece with judged states`);
  }

  if (resolved.length === 0) {
    return refuse(piece, 'this Piece has no judged states');
  }

  // Every refusal, for every state, before a window opens. The order is cheapest first and each one
  // names every state it applies to, so one run tells the agent the whole of what is missing.
  const blocked = resolved.map((s) => ({ ...s, cannot: unservable(states.defaults, s.flags) })).filter((s) => s.cannot.length);
  if (blocked.length) {
    for (const s of blocked) say(`gate judge ${piece}: state ${s.name} names ${s.cannot.join(', ')}`);
    say('gate judge: the app has no such flag yet; those states wait for the File handling spec (shots/oracle/states.json)');
    return refuse(piece, `${blocked.length} of ${resolved.length} states name flags the app has not got`);
  }

  // A state carrying `opponent` is judged against a crop of the Design oracle, which is committed
  // under `ref/ia/shots/mac-native/` rather than frozen by `tools/gate oracle` — so the Parity
  // oracle is asked about the other states only, and a Piece with none of those wants no frozen
  // shot and no fingerprint at all (ADR 0015).
  // A state carrying `assert` has no opponent of either kind: it is answered by arithmetic off
  // ours' own pixels, because neither oracle holds the thing it is about (ADR 0017).
  const parity = resolved.filter((s) => !s.opponent && !s.assert);
  const crops = resolved.filter((s) => s.opponent);
  const asserted = resolved.filter((s) => s.assert);

  // The assertions, checked for a name this build has before the first window opens, for the same
  // reason the crops are: a state naming a rule nobody wrote is a state that cannot be judged, and
  // finding that out at the third state has already spent two critics.
  const unreadable = [];
  for (const s of asserted) {
    // A state is answered one way or the other and never both. Carrying an opponent as well is a
    // state that says it is paired and says it is measured, and there is no honest order to read
    // those in — so it is refused rather than resolved by whichever branch happens to run first.
    if (s.opponent) {
      say(`gate judge ${piece}: ${s.name} names both an opponent and an assertion, and a state is answered one way`);
      unreadable.push(s.name);
      continue;
    }
    try {
      validate(s.assert);
    } catch (e) {
      say(`gate judge ${piece}: ${s.name}'s assertion cannot be read: ${e.message}`);
      unreadable.push(s.name);
    }
  }
  if (unreadable.length) {
    return refuse(piece, `${unreadable.length} of ${resolved.length} states name an assertion this build cannot run`);
  }

  // The crops, resolved against the captures on disk before the first window opens, for the reason
  // every other refusal is checked here: a capture that is not there, or a rectangle that runs off
  // one, is not something to find out at the third state with two critics already spent.
  const cropping = new Map();
  const uncroppable = [];
  for (const s of crops) {
    try {
      cropping.set(s.name, resolveOpponent(root, s.name, s.opponent, { w: s.flags.w * s.flags.scale, h: s.flags.h * s.flags.scale }));
    } catch (e) {
      say(`gate judge ${piece}: ${e.message}`);
      uncroppable.push(s.name);
    }
  }
  if (uncroppable.length) {
    return refuse(piece, `${uncroppable.length} of ${resolved.length} states name a mac-native crop that cannot be cut`);
  }

  // The opponent is a directory of shots and the fingerprint of what took them. Half of that is not
  // an opponent: shots with no fingerprint beside them are shots nobody can say the provenance of.
  const oracleDir = path.join(root, 'shots/oracle', piece);
  const fingerprintFile = path.join(oracleDir, 'fingerprint.json');
  const unfrozen = parity.filter((s) => !fs.existsSync(path.join(oracleDir, `${s.name}.png`))).map((s) => s.name);
  if (parity.length && (unfrozen.length || !fs.existsSync(fingerprintFile))) {
    const missing = unfrozen.length ? `for ${unfrozen.join(', ')}` : 'and has no fingerprint beside it';
    say(`gate judge ${piece}: no frozen opponent ${missing}; run tools/gate oracle ${piece}`);
    return refuse(piece, unfrozen.length
      ? `the Parity oracle is not frozen for ${unfrozen.length} of ${parity.length} states`
      : 'the Parity oracle has no fingerprint');
  }

  // Judging against shots the oracle would no longer take is judging against the wrong opponent, and
  // it is invisible in the round afterwards. Checked here because it is four file reads, and skipped
  // once `legacy/` is gone, which is the retirement ticket's business rather than this command's.
  const frozen = parity.length ? JSON.parse(fs.readFileSync(fingerprintFile, 'utf8')) : null;
  if (parity.length && fs.existsSync(path.join(root, 'legacy/app'))) {
    const stale = freezeReason(frozen, fingerprint(root, parity), parity.map((s) => s.name));
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
  // cargo's own words go into the trail rather than past it: a build that will not build is the one
  // ending where an agent needs every line, and it gets them all together at the bottom.
  try { execFileSync('cargo', ['build', '--release'], { cwd: root, stdio: ['ignore', 'ignore', 'pipe'], maxBuffer: BUILD_OUTPUT_MAX }); }
  catch (e) {
    const said = String(e.stderr || '').trim();
    if (said) say(said);
    return refuse(piece, 'the binary would not build');
  }

  // Whether ours can be opened at all at each state, asked of the binary rather than kept in a list
  // here, so it cannot drift from what the app actually parses: `--help` makes it read the whole
  // command line and print instead of opening a window. Asked here, after the build that is the
  // only way to ask and before the first pair, because a round that learns this at its third state
  // has already put a critic on the first two. Every state is named in one go, the way the refusals
  // above name theirs.
  const bin = path.join(root, BINARY);
  const cannot = [];
  for (const s of resolved) {
    try {
      execFileSync(bin, [...oursArgv(root, s.flags, settingsFile), '--help'], { stdio: ['ignore', 'ignore', 'pipe'] });
    } catch (e) {
      const flag = refusedFlag(String(e.stderr || ''));
      say(`gate judge ${piece}: state ${s.name} opens ours with ${flag ?? 'a command line it would not take'}`);
      cannot.push(s.name);
    }
  }
  if (cannot.length) {
    say(`gate judge: ours will not open at ${cannot.join(', ')}; those flags wait for the spec that teaches the app to parse them`);
    return refuse(piece, `${cannot.length} of ${resolved.length} states name flags the app has not got`);
  }

  const recorded = rounds(root, piece);
  const number = nextRound(recorded);
  // What ours was: the commit, the binary, and the settings file it was opened with. The last one
  // is null in almost every round, and saying so is the point — a round that shot ours against a
  // rebinding fixture reads differently from one that shot it with its own defaults, and the shots
  // do not say which it was.
  const ours = { ...build(root), settings: settingsFile ?? null };
  const template = fs.readFileSync(path.join(root, 'tools/critic.md'), 'utf8');
  const brief = JSON.parse(fs.readFileSync(path.join(root, 'progress/state.json'), 'utf8')).pieces.find((p) => p.id === piece);
  if (!brief?.judge) {
    say(`gate judge ${piece}: progress/state.json says nothing about what a critic judges this Piece on`);
    return refuse(piece, 'the Piece has no judging brief');
  }

  const judged = [];
  const stage = await openStage({ root, appId: APP_ID });
  try {
    for (const s of resolved) {
      const shot = path.join('shots', piece, `r${number}-${s.name}-ours.png`);
      say(`gate judge ${piece}: shooting ${s.name}`);
      await stage.shoot({
        bin: path.join(root, BINARY),
        argv: oursArgv(root, s.flags, settingsFile),
        w: s.flags.w,
        h: s.flags.h,
        out: path.join(root, shot),
        active: s.flags.active !== false,
      });

      // An asserted state is answered here and never paired: it is shot a second time with the
      // window active, and the rule is read off the two shots (ADR 0017). Both are kept, because
      // the measurement is only checkable by someone who has the pixels it was taken from.
      if (s.assert) {
        const lit = path.join('shots', piece, `r${number}-${s.name}-ours-lit.png`);
        await stage.shoot({
          bin: path.join(root, BINARY),
          argv: oursArgv(root, s.flags, settingsFile),
          w: s.flags.w,
          h: s.flags.h,
          out: path.join(root, lit),
          active: true,
        });
        const answer = assertState(s.assert, {
          lit: fs.readFileSync(path.join(root, lit)),
          dim: fs.readFileSync(path.join(root, shot)),
        });
        const winner = answer.ours ? 'ours' : 'theirs';
        say(`gate judge ${piece}: ${s.name}: ${winner} (asserted: ${answer.why})`);
        judged.push({
          name: s.name,
          ours: shot,
          // No opponent shot, and `null` rather than the lit one: the second shot is ours as well,
          // and a round that named it `theirs` would have the progress page caption our own window
          // as somebody else's. It is kept under its own key, because the measurement is only
          // checkable by someone holding both frames it was taken from.
          theirs: null,
          lit,
          assert: { ...s.assert, held: answer.ours },
          winner,
          margin: 'asserted',
          sameViewport: true,
          gap: answer.why,
          gapTheirs: 'no opponent: neither iA Writer for Mac nor legacy/ holds this state (ADR 0017)',
          verdict: answer.why,
          secondary: answer.secondary,
        });
        continue;
      }

      // What the critic is shown. For a Parity oracle state that is the two whole windows, as it
      // has always been. For a Design oracle state it is the two rectangles, cut here: the windows
      // are 3024 x 1898 and 2880 x 1800, so whole against whole would put a critic on the two
      // apps' chrome rather than on the row the state is about (ADR 0015). The whole shot of ours
      // stays on disk beside its crop — it is the evidence the crop was cut from.
      const cut = cropping.get(s.name);
      let ours = shot;
      let theirs = path.join('shots/oracle', piece, `${s.name}.png`);
      if (cut) {
        ours = path.join('shots', piece, `r${number}-${s.name}-ours-crop.png`);
        theirs = path.join('shots', piece, `r${number}-${s.name}-theirs-crop.png`);
        fs.writeFileSync(path.join(root, ours), cropPng(fs.readFileSync(path.join(root, shot)), cut.ours));
        fs.writeFileSync(path.join(root, theirs), cropPng(fs.readFileSync(path.join(root, cut.capture)), cut.crop));
        say(`gate judge ${piece}: ${s.name} cropped to ${cut.crop[2]}x${cut.crop[3]} against ${cut.capture}`);
      }

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
        // Which capture the crop came out of and both rectangles, so a round says what a critic
        // was shown without anyone having to re-derive it from states.json as it reads today.
        ...(cut ? { opponent: { capture: cut.capture, crop: cut.crop, ours: cut.ours }, oursWhole: shot } : {}),
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

  // Where the other side of every pair came from, in one sentence per opponent the round used.
  const sources = [];
  if (parity.length) sources.push(`shots/oracle/${piece}/ — the Parity oracle frozen by tools/gate oracle ${piece} from legacy/app ${frozen.app?.sha256} with legacy/tools/shoot.mjs ${frozen.shoot}`);
  if (crops.length) sources.push(`${CAPTURES}/ — the Design oracle, iA Writer for Mac as captured and measured in ref/ia/mac-native/ (ADR 0015), cropped per state: ${crops.map((s) => `${s.name} from ${path.basename(cropping.get(s.name).capture)}`).join(', ')}`);
  if (asserted.length) sources.push(`no opponent for ${asserted.map((s) => `${s.name} (${s.assert.kind})`).join(', ')} — measured off ours' own pixels by tools/assert-state.mjs, because neither oracle holds the state (ADR 0017)`);
  const oracle = sources.join('; ');
  const written = round({ piece, number, judged, opponent: opponentOf(resolved), build: ours, oracle, note, at: new Date().toISOString() });
  const file = path.join(root, 'progress/rounds', `${piece}-r${number}.json`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(written, null, 2)}\n`);
  say(`gate judge ${piece}: wrote ${path.relative(root, file)}`);

  if (written.winner === 'ours') { verdict(piece, 'ours', number); return 0; }

  const won = wonBefore(recorded);
  if (won) {
    console.log(`gate judge ${piece}: this Piece was won in round ${won.round} against the ${opponentName(won)} and is lost now (docs/agents/gate.md: a Piece once won is never lost)`);
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
