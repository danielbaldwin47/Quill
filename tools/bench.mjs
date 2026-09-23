#!/usr/bin/env node
// `tools/gate bench` — the latency Piece, measured with real keys on a window of our own.
//
//   tools/gate bench [regime] [--keys N] [--sessions N]
//
// One run: build the binary, open the stage's headless output, launch ours on the 10k-word
// document with the caret at the end of it, take keyboard focus and *prove* we have it, type the
// regime through `/dev/uinput`, and join what was written against what the app said it presented.
// It ends in the line the owner reads — mean, worst, p50, p99, cold start, and pass or fail against
// the Gate's budget with the Parity oracle's own numbers beside it.
//
// Four things here are not obvious and all four are deliberate.
//
// **It refuses rather than types.** Both injection paths go to whatever the compositor thinks has
// keyboard focus, and in the research's own session focus silently reverted to the owner's browser
// and a burst of keys landed in their YouTube tab. So focus is read back from the compositor before
// the first key and between every chunk, and a run that cannot prove the keys will land on ours
// does not send any.
//
// **The measurement is a subtraction across two processes**, and the arithmetic is not here: it is
// in `tools/bench-join.mjs`, which is a pure function of the two files and is tested by
// `tools/bench-selftest.mjs` under `tools/gate check`. This file is the part that cannot be
// tested without a compositor, and it is kept as thin as that division allows.
//
// **The warm-up is typed and thrown away.** A regime is 300 keys after 25 that pay for first touch
// of the editing machinery, which is how the oracle's numbers were measured; comparing a cold ours
// against a warm theirs would flatter neither honestly. Both go through one keyboard, as the
// oracle's went through one page, and the join is given only the lines written after the warm-up.
//
// **Typing waits for the app to say its launch is over.** For seconds after its first frame a
// launch paints frames no key asked for — the Annotators' first pass, GTK hiding the scrollbar the
// launch's own scroll showed — and a key inside the same refresh as one waits for the next: 16.25
// ms on an unchanged build, once key 0 came earlier (#489). Fixed waits used to outlast that work
// by about 0.8 s, which nothing checked. Now `--measure` prints `launch settled` and then `launch
// quiet`; the warm-up starts at the one, is topped up until the other, and a run whose first
// measured key still went first is refused (#495).

import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  BUDGET, NOT_SCORED, ORACLE, allSummary, launchSaid, measure, pointerLeft, regimeLine, scoredRow, summary,
  verdict, writeGaps,
} from './bench-join.mjs';
import { gitHead } from './fingerprint.mjs';
import {
  APP_ID, PANEL_IDLE_S, PANEL_WORKSPACE, Refusal, chosenList, compositorAvailable, launchEnv,
  openPanelStage, openStage, panelBlocked,
} from './harness.mjs';
import {
  DEFAULT_KEYS, PASTE_TEXT, PAUSE_MS, REFRESH_MS, TOP_UP_MS, UINPUT_PLAN, WARMUP_KEYS, hash32, regimes, scoredRegime,
  script, topUp, uinputPlan,
} from './regimes.mjs';

// The binary a bench is of. Release rather than debug, for the reason a judged shot is: the Gate
// measures what the owner would install, and a debug build's frame timings are not the shipped
// ones.
const BINARY = 'target/release/quill';

// The document the headline regime is typed into: 10,062 words, the size a draft actually is.
const DOC = 'dev/shots/latency/doc10k.md';

// The window a bench runs in — the judged states' size, so that the layout being timed is the
// layout being judged.
const WINDOW = { w: 1440, h: 900 };

// Where results land, and which regime `tools/gate bench` runs when told nothing else.
const RESULTS = 'dev/shots/latency';
const HEADLINE = 'prose_end_of_draft';

// What `informational` says inside a panel result. A sentence rather than `true`, because the field
// exists for whoever opens the JSON a year from now, and `true` answers a question they have not
// asked yet. Truthy either way, which is what `tools/gate judge latency` tests it for.
const PANEL_IS_INFORMATIONAL = 'the physical panel is never a Gate condition: this run was taken on '
  + 'a fractional-scale output rather than the headless stage the budget and the oracle numbers '
  + 'belong to, so its numbers are comparable only with another panel run of the same output';

// How long the app's capture is given to stop growing before it is read. The app drains its stamps
// every 100 ms and a stamp waits at most 250 ms (`harness.rs`'s `TAIL`) for its frame, so a file
// that has not grown for `QUIET_MS` is whole. `SETTLE_MS` is the most a wait costs when the file
// keeps growing, which nothing but a run still typing does, and the plain wait where there is no
// capture to watch.
const QUIET_MS = 500;
const SETTLE_MS = 1_500;
const DRAIN_POLL_MS = 100;

// How long one chunk of keys has to come back before the injector is called hung. A chunk is 25
// keys at 90 ms, so this is many times what it takes.
const CHUNK_TIMEOUT_MS = 30_000;

// How long a launch is given to say it is `settled` before the regime is refused untyped. It says
// so 0.3–1.4 s after `exec` (#495), the Annotators' first pass being the long end.
const LAUNCH_WAIT_MS = 10_000;
const SAID_POLL_MS = 10;

// How much of a failing build's output is kept; Node's own 1 MB default can be passed by a
// workspace-wide error set.
const BUILD_OUTPUT_MAX = 32 * 1024 * 1024;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------- the trail ----------
//
// The same division `tools/gate judge` makes: stdout is the summary lines and nothing else, and
// everything the run said on the way is in a log that is named only when the run refuses.

