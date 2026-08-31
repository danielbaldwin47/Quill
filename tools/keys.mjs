#!/usr/bin/env node
// Types real keys into ours and then looks at the glass — `tools/gate keys <piece>`.
//
//   node tools/keys.mjs <piece> [--shots <dir>]
//
// WHY THE GATE NEEDED A CONDITION THAT TYPES
//
// Every other condition is a still or a clock. `judge` shoots ours at rest and hands the pair to a
// critic; `bench` types but reads the frame clock, never the pixels; `check` runs with no display.
// So nothing pressed a key and then looked, and #108 shipped a caret whose bar was placed from
// GTK's `mark-set` — a signal not emitted for the insert mark carried along by an insertion. Three
// judged states, six fresh critics, all correct, and the bar was still at x=0 after 38 characters.
// This is the condition that would have caught it: the keys are real, and so is the reading.
//
// WHY IT IS NOT PART OF `bench`
//
// A shot taken mid-run skews the regime `bench` is measuring, and `bench`'s whole output is that
// number. The two commands share the typist and the stage and nothing else.
//
// WHY LIVE AND NOT `--deterministic`
//
// `--deterministic` freezes the blink on and takes the glide out, which is exactly right for a
// still and exactly wrong here: the caret machine is the thing under test, so it has to run. What
// that costs is the blink, and `captureBar` below is the whole of the answer to it.

import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { typeKeys } from './bench.mjs';
import { compositorAvailable, openStage, quillArgv } from './harness.mjs';
import {
  AFTER_BURST, BETWEEN_BURSTS, INK_DARK, PAPER_DARK, SETTLES, STATES, decodePng, readBar,
  resolveScript,
} from './keys-assert.mjs';

const BINARY = 'target/release/quill';
const BUILD_OUTPUT_MAX = 8 * 1024 * 1024;

// The typist's plan. The pace is brisker than a writer's because nothing here is timed — what
// matters is only that every key lands. `settle_ms` is the wait *before* the first key, for the
// compositor to pick the new virtual keyboard up.
//
// The chunk is 8 where the bench's is 25, and the difference is the whole point of having one: a
// burst here is sixteen or twenty-two keys, so at 25 the question "does ours still hold focus?"
// would be asked once before the burst and never again inside it, which is the same defence the
// `focused` proof already gives and no defence at all against focus going somewhere else halfway
// through. At 8 every burst is asked two or three times over.
const PACE_MS = 40;
const HOLD_MS = 12;
const CHUNK = 8;
const SETTLE_MS = 1200;

// How many times a burst's shot is taken before the run gives up looking for the bar.
//
// The caret holds its blink for 480 ms after an edit, so the first capture — taken as soon as the
// typist has let go — finds the bar full on and two grim frames apart agree. If that one is missed
// the blink is running: 470 ms on, 85 ms down, 445 ms off, 55 ms back, so a capture lands on a
// static page seven times in eight and on a *lit* page about half the time. Eight tries spans
// several turns of that cycle, which makes a run that never sees a bar a fact about the build
// rather than about the moment it was caught in.
const BLINK_TRIES = 8;

// ---------- the trail ----------
//
// `judge` and `bench`'s division, kept: stdout is the one line the owner reads, and everything the
// run said on the way is in a log named only when the run refuses.

const trail = [];
let logFile = null;
function say(line) {
  trail.push(line);
  if (!logFile) return;
  try { fs.appendFileSync(logFile, `${line}\n`); } catch { logFile = null; }
}

function openLog(root, piece) {
  const file = path.join(root, 'target/gate', `keys-${piece}.log`);
  try {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, `gate keys ${piece} — ${new Date().toISOString()}\n`);
    logFile = file;
  } catch { logFile = null; }
}

// Nothing was typed, or nothing could be read, and why. One code for all of them, because none of
// them is a verdict about the build.
function refuse(piece, why) {
  if (trail.length) process.stderr.write(`${trail.join('\n')}\n`);
  console.log(`gate keys ${piece}: refused (${why})`);
  return 3;
}

function usage(to = process.stderr) {
  to.write(`usage: tools/gate keys <piece> [--shots <dir>]

  piece           the Piece whose keys script to type; ${STATES} names them
  --shots <dir>   also write each burst's capture to <dir>/<name>.png

Ends in \`gate keys <piece>: pass\` or \`gate keys <piece>: fail (...)\`, and in
\`gate keys <piece>: refused (...)\` when nothing was typed or nothing could be read.
`);
}

// ---------- the shot a burst is judged on ----------

