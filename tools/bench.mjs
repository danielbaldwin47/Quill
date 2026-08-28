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
// Three things here are not obvious and all three are deliberate.
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
// against a warm theirs would flatter neither honestly. The warm-up keys are in the app's capture
// too, so the join is given only the lines written after them.

import { execFileSync, spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { BUDGET, measure, summary, verdict } from './bench-join.mjs';
import { gitHead } from './fingerprint.mjs';
import { APP_ID, compositorAvailable, openStage } from './harness.mjs';
import {
  DEFAULT_KEYS, WARMUP_KEYS, hash32, regimes, script, uinputPlan,
} from './regimes.mjs';

// The binary a bench is of. Release rather than debug, for the reason a judged shot is: the Gate
// measures what the owner would install, and a debug build's frame timings are not the shipped
// ones.
const BINARY = 'target/release/quill';

// The document the headline regime is typed into: 10,062 words, the size a draft actually is.
const DOC = 'shots/latency/doc10k.md';

// The window a bench runs in — the judged states' size, so that the layout being timed is the
// layout being judged.
const WINDOW = { w: 1440, h: 900 };

// Where results land, and which regime `tools/gate bench` runs when told nothing else.
const RESULTS = 'shots/latency';
const HEADLINE = 'prose_end_of_draft';

// How long the app is given after the last key before its capture is read: past `harness.rs`'s own
// 250 ms tail, which is what makes the frame carrying the last key complete.
const SETTLE_MS = 1_500;

// How long one chunk of keys has to come back before the injector is called hung. A chunk is 25
// keys at 90 ms, so this is many times what it takes.
const CHUNK_TIMEOUT_MS = 30_000;

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
  to.write(`usage: tools/gate bench [regime] [--keys N] [--sessions N]

  regime       which of the twelve to type; ${HEADLINE} by default
  --keys N     keys measured per session (${DEFAULT_KEYS} by default, the oracle's count)
  --sessions N launch and type this many times, for a run-to-run interval (1 by default)
`);
}

// ---------- what produced the numbers ----------

// Everything about this machine and this build that a second run would have to match for the two to
// be comparable. The same question `legacy/tools/latency.mjs` answers for the oracle's numbers, so
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

/// Types one plan, re-verifying focus between every chunk, and answers with what it wrote.
///
/// The chunk protocol is the whole defence: the injector types `chunk` keys and waits to be told to
/// type the next lot, so focus is a question asked every 25 keys rather than once at the start. A
/// run that loses focus stops at the chunk boundary and says so, rather than typing the rest of a
/// draft into whatever took it.
export async function typeKeys(root, plan, held) {
  const py = spawn('python3', [path.join(root, 'tools/uinput-keys.py')], {
    cwd: root, stdio: ['pipe', 'pipe', 'pipe'],
  });
  let stderr = '';
  py.stderr.on('data', (b) => { stderr += b; });
  const out = reader(py.stdout);
  let lost = false;
  let rechecks = 0;

  try {
    py.stdin.write(`${JSON.stringify(plan)}\n`);
    const ready = JSON.parse(await out.next());
    if (!ready.ready) throw new Error(`the injector would not start: ${JSON.stringify(ready)}`);

    for (let typed = 0; typed < ready.keys; typed += ready.chunk) {
      rechecks += 1;
      if (!held()) { lost = true; break; }
      py.stdin.write('go\n');
      await out.next();
    }
    py.stdin.write('end\n');
  } catch (e) {
    try { py.kill('SIGTERM'); } catch { /* already gone */ }
    throw new Error(`${e.message}${stderr.trim() ? `\n${stderr.trim()}` : ''}`);
  } finally {
    py.stdin.end();
  }

  await new Promise((resolve) => py.on('close', resolve));
  const tail = out.rest().trim();
  if (!tail) throw new Error(`the injector said nothing about what it wrote${stderr.trim() ? `\n${stderr.trim()}` : ''}`);
  const wrote = JSON.parse(tail);
  return { events: wrote.events || [], device: wrote.device, lost, rechecks };
}

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

// The command line every launch of a bench uses, warm-up and measured alike: the judged states'
// window size, the 10k-word document, and the caret at the end of it because that is where the
// headline regime types. Only the capture file differs, which is why it is the one argument.
function launchArgv(root, out) {
  return [
    '--deterministic', '--measure', out,
    '--text', path.join(root, DOC), '--caret', 'end',
    '--w', String(WINDOW.w), '--h', String(WINDOW.h),
  ];
}

/// Opens and closes one window on the stage, and answers with the cold start it saw.
///
/// The first GTK client on a freshly created output pays for the whole graphics stack coming up on
/// it — measured here at about 740 ms against about 115 ms for every launch after it, on the same
/// binary and the same document. That is the compositor's first client, not the app's cold start,
/// and charging it to the app would be measuring the harness. So the stage is warmed exactly as the
/// app is warmed before the measured keys, and this launch's own number is kept in the result
/// beside the measured ones rather than thrown away, so that the difference is on the record.
async function warmStage(root, stage) {
  const first = await stage.launch(path.join(root, BINARY), launchArgv(root, path.join(stage.tmp, 'capture-warmup.jsonl')));
  await sleep(SETTLE_MS);
  const cold = /^cold start: ([\d.]+) ms$/m.exec(first.said());
  stage.kill(first.child);
  await sleep(SETTLE_MS);
  return cold ? Number(cold[1]) : null;
}