const trail = [];
let logFile = null;
function say(line) {
  trail.push(line);
  if (!logFile) return;
  try { fs.appendFileSync(logFile, `${line}\n`); } catch { logFile = null; }
}

function openLog(root) {
  const file = path.join(root, 'target/gate', 'bench.log');
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, `gate bench — ${new Date().toISOString()}\n`);
    logFile = file;
  } catch { logFile = null; }
}

// Nothing was measured, and why. One code for all of them, because none of them is a number.
function refuse(why) {
  if (trail.length) process.stderr.write(`${trail.join('\n')}\n`);
  console.log(`gate bench: refused (${why})`);
  return 3;
}

function usage(to = process.stderr) {
  to.write(`usage: tools/gate bench [regime] [--all] [--regimes a,b] [--keys N] [--sessions N]
                        [--panel] [--idle-window S]

  regime          which of the nineteen to type; ${HEADLINE} by default
  --all           every one of the nineteen, one line each and one line for the run
  --regimes       just these, by name, separated by commas
  --keys N        keys measured per session (${DEFAULT_KEYS} by default, the oracle's count)
  --sessions N    launch and type this many times, for a run-to-run interval (1 by default)
  --panel         measure on the physical display, on workspace ${PANEL_WORKSPACE}, and only while
                  nobody is at the machine; informational, never a Gate condition
  --idle-window S how long everything has to have been quiet before --panel takes
                  the screen (${PANEL_IDLE_S}s by default)

A regime name, --all and --regimes each say which regimes to run, so only one of them may be given.
A run of several writes ${RESULTS}/summary-<stamp>.json beside the per-regime results, which is what
tools/gate judge latency reads. A --panel run of several writes ${RESULTS}/panel-summary-<stamp>.json
instead, which judge does not read and never will: the panel is a fractional-scale output, its
numbers are not the ones the budget is set on, and nothing about it decides a Piece. Every --panel
result file, one regime or nineteen, is marked informational inside and named bench-panel-<regime>-.
`);
}

// ---------- what produced the numbers ----------

// Everything about this machine and this build that a second run would have to match for the two to
// be comparable. The same question `dev/legacy/tools/latency.mjs` answers for the oracle's numbers, so
// that a native result and an oracle result can be read side by side.
function fingerprint(root, stage) {
  let dirty = null;
  try {
    dirty = execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }).trim() !== '';
  } catch { /* not a checkout */ }

  let busiest = [];
  try {
    busiest = execFileSync('ps', ['-eo', 'pcpu,comm', '--sort=-pcpu'], { encoding: 'utf8' })
      .split('\n').slice(1, 6).map((l) => l.trim()).filter(Boolean);
  } catch { /* no ps */ }

  const monitor = stage.monitor || {};
  const doc = fs.readFileSync(path.join(root, DOC), 'utf8');
  return {
    at: new Date().toISOString(),
    app: {
      binary: BINARY,
      sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(root, BINARY))).digest('hex').slice(0, 16),
      git_head: gitHead(root),
      tree_was_dirty: dirty,
      // The renderer every launch is pinned to by `launchEnv` (`gl`, byte-identical to `ngl` and
      // `vulkan` in a judged shot). #41 asks that the choice be in the fingerprint the moment it is
      // a choice, and it has been one since the harness pinned it.
      renderer: launchEnv().GSK_RENDERER,
    },
    display: {
      output: monitor.name ?? stage.output,
      mode: monitor.width && monitor.height ? `${monitor.width}x${monitor.height}` : null,
      refresh_hz: monitor.refreshRate ?? null,
      scale: monitor.scale ?? null,
      vrr: monitor.vrr ?? null,
    },
    machine: {
      node: process.version,
      kernel: `${os.type()} ${os.release()}`,
      cpu: os.cpus()[0]?.model ?? null,
      cpus: os.cpus().length,
      load1: os.loadavg()[0],
      busiest_processes: busiest,
      mem_gb: Math.round((os.totalmem() / 1024 ** 3) * 10) / 10,
    },
    document: {
      path: DOC,
      words: doc.split(/\s+/).filter(Boolean).length,
      chars: doc.length,
      lines: doc.split('\n').length,
    },
    window: WINDOW,
  };
}

// ---------- the injector ----------

// One line at a time out of a stream, plus whatever is left at the end. `tools/uinput-keys.py`
// frames every message with a newline except its closing summary, which it writes as it exits, so
// both halves are needed.
function reader(stream) {
  let buffered = '';
  const ready = [];
  const waiting = [];
  stream.on('data', (chunk) => {
    buffered += chunk;
    for (let at = buffered.indexOf('\n'); at >= 0; at = buffered.indexOf('\n')) {
      const line = buffered.slice(0, at);
      buffered = buffered.slice(at + 1);
      if (waiting.length) waiting.shift()(line);
      else ready.push(line);
    }
  });
  return {
    next: (ms = CHUNK_TIMEOUT_MS) => new Promise((resolve, reject) => {
      if (ready.length) { resolve(ready.shift()); return; }
      const timer = setTimeout(() => reject(new Error('the injector stopped answering')), ms);
      waiting.push((line) => { clearTimeout(timer); resolve(line); });
    }),
    rest: () => buffered,
  };
}