// A capture with the caret lit in it, or the last one taken if the bar never appeared.
//
// `steady` wants two consecutive grim frames byte-identical, which a page mid-fade never gives; it
// throws rather than guessing, and here that throw is just another try.
async function captureBar(stage, toplevel, colours) {
  let last = null;
  for (let i = 0; i < BLINK_TRIES; i += 1) {
    let buf;
    try {
      buf = await stage.steady(toplevel);
    } catch (e) {
      say(`gate keys: capture ${i + 1} never settled (${e.message})`);
      continue;
    }
    const png = decodePng(buf);
    const read = readBar(png, colours);
    last = { buf, png, read };
    if (read.bar) return last;
    say(`gate keys: capture ${i + 1} caught the blink's dark half; looking again`);
  }
  return last;
}

// A capture of a page with no caret on it: the still a burst that ends in a selection leaves.
//
// The retry above is not merely unnecessary here, it is the wrong question — a selection puts the
// caret out entirely (ADR 0014), so a run looking for a lit bar would spend its eight tries on a
// page that is exactly right and then refuse it. There is nothing to wait out either: the caret's
// idle hold is the caret's, and this page has none, so `steady`'s two agreeing captures are the
// whole rule and no millisecond of the blink is written down a second time here.
// Every settle rule is called `(stage, toplevel, colours)`, and this one has no use for the third:
// the colours say which way round the page's ink and paper are, which only the bar's reading needs.
async function captureStill(stage, toplevel) {
  try {
    const buf = await stage.steady(toplevel);
    return { buf, png: decodePng(buf), read: null };
  } catch (e) {
    say(`gate keys: the page never settled (${e.message})`);
    return null;
  }
}

// The settle rules by the name a burst says in `settle`. The names themselves are
// `keys-assert.mjs`'s `SETTLES`, so that a script naming one is refused before a window opens; this
// half holds the functions, and the check below is what keeps the two halves one list — a rule
// added there and not here says so on the next run rather than at the burst that reaches for it.
const CAPTURE = { caret: captureBar, still: captureStill };
for (const name of SETTLES) {
  if (!CAPTURE[name]) throw new Error(`gate keys: no capture for the settle rule ${name}`);
}

// ---------- the run ----------

