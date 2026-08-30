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
import { decodePng, judgeBurst, judgeMove, readBar } from './keys-assert.mjs';

const BINARY = 'target/release/quill';
const STATES = 'shots/oracle/states.json';
const BUILD_OUTPUT_MAX = 8 * 1024 * 1024;

// The typist's plan. The pace is brisker than a writer's because nothing here is timed — what
// matters is only that every key lands — and the chunk is the bench's, so focus is asked the same
// question every 25 keys. `settle_ms` is the wait *before* the first key, for the compositor to
// pick the new virtual keyboard up.
const PACE_MS = 40;
const HOLD_MS = 12;
const CHUNK = 25;
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

// The assertions a script may name, by the phrase the failing line prints.
const AFTER_BURST = { 'bar-after-ink': judgeBurst };
const BETWEEN_BURSTS = { 'bar-moved-right': judgeMove };

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

// ---------- the script a Piece is typed by ----------

/// The bursts and assertions listed for a Piece, with the judged defaults filled in around the
/// state it opens.
export function resolveScript(states, piece) {
  const scripts = states.keys || {};
  // `_about` and its kind are prose for whoever opens the file, not Pieces.
  const named = Object.keys(scripts).filter((k) => !k.startsWith('_'));
  if (!named.includes(piece)) {
    throw new Error(`no keys script for ${piece}`
      + `${named.length ? ` (${STATES} names ${named.join(', ')})` : ''}`);
  }
  const script = scripts[piece];
  const bursts = script.bursts || [];
  if (!bursts.length) throw new Error(`no keys script for ${piece}`);
  // `chars` is how many characters stand on the line once the burst has been typed, and the
  // advance the assertions measure against is derived from it. It is written down rather than
  // counted so that the day a script presses Enter or Backspace, the number that stops being the
  // running total says so here instead of quietly shifting the tolerance.
  let running = 0;
  for (const burst of bursts) {
    running += [...burst.text].length;
    if (burst.chars !== running) {
      throw new Error(`${piece}: burst ${burst.name} says chars ${burst.chars}, but ${running} `
        + 'characters have been typed by the end of it');
    }
  }
  for (const burst of bursts) {
    for (const name of burst.assert || []) {
      if (!AFTER_BURST[name]) {
        throw new Error(`${piece}: no assertion called ${name} `
          + `(this command knows ${Object.keys(AFTER_BURST).join(', ')})`);
      }
    }
  }
  for (const name of script.between || []) {
    if (!BETWEEN_BURSTS[name]) {
      throw new Error(`${piece}: no between-bursts assertion called ${name} `
        + `(this command knows ${Object.keys(BETWEEN_BURSTS).join(', ')})`);
    }
  }
  return { ...script, bursts, flags: { ...states.defaults, ...(script.state || {}) } };
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
  const colours = script.colours;
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
      const plan = {
        pace_ms: PACE_MS, hold_ms: HOLD_MS, settle_ms: SETTLE_MS, chunk: CHUNK, text: burst.text,
      };
      say(`gate keys: burst ${burst.name}: ${burst.text.length} keys`);
      const wrote = await typeKeys(root, plan, () => stage.holds(ours.address));
      if (wrote.lost) {
        return refuse(piece, `focus was taken away during ${burst.name}, so typing stopped there`);
      }
      if (wrote.events.length !== burst.text.length) {
        return refuse(piece, `${burst.name} asked for ${burst.text.length} keys and the typist `
          + `wrote ${wrote.events.length}`);
      }

      const shot = await captureBar(stage, ours.toplevel.id, colours);
      if (!shot) return refuse(piece, `no capture of ${burst.name} ever settled`);
      if (shotsDir) {
        fs.mkdirSync(shotsDir, { recursive: true });
        fs.writeFileSync(path.join(shotsDir, `${burst.name}.png`), shot.buf);
        say(`gate keys: wrote ${path.join(shotsDir, `${burst.name}.png`)}`);
      }
      seen.push({ burst, shot });
    }

    // Every assertion is evaluated, and the first that fails is the one the line names — a run that
    // stopped at the first failure would hide the rest from the log the owner reads next.
    const failures = [];
    for (const { burst, shot } of seen) {
      for (const name of burst.assert || []) {
        const verdict = AFTER_BURST[name](shot.png, { chars: burst.chars, colours, read: shot.read });
        say(`gate keys: ${burst.name} ${name}: ${verdict.pass ? 'ok' : 'FAILED'} — ${verdict.said}`);
        if (!verdict.pass) failures.push(`${name} after ${burst.name}: ${verdict.said}`);
      }
    }
    for (const name of script.between || []) {
      for (let i = 1; i < seen.length; i += 1) {
        const verdict = BETWEEN_BURSTS[name](seen[i - 1].shot.read, seen[i].shot.read);
        const where = `${seen[i - 1].burst.name} to ${seen[i].burst.name}`;
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