/// Makes the run's one keyboard and waits out its settle, and answers with it.
///
/// Once a run rather than once a regime: a new device costs `UINPUT_PLAN.settle_ms` before the
/// compositor types with it, and the bench opens this one while the stage warms, so no regime
/// waits for it (#495). `close` ends the injector's stdin, which removes the device.
export async function openInjector(root) {
  const py = spawn('python3', [path.join(root, 'tools/uinput-keys.py')], {
    cwd: root, stdio: ['pipe', 'pipe', 'pipe'],
  });
  const said = { stderr: '' };
  py.stderr.on('data', (b) => { said.stderr += b; });
  const closed = new Promise((resolve) => py.on('close', resolve));
  const injector = {
    py, out: reader(py.stdout), said,
    close: async () => { py.stdin.end(); await closed; },
  };
  try {
    py.stdin.write(`${JSON.stringify({ ...UINPUT_PLAN, reuse: true, keys: [] })}\n`);
    // Its settle is inside this first answer, so the wait is the settle's and not a chunk's.
    await started(injector.out, CHUNK_TIMEOUT_MS + UINPUT_PLAN.settle_ms);
    JSON.parse(await injector.out.next());
  } catch (e) {
    try { py.kill('SIGTERM'); } catch { /* already gone */ }
    throw new Error(`${e.message}${stderrOf(said)}`);
  }
  return injector;
}

/// Types one plan through the run's keyboard, re-verifying focus between every chunk, and answers
/// with what it wrote.
///
/// The chunk protocol is the whole defence: the injector types `chunk` keys and waits to be told to
/// type the next lot, so focus is a question asked every 25 keys rather than once at the start. A
/// run that loses focus stops at the chunk boundary and says so, rather than typing the rest of a
/// draft into whatever took it.
///
/// `enough`, when given, makes the plan's keys from `enough.from` on a top-up rather than a
/// script: they go `enough.step` at a time, and the plan ends at the first boundary where
/// `enough.met()` says so. A plan whose every key went without it being met answers `met: false`.
export async function typeKeys(injector, plan, held, { enough = null } = {}) {
  const { py, out, said } = injector;
  let lost = false;
  let met = enough === null;
  let rechecks = 0;

  try {
    py.stdin.write(`${JSON.stringify(plan)}\n`);
    const ready = await started(out);

    let typed = 0;
    while (typed < ready.keys) {
      if (enough && typed >= enough.from && enough.met()) { met = true; break; }
      rechecks += 1;
      if (!held()) { lost = true; break; }
      const n = enough && typed >= enough.from ? enough.step : Math.min(ready.chunk, (enough?.from ?? ready.keys) - typed);
      py.stdin.write(`go ${n}\n`);
      const ack = JSON.parse(await out.next());
      if (!ack.typed) throw new Error(`the injector typed none of the ${ready.keys - typed} keys left`);
      typed += ack.typed;
    }
    // A plan stopped short is ended; one typed to its last key has ended itself.
    if (typed < ready.keys) py.stdin.write('end\n');
    if (enough && !met && !lost) met = enough.met();
    const wrote = JSON.parse(await out.next());
    if (!wrote.ok) throw new Error(`the injector could not type the plan: ${JSON.stringify(wrote)}`);
    return { events: wrote.events || [], device: wrote.device, lost, met, rechecks };
  } catch (e) {
    try { py.kill('SIGTERM'); } catch { /* already gone */ }
    throw new Error(`${e.message}${stderrOf(said)}`);
  }
}

/// The injector's answer to a plan: how many keys and how big a chunk, or why it would not start.
async function started(out, ms) {
  const ready = JSON.parse(await out.next(ms));
  if (!ready.ready) throw new Error(`the injector would not start: ${JSON.stringify(ready)}`);
  return ready;
}

/// What the injector said on stderr, as the tail of an error that names it.
const stderrOf = (said) => (said.stderr.trim() ? `\n${said.stderr.trim()}` : '');

// ---------- one session ----------

// Every line the app has written into its capture so far. Read rather than watched: the app
// appends and flushes, so the file is whole as of its last drain, and a partial last line is one
// the next read will have whole.
function capture(file) {
  let text = '';
  try { text = fs.readFileSync(file, 'utf8'); } catch { return []; }
  const keys = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try { keys.push(JSON.parse(line)); } catch { /* a line still being written */ }
  }
  return keys;
}

/// Waits until the capture has stopped growing — see `QUIET_MS` — or `SETTLE_MS` has passed.
async function captureDrained(file) {
  let last = capture(file).length;
  for (let quiet = 0, waited = 0; quiet < QUIET_MS && waited < SETTLE_MS; waited += DRAIN_POLL_MS) {
    await sleep(DRAIN_POLL_MS);
    const now = capture(file).length;
    quiet = now === last ? quiet + DRAIN_POLL_MS : 0;
    last = now;
  }
}

/// Waits up to `ms` for a launch to say it reached `moment` on its stdout (`launchSaid`), and
/// answers with when it did, or `null`.
async function saidBy(ours, moment, ms) {
  for (let waited = 0; ; waited += SAID_POLL_MS) {
    const said = launchSaid(ours.said(), moment);
    if (said || waited >= ms) return said;
    await sleep(SAID_POLL_MS);
  }
}

/// Where the regime says the writer is, in the flag the app takes.
///
/// `end` is the flag's own word. `middle` is the first line break past half the document — the
/// same rule `dev/legacy/tools/latency.mjs` applies, so that the two benches type into the same
/// paragraph — except that the app counts UTF-8 bytes where the browser counted characters, so the
/// search is done on the bytes and the offset is a byte offset.
// Read once. It is a fact about a file that does not change under a run, and a run asks for it
// twice per launch — once for the command line, once for the result's `definition`.
let middleOfDoc = null;