/// Launches ours, warms it, types the regime and answers with the two sides of the join.
async function runSession(root, stage, { regime, keys, index }) {
  const out = path.join(stage.tmp, `capture-${index}.jsonl`);

  stage.rules({ w: WINDOW.w, h: WINDOW.h, initialFocus: true });

  // `$QUILL_T0_NS` is stamped inside `launch`, immediately before the exec — see there for why it
  // cannot be stamped from here.
  const ours = await stage.launch(path.join(root, BINARY), launchArgv(root, out));

  try {
    if (!(await stage.focused(ours.address))) {
      throw new Error('keyboard focus would not stay on ours, so nothing was typed');
    }
    say(`gate bench: focus is on ours (${ours.address}), ${APP_ID}`);

    // The warm-up, thrown away: typed, then the capture is measured so the join never sees it.
    const warm = uinputPlan(script('letters', WARMUP_KEYS, hash32('warmup')), regime.pace);
    await typeKeys(root, warm.plan, () => stage.holds(ours.address));
    await sleep(SETTLE_MS);
    const before = capture(out).length;

    const planned = uinputPlan(script(regime.mix, keys, hash32(regime.name)), regime.pace);
    if (planned.first_unexpressible) {
      throw new Error(`${regime.name} reaches ${planned.first_unexpressible.press} at step `
        + `${planned.first_unexpressible.at}, which the injector cannot type yet (#65)`);
    }
    say(`gate bench: typing ${planned.keys} keys at ${planned.plan.pace_ms} ms`);
    const wrote = await typeKeys(root, planned.plan, () => stage.holds(ours.address));

    await sleep(SETTLE_MS);
    const seen = capture(out).slice(before);
    const cold = /^cold start: ([\d.]+) ms$/m.exec(ours.said());

    return {
      sent: wrote.events,
      seen,
      planned: planned.keys,
      cold: cold ? Number(cold[1]) : null,
      device: wrote.device,
      focus_rechecks: wrote.rechecks,
      stopped_because_focus_was_lost: wrote.lost,
    };
  } finally {
    stage.kill(ours.child);
  }
}

// ---------- the run ----------

async function bench(root, { regime, keys, sessions }) {
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

  const stage = await openStage({ root });
  const runs = [];
  let warmup = null;
  try {
    stage.rules({ w: WINDOW.w, h: WINDOW.h, initialFocus: true });
    warmup = await warmStage(root, stage);
    say(`gate bench: the stage's first client cold-started in ${warmup} ms, and is not measured`);
    for (let index = 0; index < sessions; index += 1) {
      say(`gate bench: session ${index + 1} of ${sessions}`);
      runs.push(await runSession(root, stage, { regime, keys, index }));
    }
  } catch (e) {
    say(String(e.stack || e.message));
    return refuse(e.message.split('\n')[0]);
  } finally {
    stage.close();
  }

  // Pooled across sessions: the budget is a property of the app, not of one launch of it.
  const sent = runs.flatMap((r) => r.sent);
  const seen = runs.flatMap((r) => r.seen);
  // What the regime asked for, so that a run which stopped early is short against the plan rather
  // than complete against itself.
  const decided = measure(sent, seen, runs.reduce((a, r) => a + r.planned, 0));
  const colds = runs.map((r) => r.cold).filter((c) => c != null);
  const cold = colds.length ? Math.max(...colds) : null;
  const said = verdict(decided.uinput_write_to_presented_ms, cold);

  const result = {
    regime: regime.name,
    definition: {
      mix: regime.mix, where: regime.where, pace_ms: regime.pace, focus: regime.focus,
      keys_per_session: keys, sessions, warmup_keys: WARMUP_KEYS,
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
    },
    cold_start_ms: {
      each: runs.map((r) => r.cold),
      worst: cold,
      // Kept, and not judged: the graphics stack coming up on a brand-new output. See `warmStage`.
      stage_first_client: warmup,
    },
    sessions_mean_ms: runs.map((r) => measure(r.sent, r.seen).uinput_write_to_presented_ms?.mean ?? null),
    fingerprint: fingerprint(root, stage),
  };

  const stamp = new Date().toISOString().replace(/[-:]/g, '').replace(/\..*/, '');
  const file = path.join(RESULTS, `bench-${regime.name}-${stamp}.json`);
  fs.mkdirSync(path.join(root, RESULTS), { recursive: true });
  fs.writeFileSync(path.join(root, file), `${JSON.stringify(result, null, 2)}\n`);
  say(`gate bench: wrote ${file}`);

  // A run whose keys cannot all be accounted for has not measured slowly, it has measured something
  // else. Refused rather than reported, so a mean over whichever keys survived never reaches a
  // ticket as evidence.
  if (!decided.accounting.every_keystroke_accounted_for) {
    say(JSON.stringify(decided.accounting, null, 2));
    const lost = runs.some((r) => r.stopped_because_focus_was_lost);
    return refuse(lost
      ? `focus was taken away mid-run; ${file} records where it stopped`
      : `not every keystroke is accounted for; the numbers are in ${file}`);
  }

  for (const line of summary(regime.name, {
    accounting: decided.accounting, verdict: said, stage_first_client: warmup,
  })) {
    console.log(line);
  }
  return said.pass ? 0 : 1;
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  process.chdir(root);

  let name = null;
  let keys = DEFAULT_KEYS;
  let sessions = 1;
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

  const regime = regimes().find((r) => r.name === (name ?? HEADLINE));
  if (!regime) {
    process.stderr.write(`gate bench: ${name}: not one of the twelve regimes `
      + `(${regimes().map((r) => r.name).join(', ')})\n`);
    usage();
    return 3;
  }

  openLog(root);
  try {
    return await bench(root, { regime, keys, sessions });
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