async function run(root, piece, { shotsDir }) {
  // Asked first, and before the build, because whether a Piece has a script at all is a fact about
  // the command line: a Piece that carries none should hear so at once rather than after two
  // minutes of `cargo build` and a window it never needed.
  let script;
  try {
    script = resolveScript(JSON.parse(fs.readFileSync(path.join(root, STATES), 'utf8')), piece);
  } catch (e) {
    return refuse(piece, e.message);
  }

  if (!compositorAvailable()) {
    say('gate keys: no Hyprland to open a window on; real keys need the compositor '
      + '(docs/research/native-harness.md)');
    return refuse(piece, 'there is no compositor to type on');
  }

  say(`gate keys: building ${BINARY}`);
  try {
    execFileSync('cargo', ['build', '--release'], {
      cwd: root, stdio: ['ignore', 'ignore', 'pipe'], maxBuffer: BUILD_OUTPUT_MAX,
    });
  } catch (e) {
    const said = String(e.stderr || '').trim();
    if (said) say(said);
    return refuse(piece, 'the binary would not build');
  }

  const { flags } = script;
  // Which pair the ink test is looking for, taken from the theme the script opens rather than
  // asked for separately: a script that says `--theme dark` has already said which way round its
  // page is.
  const colours = flags.theme === 'dark' ? { ink: INK_DARK, paper: PAPER_DARK } : undefined;
  const stage = await openStage({ root });
  let ours = null;
  try {
    stage.rules({ w: flags.w, h: flags.h, initialFocus: true });
    ours = await stage.launch(path.join(root, BINARY), quillArgv(root, flags, { live: true }));

    // The one thing the typist cannot check for itself: `uinput-keys.py` writes to the kernel and
    // has no idea which window the kernel's keys reach. Proving focus here, and asking `holds`
    // between every chunk below, is the whole of the defence.
    if (!(await stage.focused(ours.address))) {
      return refuse(piece, 'keyboard focus would not stay on ours, so no keys typed');
    }

    const seen = [];
    for (const burst of script.bursts) {
      // A burst's `text` is what a keyboard spells straight, and its `keys` is what `text` cannot
      // spell: `Control+a` is one thing the writer did, and the typist's chord form is the only
      // form that can say it. The typist takes one or the other — `keys` wins when both are given
      // — so a burst carrying both is expanded here into the one array, one entry per code point
      // of `text` as `uinput-keys.py` expands `text` itself, and the chords after it.
      const keys = burst.keys
        ? [...[...(burst.text || '')].map((press) => ({ press })), ...burst.keys]
        : null;
      const plan = {
        pace_ms: PACE_MS, hold_ms: HOLD_MS, settle_ms: SETTLE_MS, chunk: CHUNK,
        ...(keys ? { keys } : { text: burst.text }),
      };
      // Code points for a burst that only types, as the typist counts them; entries for one that
      // spells its keys, because a chord is one key however many go down for it — one write(2),
      // one stamp, one recorded event (`uinput-keys.py` § WHY A CHORD IS ONE KEY).
      const wanted = keys ? keys.length : [...burst.text].length;
      say(`gate keys: burst ${burst.name}: ${wanted} keys`);
      const wrote = await typeKeys(root, plan, () => stage.holds(ours.address));
      if (wrote.lost) {
        return refuse(piece, `focus was taken away during ${burst.name}, so typing stopped there`);
      }
      if (wrote.events.length !== wanted) {
        return refuse(piece, `${burst.name} asked for ${wanted} keys and the typist `
          + `wrote ${wrote.events.length}`);
      }

      const settle = burst.settle || 'caret';
      const shot = await CAPTURE[settle](stage, ours.toplevel.id, colours);
      if (!shot) return refuse(piece, `no capture of ${burst.name} ever settled`);
      // A shot with no bar in it is nothing read, not a bar in the wrong place, so it refuses
      // rather than condemning the build: after `BLINK_TRIES` turns of a 1,055 ms cycle the honest
      // thing to say is that the caret was never caught lit, and 3 is the code for that. Asked
      // only of a burst that left a caret on the page: a `still` burst is a page with none.
      if (settle === 'caret' && !shot.read.bar) {
        return refuse(piece, `the caret was never caught lit after ${burst.name}, so there was `
          + 'no bar to measure');
      }
      if (shotsDir) {
        fs.mkdirSync(shotsDir, { recursive: true });
        fs.writeFileSync(path.join(shotsDir, `${burst.name}.png`), shot.buf);
        say(`gate keys: wrote ${path.join(shotsDir, `${burst.name}.png`)}`);
      }
      seen.push({ burst, shot, settle });
    }

    // Every assertion is evaluated, and the first that fails is the one the line names — a run that
    // stopped at the first failure would hide the rest from the log the owner reads next.
    const failures = [];
    for (const { burst, shot } of seen) {
      for (const name of burst.assert || []) {
        const verdict = AFTER_BURST[name](shot.png, {
          chars: burst.chars, rows: burst.rows, colours, read: shot.read,
        });
        say(`gate keys: ${burst.name} ${name}: ${verdict.pass ? 'ok' : 'FAILED'} — ${verdict.said}`);
        if (!verdict.pass) failures.push(`${name} after ${burst.name}: ${verdict.said}`);
      }
    }
    // Every between-bursts assertion there is compares two bars, and a burst that leaves no caret
    // on the page has none to compare — so a pair with a `still` burst at either end is passed
    // over rather than failed, and the log says which pair and why.
    for (const name of script.between || []) {
      for (let i = 1; i < seen.length; i += 1) {
        const where = `${seen[i - 1].burst.name} to ${seen[i].burst.name}`;
        if (seen[i - 1].settle !== 'caret' || seen[i].settle !== 'caret') {
          say(`gate keys: ${where} ${name}: not asked (a burst with no caret on the page)`);
          continue;
        }
        const verdict = BETWEEN_BURSTS[name](seen[i - 1].shot.read, seen[i].shot.read);
        say(`gate keys: ${where} ${name}: ${verdict.pass ? 'ok' : 'FAILED'} — ${verdict.said}`);
        if (!verdict.pass) failures.push(`${name} from ${where}: ${verdict.said}`);
      }
    }

    if (failures.length) {
      console.log(`gate keys ${piece}: fail (${failures[0]})`);
      return 1;
    }
    console.log(`gate keys ${piece}: pass`);
    return 0;
  } finally {
    if (ours) stage.kill(ours.child);
    stage.close();
  }
}

async function main(argv) {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  let piece = null;
  let shotsDir = null;
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '-h' || a === '--help') { usage(process.stdout); return 0; }
    if (a === '--shots') {
      shotsDir = argv[i + 1];
      i += 1;
      if (!shotsDir) { process.stderr.write('gate keys: --shots wants a directory\n'); usage(); return 3; }
    } else if (a.startsWith('-')) {
      process.stderr.write(`gate keys: ${a}: not a flag this command has\n`);
      usage();
      return 3;
    } else if (piece === null) {
      piece = a;
    } else {
      process.stderr.write(`gate keys: ${piece} and ${a}: one Piece at a time\n`);
      usage();
      return 3;
    }
  }
  if (!piece) { usage(); return 3; }

  openLog(root, piece);
  try {
    return await run(root, piece, { shotsDir });
  } catch (e) {
    say(String(e.stack || e.message));
    return refuse(piece, e.message.split('\n')[0]);
  }
}

// The exit code is set rather than taken: process.exit() can cut the last line short on its way
// down a pipe, and that line is the whole point of the command.
if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  process.exitCode = await main(process.argv.slice(2));
}