function caretFor(root, where) {
  if (where !== 'middle') return 'end';
  if (middleOfDoc === null) {
    const doc = fs.readFileSync(path.join(root, DOC));
    const half = Math.floor(doc.length / 2);
    const at = doc.indexOf(0x0a, half);
    middleOfDoc = String(at < 0 ? half : at);
  }
  return middleOfDoc;
}

// The command line one regime's launches use, warm-up and measured alike: the judged states'
// window size, the 10k-word document, and the caret, Focus, Live, Preview and Syntax highlight
// the regime asks for. Every launch pins Syntax highlight, including when it is off.
function launchArgv(root, out, regime) {
  const argv = [
    '--deterministic', '--measure', out,
    '--text', path.join(root, DOC), '--caret', caretFor(root, regime.where),
    '--focus', regime.focus || 'off',
    '--w', String(WINDOW.w), '--h', String(WINDOW.h),
  ];
  if (regime.live) argv.push('--live');
  argv.push('--syntax', regime.syntax || 'off');
  argv.push('--style', regime.style || 'off');
  argv.push('--spell', regime.spell || 'off');
  // Stats has no `off` to pin every launch with the way the three above do, and
  // needs none: `--deterministic` is already on this command line, so a regime
  // that names no `--stats` is launched on the table's defaults — the three
  // cells the other eighteen regimes have always typed against.
  if (regime.stats) argv.push('--stats', regime.stats);
  if (regime.preview) argv.push('--preview', regime.preview);
  return argv;
}

/// Puts the regime's passage on the clipboard, for the regimes that paste.
///
/// `Control+v` pastes whatever the compositor is holding, so without this a paste regime measures
/// whatever the owner last copied — or, on a machine where nothing has been copied, a keystroke
/// that changes no text and so produces no frame, which the accounting would then refuse. The text
/// goes in over stdin rather than as an argument because `wl-copy` adds a newline to an argument
/// and the passage's own trailing blank line is part of what is being pasted.
///
/// Both of its output streams are discarded rather than captured, and that is not tidiness. The
/// parent `wl-copy` exits at once, but the server it forks to hold the selection lives until
/// something replaces it — and it inherits whatever it was given, so a captured pipe is one this
/// call then waits on for an end-of-file that only arrives when the clipboard changes hands. That
/// is a bench that types eleven regimes and then hangs for ever on the twelfth.
function loadClipboard(text) {
  execFileSync('wl-copy', ['--type', 'text/plain'], { input: text, stdio: ['pipe', 'ignore', 'ignore'] });
}

/// Opens and closes one window on the stage, and answers with the cold start it saw.
///
/// The first GTK client on a freshly created output pays for the whole graphics stack coming up on
/// it — measured here at about 740 ms against about 115 ms for every launch after it, on the same
/// binary and the same document. That is the compositor's first client, not the app's cold start,
/// and charging it to the app would be measuring the harness. So the stage is warmed exactly as the
/// app is warmed before the measured keys, and this launch's own number is kept in the result
/// beside the measured ones rather than thrown away, so that the difference is on the record.
async function warmStage(root, stage, regime) {
  const first = await stage.launch(path.join(root, BINARY), launchArgv(root, path.join(stage.tmp, 'capture-warmup.jsonl'), regime));
  await sleep(SETTLE_MS);
  const cold = /^cold start: ([\d.]+) ms$/m.exec(first.said());
  // Gone rather than waited out: what the first measured launch must not share the machine with
  // is a client still shutting down, and that is over when its process is.
  const exited = new Promise((resolve) => first.child.once('exit', resolve));
  stage.kill(first.child);
  await Promise.race([exited, sleep(SETTLE_MS)]);
  return cold ? Number(cold[1]) : null;
}

/// Launches ours, warms it, types the regime and answers with the two sides of the join.
async function runSession(root, stage, { regime, keys, index, injector }) {
  const out = path.join(stage.tmp, `capture-${index}.jsonl`);

  stage.rules({ w: WINDOW.w, h: WINDOW.h, initialFocus: true });

  // `$QUILL_T0_NS` is stamped inside `launch`, immediately before the exec — see there for why it
  // cannot be stamped from here.
  const ours = await stage.launch(path.join(root, BINARY), launchArgv(root, out, regime));

  try {
    if (!(await stage.focused(ours.address))) {
      throw new Error('keyboard focus would not stay on ours, so nothing was typed');
    }
    say(`gate bench: focus is on ours (${ours.address}), ${APP_ID}`);

    // The warm-up, thrown away: the keyboard that types the measured keys types it first, and at
    // the boundary the capture is measured so the join never sees it.
    //
    // It is fitted to the app's two launch lines (`launchSaid`, #495). It starts at `settled`: a key
    // typed while the Annotators' first pass is out throws the pass away, and the whole Document is
    // done again at the warm-up's end, on key 0 (17.41 ms). And it ends only after `quiet`: until
    // then GTK has the scrollbar the launch's scroll showed still to hide, in a frame of its own,
    // and a measured key inside the same refresh waits a whole one (16.25 ms in #489). The 25 keys
    // at the regime's pace stay, because they are first touch; when the launch is not quiet after
    // them, pairs of a letter and its Backspace top them up until it is. So the last warm-up key is
    // always the regime's pace and one capture settle before key 0, as it was before the wait.
    const settledAt = await saidBy(ours, 'settled', LAUNCH_WAIT_MS);
    if (!settledAt) {
      throw new Error(`the app did not say its launch was settled within ${LAUNCH_WAIT_MS / 1000} s, `
        + 'so nothing was typed');
    }
    const first = uinputPlan(script('letters', WARMUP_KEYS, hash32('warmup')), regime.pace);
    const pairs = Math.ceil(TOP_UP_MS / first.plan.pace_ms / 2);
    const warm = { ...first.plan, keys: [...first.plan.keys, ...uinputPlan(topUp(pairs), regime.pace).plan.keys] };
    const planned = uinputPlan(script(regime.mix, keys, hash32(regime.name)), regime.pace,
      { pauseEvery: regime.pauseEvery, pauseMs: regime.pauseMs });
    if (planned.first_unexpressible) {
      throw new Error(`${regime.name} reaches ${planned.first_unexpressible.press} at step `
        + `${planned.first_unexpressible.at}, which the injector cannot type`);
    }
    const held = () => stage.holds(ours.address);
    const warming = await typeKeys(injector, warm, held,
      { enough: { from: first.keys, step: 2, met: () => launchSaid(ours.said(), 'quiet') !== null } });
    if (!warming.lost && !warming.met) {
      throw new Error(`the app's launch was not quiet after ${first.keys} warm-up keys and `
        + `${TOP_UP_MS / 1000} s of top-ups, so its first measured keys would have carried it`);
    }
    const topped = warming.events.length - first.keys;
    say(`gate bench: typing ${planned.keys} keys at ${planned.plan.pace_ms} ms, after ${first.keys} of warm-up`
      + `${topped > 0 ? ` and ${topped} of top-up` : ''}`);

    // A run that lost focus inside the warm-up sends nothing more.
    let wrote = { events: [], lost: true, rechecks: warming.rechecks };
    let before = null;
    if (!warming.lost) {
      await captureDrained(out);
      before = capture(out).length;
      wrote = await typeKeys(injector, planned.plan, held);
      wrote.rechecks += warming.rechecks;
    }

    await captureDrained(out);
    const sent = wrote.events.map((e, i) => ({ ...e, i }));
    const seen = before === null ? [] : capture(out).slice(before);
    const cold = /^cold start: ([\d.]+) ms$/m.exec(ours.said());
    // The refusal the wait exists for, asked of what happened rather than of what was meant to: a
    // first measured key written before the app said its launch was quiet, or inside the refresh
    // after, can have waited on the launch's last frame, and measured the launch.
    const quietAt = launchSaid(ours.said(), 'quiet');
    const lead = sent.length && quietAt ? sent[0].t_ns / 1e6 - quietAt.at_us / 1e3 : null;
    if (lead !== null && lead < REFRESH_MS) {
      throw new Error(`the first measured key went ${lead.toFixed(1)} ms after the app said its launch `
        + `was quiet, inside the ${REFRESH_MS.toFixed(1)} ms refresh its last frame takes, so it measured the launch`);
    }

    return {
      sent,
      seen,
      planned: planned.keys,
      cold: cold ? Number(cold[1]) : null,
      launch: {
        settled_ms: settledAt.from_exec_ms,
        quiet_ms: quietAt?.from_exec_ms ?? null,
        quiet_before_first_key_ms: lead === null ? null : Number(lead.toFixed(1)),
        top_up_keys: topped,
      },
      device: warming.device,
      focus_rechecks: wrote.rechecks,
      stopped_because_focus_was_lost: wrote.lost,
      // Read after the keys, before the kill: the leave the kill itself causes is never in `said`.
      the_pointer_left_the_window: pointerLeft(ours.said()),
    };
  } finally {
    stage.kill(ours.child);
  }
}

// ---------- the run ----------

/// One regime, measured: its sessions, pooled, and written to its own result file.
///
/// The stage is the caller's, because opening one costs a compositor output and the nineteen regimes
/// of a release run share it. The launch is not shared: every regime gets its own, so the caret it
/// types at, the Focus it runs under and the cold start it reports are its own and not the last
/// regime's.
async function benchOne(root, stage, { regime, keys, sessions, warmup, panel, injector }) {
  // Loaded before the first launch rather than once for the whole run, so that a regime which
  // pastes is pasting its own passage even when the owner used the clipboard between regimes.
  if (regime.mix === 'paste') loadClipboard(PASTE_TEXT);

  const runs = [];
  for (let index = 0; index < sessions; index += 1) {
    say(`gate bench: ${regime.name}, session ${index + 1} of ${sessions}`);
    runs.push(await runSession(root, stage, { regime, keys, index, injector }));
  }

  // Pooled across sessions: the budget is a property of the app, not of one launch of it.
  const sent = runs.flatMap((r) => r.sent);
  const seen = runs.flatMap((r) => r.seen);
  // What the regime asked for, so that a run which stopped early is short against the plan rather
  // than complete against itself.
  const decided = measure(sent, seen, runs.reduce((a, r) => a + r.planned, 0));
  const colds = runs.map((r) => r.cold).filter((c) => c != null);
  const cold = colds.length ? Math.max(...colds) : null;
  const scored = scoredRegime(regime.name);
  const said = verdict(decided.uinput_write_to_presented_ms, cold, scored);

  const result = {
    // First in the file, and in the file at all rather than only in the line the run printed,
    // because a result outlives the terminal it was printed in: whoever opens this in six months
    // meets "not a Gate condition" before they meet a mean. The same for a regime that is recorded
    // and not scored: its `verdict.pass` is null, and the reason is the first thing in the file.
    ...(panel ? { informational: PANEL_IS_INFORMATIONAL, panel } : {}),
    ...(scored ? {} : { not_scored: NOT_SCORED }),
    regime: regime.name,
    definition: {
      mix: regime.mix, where: regime.where, pace_ms: regime.pace, focus: regime.focus,
      caret: caretFor(root, regime.where),
      pause_every_keys: regime.pauseEvery ?? null, pause_ms: regime.pauseEvery ? (regime.pauseMs || PAUSE_MS) : null,
      keys_per_session: keys, sessions, warmup_keys: WARMUP_KEYS, warmup_top_up_ms: TOP_UP_MS,
    },
    budget: BUDGET,
    verdict: said,
    ...decided,
    input_injection: {
      how: 'real keys written to /dev/uinput, tools/uinput-keys.py',
      device: runs[0]?.device ?? null,
      aligned_by: 'gdk_keycode === evdev code + 8, greedily',
      focus_rechecks: runs.reduce((a, r) => a + r.focus_rechecks, 0),
      stopped_because_focus_was_lost: runs.some((r) => r.stopped_because_focus_was_lost),
      the_pointer_left_the_window: runs.some((r) => r.the_pointer_left_the_window),
    },
    cold_start_ms: {
      each: runs.map((r) => r.cold),
      worst: cold,
      // Kept, and not judged: the graphics stack coming up on a brand-new output. See `warmStage`.
      stage_first_client: warmup,
    },
    // Per session: when the app said its launch's own work was over, how long before the first
    // measured key that was, and how many top-up keys the warm-up needed to get there (#495).
    launch: runs.map((r) => r.launch),
    sessions_mean_ms: runs.map((r) => measure(r.sent, r.seen).uinput_write_to_presented_ms?.mean ?? null),
    // What the injector did between keys, against what `definition` said it would.
    write_gaps_ms: writeGaps(runs.map((r) => r.sent)),
    fingerprint: fingerprint(root, stage),
  };

  const file = path.join(RESULTS, `bench-${panel ? 'panel-' : ''}${regime.name}-${stamp()}.json`);
  fs.mkdirSync(path.join(root, RESULTS), { recursive: true });
  fs.writeFileSync(path.join(root, file), `${JSON.stringify(result, null, 2)}\n`);
  say(`gate bench: wrote ${file}`);
  // A refused run keeps what it was refused on: the keys as written and the stamps as the app wrote
  // them, which otherwise go with the stage's tmp directory. The result file carries counts and
  // samples, and a count of two keys without a presentation time says nothing about which two.
  if (!decided.accounting.every_keystroke_accounted_for) {
    const raw = file.replace(/\.json$/, '.capture.json');
    fs.writeFileSync(path.join(root, raw), `${JSON.stringify({ sent, seen })}\n`);
    say(`gate bench: wrote ${raw} (the keys and stamps the accounting refused)`);
  }

  return {
    regime: regime.name,
    file,
    result,
    said,
    // The one shape both `summary` and `regimeLine` read, built once here so that the lines a run
    // of one prints and the lines a run of nineteen prints cannot be assembled two different ways.
    reported: {
      accounting: decided.accounting, verdict: said, stage_first_client: warmup, panel,
    },
    lost: runs.some((r) => r.stopped_because_focus_was_lost),
    pointerLeft: runs.some((r) => r.the_pointer_left_the_window),
  };
}

// The stamp a result file is named by: one second's resolution, which is finer than a regime runs.
const stamp = () => new Date().toISOString().replace(/[-:]/g, '').replace(/\..*/, '');

async function bench(root, { ran, chosen, keys, sessions, wantsPanel, idle }) {
  // Asked before the build, because a refusal the owner waits two minutes of `cargo build` for
  // is a refusal that arrives after they have gone. The one check not made here is the idle one:
  // that is a question about the last few seconds, and `openPanelStage` asks it immediately
  // before it takes the screen.
  if (wantsPanel) {
    const blocked = panelBlocked({ workspace: PANEL_WORKSPACE });
    if (blocked) {
      say(`gate bench: ${blocked}`);
      return refuse(blocked);
    }
  }
  if (!compositorAvailable()) {
    say('gate bench: no Hyprland to open a window on; real keys need the compositor '
      + '(docs/research/native-harness.md)');
    return refuse('there is no compositor to type on');
  }

  say(`gate bench: building ${BINARY}`);
  try {
    execFileSync('cargo', ['build', '--release'], {
      cwd: root, stdio: ['ignore', 'ignore', 'pipe'], maxBuffer: BUILD_OUTPUT_MAX,
    });
  } catch (e) {
    const said = String(e.stderr || '').trim();
    if (said) say(said);
    return refuse('the binary would not build');
  }

  const stage = wantsPanel
    ? await openPanelStage({ root, workspace: PANEL_WORKSPACE, idle })
    : await openStage({ root });

  // What the panel run is of, taken from the stage rather than from the flag, so that a line saying
  // "on DP-3 at 3840x2160 scale 1.5" is saying what the compositor actually gave it.
  const panel = wantsPanel ? {
    output: stage.monitor.name,
    mode: `${stage.monitor.width}x${stage.monitor.height}`,
    scale: stage.monitor.scale,
    workspace: stage.workspace,
    idle_window_s: idle,
  } : null;
  if (panel) {
    say(`gate bench: measuring on the PHYSICAL panel, ${panel.output} workspace ${panel.workspace}`
      + '; the workspace goes back afterwards, and the numbers are informational');
  }

  const done = [];
  let warmup = null;
  let injector = null;
  // The run's one keyboard is made while the stage warms, so its settle is paid inside a wait the
  // run has anyway rather than before a regime's first key.
  const opening = openInjector(root);
  opening.catch(() => {});
  try {
    stage.rules({ w: WINDOW.w, h: WINDOW.h, initialFocus: true });
    warmup = await warmStage(root, stage, chosen[0]);
    injector = await opening;
    say(`gate bench: the stage's first client cold-started in ${warmup} ms, and is not measured`);
    for (const regime of chosen) {
      const one = await benchOne(root, stage, { regime, keys, sessions, warmup, panel, injector });
      done.push(one);
      // Two things stop the rest of a run. Focus going somewhere else: the keys after it would be
      // typed into whatever took the focus, and no later regime's number would be of this app. And,
      // in a run of several, a regime whose keys did not add up: the run is refused whatever the
      // rest measure and its files are not committed, so the six minutes the rest would take buy
      // nothing. Every other kind of short run is said and carried on from. The pointer leaving
      // is the owner at the mouse, which is the same run-ending news as focus going: the keys
      // after it were measured under the chrome's fade (#327), and the next regime's would be too.
      if (one.lost || one.pointerLeft) break;
      if (ran !== null && !one.reported.accounting.every_keystroke_accounted_for) break;
    }
  } catch (e) {
    say(String(e.stack || e.message));
    return refuse(e.message.split('\n')[0]);
  } finally {
    await (injector ?? await opening.catch(() => null))?.close();
    stage.close();
  }

  // What was asked for decides how it is said: a bare `gate bench [regime]` is one regime and the
  // three lines #64 settled, and `--all` or `--regimes` is a run, however many regimes are in it.
  return ran === null
    ? oneSaid(done[0])
    : manySaid(root, { ran, chosen, done, warmup, panel });
}

/// What one regime's run prints, and the code it exits with.
///
/// A run whose keys cannot all be accounted for has not measured slowly, it has measured something
/// else. Refused rather than reported, so that a mean over whichever keys survived never reaches a
/// ticket as evidence.
function oneSaid(one) {
  if (!one.reported.accounting.every_keystroke_accounted_for) {
    say(JSON.stringify(one.reported.accounting, null, 2));
    return refuse(one.lost
      ? `focus was taken away mid-run; ${one.file} records where it stopped`
      : `not every keystroke is accounted for; the numbers are in ${one.file}`);
  }
  // Accounted for, and still not a measurement of the keystroke path: from the leave on, the
  // chrome's fade had the frame clock pacing keys to the refresh grid (#327).
  if (one.pointerLeft) {
    return refuse(`the pointer left the window mid-run, so the keys after it were paced by the `
      + `chrome's fade rather than measured; ${one.file} records the run`);
  }
  for (const line of summary(one.regime, one.reported)) console.log(line);
  // 1 is "this missed the budget", and neither the panel nor an unscored regime is held to the
  // budget. A run of either that measured every key it sent has done the whole of what it was
  // asked, so it exits 0 whatever the numbers are; the only way it fails is the accounting above,
  // which is not about speed.
  return one.reported.panel || !scoredRow(one.said) || one.said.pass ? 0 : 1;
}

/// What a run of several regimes prints, and the code it exits with.
///
/// One line per regime and then one line for the run, and a summary file beside the per-regime
/// results holding what those lines say — because the release check and `tools/gate judge latency`
/// both ask the same question of a whole run, and neither should have to reopen nineteen files and
/// decide for itself which nineteen they were.
function manySaid(root, { ran, chosen, done, warmup, panel }) {
  const rows = done.map((one) => ({
    regime: one.regime,
    file: one.file,
    ...one.said,
    every_keystroke_accounted_for: one.reported.accounting.every_keystroke_accounted_for,
    the_pointer_left_the_window: one.pointerLeft,
  }));
  // A regime that never ran is not a regime that passed. The run is short, and the summary says so
  // by name rather than by a count the reader has to do themselves.
  const missing = chosen.slice(done.length).map((r) => r.name);
  // Every regime accounts for its keys, scored or not: an unscored regime is recorded, and a record
  // over whichever keys survived is not one.
  const unaccounted = rows.filter((r) => !r.every_keystroke_accounted_for).map((r) => r.regime);
  // And a regime the pointer left mid-way measured the chrome's fade, not the keys (#327).
  const paced = rows.filter((r) => r.the_pointer_left_the_window).map((r) => r.regime);
  const cleared = (r) => !scoredRow(r) || r.pass;

  // Written before anything is printed, and holding the printed lines themselves, because the file
  // is the run's own record of what it said: `tools/gate judge latency` reads it rather than
  // re-deriving a verdict from nineteen result files and hoping it phrases it the same way.
  const whole = !missing.length && !unaccounted.length && !paced.length;
  const lines = done.map((one) => regimeLine(one.regime, one.reported));
  if (whole) lines.push(allSummary(ran, rows, panel));

  // Named apart from a Gate run's summary rather than only flagged inside it. `tools/gate judge
  // latency` takes the newest `summary-*.json` under this directory when it is not given one, and a
  // panel run that could become the newest of those is a panel run that could be judged by
  // accident. The flag inside is the second guard, for the summary that is named to judge by hand.
  const file = path.join(RESULTS, `${panel ? 'panel-' : ''}summary-${stamp()}.json`);
  fs.writeFileSync(path.join(root, file), `${JSON.stringify({
    ran,
    at: new Date().toISOString(),
    budget: BUDGET,
    oracle: ORACLE,
    headline: HEADLINE,
    regimes: rows,
    regimes_not_run: missing,
    regimes_unaccounted_for: unaccounted,
    regimes_the_pointer_left: paced,
    // Recorded and not held to the budget, `NOT_SCORED`; their rows carry `scored: false` and a
    // null `pass`, and `pass` below is over the others.
    regimes_not_scored: rows.filter((r) => !scoredRow(r)).map((r) => r.regime),
    stage_first_client_ms: warmup,
    // Null rather than a boolean for a panel run: `pass` here means "cleared the Gate's budget",
    // and these numbers were not taken where that budget applies. There is no answer to give.
    pass: panel ? null : whole && rows.every(cleared),
    ...(panel ? { informational: PANEL_IS_INFORMATIONAL, panel } : {}),
    lines,
    build: {
      git: done[0]?.result.fingerprint.app.git_head ?? null,
      binary: done[0]?.result.fingerprint.app.sha256 ?? null,
    },
    fingerprint: done[0]?.result.fingerprint ?? null,
  }, null, 2)}\n`);
  say(`gate bench: wrote ${file}`);

  for (const line of lines) console.log(line);
  if (whole && panel) return 0;
  if (!whole) {
    const never = missing.length ? ` and ${missing.join(', ')} never ran` : '';
    return refuse(unaccounted.length
      ? `not every keystroke is accounted for in ${unaccounted.join(', ')}, so the run stopped there`
        + `${never}; the numbers are in ${file}`
      : paced.length
        ? `the pointer left the window during ${paced.join(', ')}, so the keys after it were paced by `
          + `the chrome's fade rather than measured and the run stopped there${never}; ${file} records it`
        : `${missing.join(', ')} never ran; ${file} records how far the run got`);
  }
  return rows.every(cleared) ? 0 : 1;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);

  let name = null;
  let all = false;
  let listed = null;
  let keys = DEFAULT_KEYS;
  let sessions = 1;
  let wantsPanel = false;
  let idle = PANEL_IDLE_S;
  let idleGiven = false;
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--keys' || a === '--sessions') {
      const n = Number(argv[i += 1]);
      if (!Number.isInteger(n) || n < 1) {
        process.stderr.write(`gate bench: ${a} takes a whole number of at least 1\n`);
        usage();
        return 3;
      }
      if (a === '--keys') keys = n; else sessions = n;
    } else if (a === '--idle-window') {
      // Seconds, and not necessarily whole ones: `tools/idle-check.py` takes a float, and the
      // fraction is worth keeping for a run being tried repeatedly on a machine somebody is about
      // to leave. Zero is allowed and means "ask once and do not wait" — still a check, because
      // `idle-check.py` refuses a machine it cannot read the input devices of whatever the window.
      idleGiven = true;
      const n = Number(argv[i += 1]);
      if (!Number.isFinite(n) || n < 0) {
        process.stderr.write('gate bench: --idle-window takes seconds, 0 or more\n');
        usage();
        return 3;
      }
      idle = n;
    } else if (a === '--panel') { wantsPanel = true; } else if (a === '--all') {
      all = true;
    } else if (a === '--regimes') {
      listed = argv[i += 1] ?? '';
    } else if (a === '-h' || a === '--help') { usage(process.stdout); return 0; } else if (a.startsWith('-')) {
      process.stderr.write(`gate bench: ${a}: not a flag this command has\n`);
      usage();
      return 3;
    } else if (name === null) { name = a; } else {
      process.stderr.write('gate bench: one regime at a time\n');
      usage();
      return 3;
    }
  }

  // Refused rather than ignored. The flag's whole job is to move a threshold that only --panel
  // consults, so a run given it without --panel is a run whose author believes something about it
  // that is not true.
  if (idleGiven && !wantsPanel) {
    process.stderr.write('gate bench: --idle-window is how long --panel waits for the machine to go '
      + 'quiet, and this run is not a --panel run\n');
    usage();
    return 3;
  }

  const known = regimes();
  let choice;
  try {
    choice = chosenList({
      command: 'gate bench', flag: '--regimes', listOf: 'regime names', what: 'which regimes to run',
      all, listed, named: name, universe: () => known.map((r) => r.name),
    });
  } catch (e) {
    if (!(e instanceof Refusal)) throw e;
    process.stderr.write(`${e.message}\n`);
    usage();
    return 3;
  }

  // `ran` is null for a bare `gate bench [regime]` and the flag itself otherwise, because it is
  // both what decides the shape of the output and what the summary file records as the run.
  const ran = choice.flag === '--regimes' ? `--regimes ${choice.names.join(',')}` : choice.flag;
  const chosen = [];
  for (const want of choice.names ?? [name ?? HEADLINE]) {
    const found = known.find((r) => r.name === want);
    if (!found) {
      process.stderr.write(`gate bench: ${want}: not one of the nineteen regimes `
        + `(${known.map((r) => r.name).join(', ')})\n`);
      usage();
      return 3;
    }
    chosen.push(found);
  }

  openLog(root);
  try {
    return await bench(root, { ran, chosen, keys, sessions, wantsPanel, idle });
  } catch (e) {
    say(String(e.stack || e.message));
    return refuse(e.message.split('\n')[0]);
  }
}

// The exit code is set rather than taken: process.exit() can cut the last line short on its way down
// a pipe, and that line is the whole point of the command.
if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
