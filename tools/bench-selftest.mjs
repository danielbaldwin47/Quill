#!/usr/bin/env node
// Does the bench's join still pair the right keys? — `tools/gate bench`'s own test.
//
//   node tools/bench-selftest.mjs
//
// The join is the one part of the bench that decides what the numbers mean, and it is the one part
// that can be checked without a compositor, a window or a keyboard: it is a function of two arrays.
// So it is checked here, on captures written out by hand, and `tools/gate check` runs this.
//
// The captures are small and literal on purpose. A generated fixture would be checked against the
// same idea of the world that produced it; these are what the two files actually look like, with
// the two things that actually go wrong in them — a stray key of the owner's, and a key no frame
// ever carried.

import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  BUDGET, KEYCODE_OFFSET, against, align, allSummary, clears, latencyMs, latencyVerdict, measure,
  pointerLeft, regimeLine, summary, verdict, writeGaps,
} from './bench-join.mjs';
import { PANEL_WORKSPACE, panelRefusal, physicalMonitors } from './harness.mjs';
import { DEFAULT_KEYS, PAUSE_MS, REFRESH_MS, hash32, regimes, scoredRegime, script, uinputPlan } from './regimes.mjs';

// The checkout, for the two cases that ask the app and the injector rather than a table copied here.
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

let cases = 0;
let failures = 0;
function ok(name, body) {
  cases += 1;
  try {
    body();
    console.log(`bench selftest: ${name} ok`);
  } catch (e) {
    failures += 1;
    console.log(`bench selftest: ${name} FAILED`);
    console.log(String(e.message).split('\n').map((l) => `    ${l}`).join('\n'));
  }
}

// ---------- the two files, by hand ----------

// What `tools/uinput-keys.py` says it wrote: evdev codes, and `CLOCK_MONOTONIC` nanoseconds taken
// immediately before each `write(2)`. 'q', 'u', 'i', 'l', 'l' on a US layout.
const CODES = { q: 16, u: 22, i: 23, l: 38 };
const written = (codes, first = 100_000_000_000, step = 90_000_000) =>
  codes.map((code, i) => ({ i, code, shift: false, t_ns: first + i * step }));

// What the app wrote under `--measure`: one line per key, keycode = evdev + 8, presentation time in
// `CLOCK_MONOTONIC` microseconds. Two milliseconds after each write, which is the shape the
// research measured.
const captured = (codes, { first = 100_002_000, step = 90_000, frame = 400 } = {}) =>
  codes.map((code, i) => ({
    keycode: code + KEYCODE_OFFSET,
    evdev_ms: Math.round((100_000_000_000 + i * 90_000_000) / 1e6),
    handler_us: first + i * step - 500,
    frame: frame + i,
    present_us: first + i * step,
    refresh_us: 16_666,
  }));

const QUILL = [CODES.q, CODES.u, CODES.i, CODES.l, CODES.l];

// ---------- the join ----------

ok('every key pairs with its own, and the latency is write(2) to presented', () => {
  const joined = align(written(QUILL), captured(QUILL));
  assert.equal(joined.pairs.length, 5);
  assert.equal(joined.missing.length, 0);
  assert.equal(joined.stray.length, 0);
  for (const pair of joined.pairs) {
    assert.equal(pair.seen.keycode, pair.sent.code + KEYCODE_OFFSET, 'paired the wrong key');
    assert.equal(latencyMs(pair), 2, 'two milliseconds is two milliseconds');
  }
});

ok('a stray key of the owner\'s is skipped rather than shifting every pair', () => {
  // The owner pressed Super in the middle of the run. It is in the app's capture and in nothing the
  // bench wrote. Index alignment would pair every key after it with its neighbour's frame and
  // report four confident lies; this has to report five correct pairs and one stray.
  const seen = captured(QUILL);
  const SUPER = 125;
  seen.splice(2, 0, {
    keycode: SUPER + KEYCODE_OFFSET, evdev_ms: 100_002, handler_us: 100_002_100,
    frame: 999, present_us: 100_002_600, refresh_us: 16_666,
  });

  const joined = align(written(QUILL), seen);
  assert.equal(joined.pairs.length, 5, 'every key the bench wrote should still have found its own');
  assert.equal(joined.stray.length, 1, 'the owner\'s key should be the only thing left over');
  assert.equal(joined.stray[0].frame, 999);
  for (const pair of joined.pairs) {
    assert.equal(pair.seen.keycode, pair.sent.code + KEYCODE_OFFSET);
    assert.equal(latencyMs(pair), 2, 'a stray key must not shift a single pair');
  }
});

ok('a key no frame carried is paired, and counted as having no presentation time', () => {
  // The last key of a run, in the frame that was still waiting when the app was told to stop. The
  // app writes it with nulls rather than leaving it out, so the bench can say it is one short
  // instead of quietly reporting a mean over four keys and calling it five.
  const seen = captured(QUILL);
  seen[4] = { ...seen[4], frame: null, present_us: null, refresh_us: null };

  const joined = align(written(QUILL), seen);
  assert.equal(joined.pairs.length, 5, 'it was seen, so it pairs');
  assert.equal(latencyMs(joined.pairs[4]), null, 'but it has no latency');

  const decided = measure(written(QUILL), seen);
  assert.equal(decided.accounting.keys_sent, 5);
  assert.equal(decided.accounting.keys_seen_by_the_app, 5);
  assert.equal(decided.accounting.keys_with_a_presentation_time, 4);
  assert.equal(decided.accounting.every_keystroke_accounted_for, false,
    'four presented out of five sent is not every keystroke accounted for');
});

ok('a key written late, because no frame ever carried it, is scanned where it was seen', () => {
  // A key that changed no pixel — #220's ArrowLeft — has no frame to complete, so the app writes
  // it only when it goes stale, after the keys that followed it. In file order the walk would pair
  // it with the next key of its code and shift every pair after that; in the order the app saw
  // them, it pairs with itself, carries no latency, and the four around it keep theirs.
  const seen = captured(QUILL);
  const [late] = seen.splice(1, 1);
  seen.push({ ...late, frame: null, present_us: null, refresh_us: null });

  const joined = align(written(QUILL), seen);
  assert.equal(joined.pairs.length, 5, 'every key was seen, so every key pairs');
  assert.equal(joined.stray.length, 0);
  assert.equal(joined.pairs[1].sent.code, CODES.u);
  assert.equal(latencyMs(joined.pairs[1]), null, 'the late key has no presentation to measure');
  for (const pair of joined.pairs.filter((_, i) => i !== 1)) {
    assert.equal(latencyMs(pair), 2, 'the keys around it keep their own frames');
  }
});

ok('a key the app never saw does not shift the keys after it', () => {
  // The opposite failure: the bench wrote a key that never arrived. The cursor must not advance,
  // or every later key pairs with the wrong frame.
  const seen = captured(QUILL).filter((_, i) => i !== 1);
  const joined = align(written(QUILL), seen);
  assert.equal(joined.pairs.length, 4);
  assert.equal(joined.missing.length, 1);
  assert.equal(joined.missing[0].code, CODES.u);
  for (const pair of joined.pairs) {
    assert.equal(pair.seen.keycode, pair.sent.code + KEYCODE_OFFSET);
  }
});

ok('keys that shared a frame are counted, not treated as a fault', () => {
  // A burst faster than the refresh interval genuinely shares a frame, and both keys genuinely
  // turned into light then. The run is still whole.
  const seen = captured(QUILL).map((k) => ({ ...k, frame: 400, present_us: 100_002_000 }));
  const decided = measure(written(QUILL), seen);
  assert.equal(decided.accounting.keys_sharing_a_frame, 4);
  assert.equal(decided.accounting.every_keystroke_accounted_for, true);
});

// ---------- the accounting and the verdict ----------

ok('a whole run is every keystroke accounted for', () => {
  const decided = measure(written(QUILL), captured(QUILL));
  assert.deepEqual(
    [decided.accounting.keys_sent, decided.accounting.keys_aligned,
      decided.accounting.keys_with_a_presentation_time],
    [5, 5, 5],
  );
  assert.equal(decided.accounting.every_keystroke_accounted_for, true);
  assert.equal(decided.uinput_write_to_presented_ms.mean, 2);
  assert.equal(decided.uinput_write_to_presented_ms.max, 2);
});

ok('the pointer leaving the window is read from the line the app prints, and only that line', () => {
  // The line `quill/src/harness.rs` prints per leave, among the cold start it also prints; and the
  // words alone, or a leave the app never reported, are not one (#327).
  assert.equal(pointerLeft('cold start: 167.061 ms\npointer left the window at 6516887400 us\n'), true);
  assert.equal(pointerLeft('cold start: 167.061 ms\n'), false);
  assert.equal(pointerLeft('the pointer left the window at some point'), false);
  assert.equal(pointerLeft(''), false);
  assert.equal(pointerLeft(undefined), false);
});

ok('a run that lost focus part-way is short against the plan, not whole against itself', () => {
  // The failure this is here for: focus is taken away after the first chunk, so the injector stops
  // at the boundary and reports only the keys it actually wrote. Those keys were all seen and all
  // presented, so every count agrees with every other one — and the run is still missing most of a
  // regime. Only the plan can tell the difference, which is why the plan is in the accounting.
  const typed = QUILL.slice(0, 2);
  const decided = measure(written(typed), captured(typed), 5);
  assert.equal(decided.accounting.keys_sent, 2);
  assert.equal(decided.accounting.keys_aligned, 2);
  assert.equal(decided.accounting.keys_with_a_presentation_time, 2);
  assert.equal(decided.accounting.keys_planned, 5);
  assert.equal(decided.accounting.every_keystroke_accounted_for, false,
    'two keys of a five-key plan is not every keystroke accounted for');

  const lines = summary('prose_end_of_draft', {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
  });
  assert.match(lines[0], /2 keys sent of 5 planned/, 'the line the owner reads says how short it is');
  assert.match(lines[0], /not every keystroke is accounted for/);
});

ok('a run that sent nothing is not a whole run', () => {
  const decided = measure([], []);
  assert.equal(decided.accounting.every_keystroke_accounted_for, false,
    'zero of zero keys is not evidence of anything');
  assert.equal(decided.uinput_write_to_presented_ms, null);
});

ok('the budget is the Gate\'s three numbers, and a miss on any one of them fails', () => {
  const clears = { mean: BUDGET.mean_ms - 1, max: BUDGET.worst_ms - 1, p50: 1, p99: 2 };
  assert.equal(verdict(clears, BUDGET.cold_ms - 1).pass, true);
  assert.equal(verdict({ ...clears, mean: BUDGET.mean_ms + 0.01 }, 1).pass, false, 'mean');
  assert.equal(verdict({ ...clears, max: BUDGET.worst_ms + 0.01 }, 1).pass, false, 'worst');
  assert.equal(verdict(clears, BUDGET.cold_ms + 1).pass, false, 'cold start');
});

ok('a regime that is recorded and not scored keeps its numbers and has no pass to give', () => {
  const over = { mean: 24.83, max: 33.89, p50: 25.08, p99: 31.2 };
  const said = verdict(over, 149, false);
  assert.equal(said.scored, false);
  assert.equal(said.pass, null, 'neither pass nor fail: the budget is not what it is held to');
  assert.equal(said.clears_mean, false, 'the numbers are still real, and still written');
  assert.equal(said.mean_ms, 24.83);
  assert.equal(verdict(over, 149).scored, true, 'scored unless told otherwise');
  assert.equal(verdict(over, 149).pass, false, 'the same numbers on a scored regime fail');
});

ok('a run with no cold start is a miss rather than a pass', () => {
  // `--measure` with no `$QUILL_T0_NS` prints no cold start. The Gate asks for three numbers, and a
  // run that produced two has not answered it.
  const decided = verdict({ mean: 1, max: 2, p50: 1, p99: 2 }, null);
  assert.equal(decided.cold_ms, null);
  assert.equal(decided.pass, false);
});

ok('a miss is printed as measured rather than hidden', () => {
  const seen = captured(QUILL).map((k, i) => (i === 3 ? { ...k, present_us: k.present_us + 40_000 } : k));
  const decided = measure(written(QUILL), seen);
  const last = summary('prose_end_of_draft', {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
  }).at(-1);
  // The last line is the verdict, and on its own it answers every number the Gate asks bench for.
  assert.match(last, /^gate bench prose_end_of_draft: fail /);
  assert.match(last, /mean [\d.]+ ms/);
  assert.match(last, /worst 42 ms/, 'the number that missed is in the line');
  assert.match(last, /p50 [\d.]+ ms/);
  assert.match(last, /p99 [\d.]+ ms/);
  assert.match(last, /cold 120 ms/);
  assert.match(last, /oracle /, 'the oracle\'s numbers are printed beside');
});

ok('the cold start the budget judges is the one the line names, warm-up said out loud', () => {
  // Warming the stage is the bench choosing which launch the cold-start budget judges. That choice
  // has to be in front of the owner, not three levels into a result file.
  const decided = measure(written(QUILL), captured(QUILL));
  const shape = {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
  };
  assert.equal(summary('prose_end_of_draft', shape).length, 2, 'no warm-up, no line about one');

  const lines = summary('prose_end_of_draft', { ...shape, stage_first_client: 836.826 });
  assert.equal(lines.length, 3);
  assert.match(lines[1], /first client cold-started in 836.83 ms and is not measured/);
  assert.match(lines.at(-1), /cold 120 ms/, 'and the judged number is still ours');
});

// ---------- what the owner reads ----------

ok('every line starts with `gate bench` and is one line', () => {
  const decided = measure(written(QUILL), captured(QUILL));
  const lines = summary('prose_end_of_draft', {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
    stage_first_client: 836.826,
  });
  for (const line of lines) {
    assert.match(line, /^gate bench /, line);
    assert.ok(!line.includes('\n'), line);
    assert.ok(!line.includes('null'), `a line the owner reads should not say null: ${line}`);
    assert.ok(!line.includes('undefined'), line);
  }
  assert.match(lines.at(-1), /^gate bench prose_end_of_draft: pass /);
});

// ---------- a run of several ----------

// One regime's row as `bench --all` records it in its summary file.
const row = (regime, mean, worst, cold, pass = true, extra = {}) => ({
  regime,
  file: `shots/latency/bench-${regime}-20260828T000000.json`,
  mean_ms: mean,
  worst_ms: worst,
  p50_ms: mean,
  p99_ms: worst,
  cold_ms: cold,
  pass,
  every_keystroke_accounted_for: true,
  ...extra,
});

// The saturation regime as a `bench --all` records it since it stopped being scored: over every
// bar, `pass` null, `scored` false. The oracle's own figure was over the bars too.
const saturation = () => row('saturation_stress', 24.83, 33.89, 149, null, { scored: false });

const runOf = (rows, extra = {}) => ({
  ran: '--all',
  at: '2026-08-28T00:00:00.000Z',
  headline: 'prose_end_of_draft',
  regimes: rows,
  regimes_not_run: [],
  regimes_unaccounted_for: [],
  lines: rows.map((r) => `gate bench ${r.regime}: pass`).concat('gate bench --all: pass'),
  ...extra,
});

ok("a regime's line is one line of numbers, and the run's line says how many cleared", () => {
  const decided = measure(written(QUILL), captured(QUILL));
  const line = regimeLine('fence_flip', {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
  });
  assert.match(line, /^gate bench fence_flip: pass — mean /);
  assert.ok(!line.includes('\n'), line);
  assert.ok(!line.includes('budget'), 'the budget is said once, by the run, not sixteen times');

  const rows = [row('prose_end_of_draft', 2, 8, 120), row('revision', 2.4, 9, 130)];
  assert.match(allSummary('--all', rows), /^gate bench --all: pass — 2 of 2 regimes clear the budget/);
});

ok('a run whose regime missed the budget fails and is named', () => {
  const rows = [row('prose_end_of_draft', 2, 8, 120), row('revision', 6.1, 19, 130, false)];
  const line = allSummary('--all', rows);
  assert.match(line, /^gate bench --all: fail — 1 of 2 regimes clear the budget, not revision/);
  assert.ok(!line.includes('\n'), line);
});

ok('--regimes says which subset it was, so two runs are not confused', () => {
  const line = allSummary('--regimes revision,paste_blocks', [row('revision', 2, 8, 120), row('paste_blocks', 2, 8, 120)]);
  assert.match(line, /^gate bench --regimes revision,paste_blocks: pass — 2 of 2 /);
});

ok('an unscored regime says informational on every line it has, and its numbers stay on it', () => {
  const decided = measure(written(QUILL), captured(QUILL));
  const said = verdict({ mean: 24.83, max: 33.89, p50: 25.08, p99: 31.2 }, 149, false);
  const one = summary('saturation_stress', { accounting: decided.accounting, verdict: said });
  const last = one[one.length - 1];
  assert.match(last, /^gate bench saturation_stress: informational — mean 24\.83 ms, worst 33\.89 ms, p50 25\.08 ms, p99 31\.2 ms, cold 149 ms \(not scored: keys share frames/);
  assert.ok(!last.includes('budget'), 'a regime not held to the budget does not quote it');

  const inRun = regimeLine('saturation_stress', { accounting: decided.accounting, verdict: said });
  assert.match(inRun, /^gate bench saturation_stress: informational — mean 24\.83 ms.* \(not scored: /);
  assert.ok(!inRun.includes('\n'), inRun);

  // Scored, the same numbers are a fail, and say so.
  const scored = verdict({ mean: 24.83, max: 33.89, p50: 25.08, p99: 31.2 }, 149);
  assert.match(regimeLine('fast_typist', { accounting: decided.accounting, verdict: scored }), /^gate bench fast_typist: fail — /);
});

ok('an unscored regime over every bar does not fail the run, and a scored one still does', () => {
  const rows = [row('prose_end_of_draft', 2, 8, 120), row('revision', 2.4, 9, 130), saturation()];
  const line = allSummary('--all', rows);
  assert.match(line, /^gate bench --all: pass — 2 of 2 scored regimes clear the budget; saturation_stress informational \(mean <= /);
  assert.ok(!line.includes('\n'), line);

  const failing = [row('prose_end_of_draft', 2, 8, 120), row('revision', 6.1, 19, 130, false), saturation()];
  assert.match(allSummary('--all', failing), /^gate bench --all: fail — 1 of 2 scored regimes clear the budget, not revision; saturation_stress informational/);

  // A summary written before rows carried `scored` reads the same way: the regime's definition
  // says which one is not scored, so the name is enough.
  const legacy = [row('prose_end_of_draft', 2, 8, 120), row('saturation_stress', 24.43, 33.44, 133, false)];
  assert.match(allSummary('--all', legacy), /^gate bench --all: pass — 1 of 1 scored regimes clear the budget; saturation_stress informational/);
});

// ---------- the latency Piece's verdict ----------

ok('the headline regime is held to the oracle as well as the budget, and the rest to the budget', () => {
  assert.equal(against(row('prose_end_of_draft', 2, 8, 120), true).length, 5);
  assert.equal(against(row('revision', 2, 8, 120), false).length, 3);
  // The budget is the stricter of the two on both numbers ours is judged on — 5 ms against the
  // oracle's 12.17, and 250 ms against its 370 — so a run that clears the budget has beaten the
  // oracle already. Both are checked anyway, because the rule is the rule and the budget could move.
  const bars = against(row('prose_end_of_draft', 2, 8, 120), true);
  assert.equal(bars[0].whose, 'the budget', 'the tightest bar on a passing run is the budget’s');
});

ok('ours when every regime clears its bars', () => {
  const said = latencyVerdict(runOf([row('prose_end_of_draft', 2, 8, 120), row('revision', 2.4, 9, 130)]));
  assert.equal(said.winner, 'ours');
  assert.equal(said.states.length, 2);
  assert.match(said.gap, /is the closest to a bar/);
  assert.match(said.gapTheirs, /Parity oracle is 12\.17 ms uinput → presented/);
  assert.match(said.verdict, /gate bench --all: pass/);
});

ok('theirs when one regime is past a bar, and that regime is the one the gap names', () => {
  const said = latencyVerdict(runOf([row('prose_end_of_draft', 2, 8, 120), row('revision', 6.1, 19, 130, false)]));
  assert.equal(said.winner, 'theirs');
  assert.equal(said.states.find((s) => s.name === 'revision').winner, 'theirs');
  assert.match(said.gap, /^revision is past a bar/);
});

ok('a number that was never measured is not a number that passed', () => {
  const said = latencyVerdict(runOf([row('prose_end_of_draft', 2, 8, null)]));
  assert.equal(said.winner, 'theirs');
  assert.match(said.gap, /no number at all/);
});

ok('a run with no regimes in it wins nothing', () => {
  assert.equal(latencyVerdict(runOf([])).winner, 'theirs');
});

ok('an unscored regime is recorded beside the verdict and decides nothing', () => {
  const said = latencyVerdict(runOf([row('prose_end_of_draft', 2, 8, 120), row('revision', 2.4, 9, 130), saturation()]));
  assert.equal(said.winner, 'ours', 'saturation is over every bar and the Piece is still ours');
  assert.deepEqual(said.states.map((s) => s.name), ['prose_end_of_draft', 'revision'], 'not a state: a state has a winner');
  assert.equal(said.informational.length, 1);
  assert.equal(said.informational[0].name, 'saturation_stress');
  assert.equal(said.informational[0].scored, false);
  assert.equal(said.informational[0].mean_ms, 24.83, 'its numbers travel with it');
  assert.match(said.informational[0].why, /keys share frames/);
  assert.ok(!said.gap.startsWith('saturation_stress'), `the gap names a scored regime: ${said.gap}`);

  // A scored regime past a bar is still theirs, with saturation in the run.
  const lost = latencyVerdict(runOf([row('prose_end_of_draft', 2, 8, 120), row('revision', 6.1, 19, 130, false), saturation()]));
  assert.equal(lost.winner, 'theirs');
  assert.match(lost.gap, /^revision is past a bar/);
  // A run of only unscored regimes has no state to win on.
  assert.equal(latencyVerdict(runOf([saturation()])).winner, 'theirs');
});

// ---------- the plan, and the keyboard that has to type it ----------

ok('syntax is the fifteenth regime and is scored like the headline prose', () => {
  const all = regimes();
  assert.equal(all.length, 16);
  const syntax = all.find((r) => r.name === 'syntax');
  assert.deepEqual(syntax, { ...all[0], name: 'syntax', syntax: 'on' });
  assert.equal(scoredRegime('syntax'), true);
});

ok('style is the sixteenth regime and is scored like the headline prose', () => {
  const all = regimes();
  assert.equal(all.length, 16);
  const style = all.find((r) => r.name === 'style');
  assert.deepEqual(style, { ...all[0], name: 'style', style: 'on' });
  assert.equal(scoredRegime('style'), true);
});

ok('every one of the sixteen is a plan the injector can be handed', () => {
  for (const r of regimes()) {
    const plan = uinputPlan(script(r.mix, DEFAULT_KEYS, hash32(r.name)), r.pace,
      { pauseEvery: r.pauseEvery, pauseMs: r.pauseMs });
    assert.equal(plan.first_unexpressible, null, `${r.name}: ${JSON.stringify(plan.first_unexpressible)}`);
    assert.equal(plan.keys, DEFAULT_KEYS, r.name);
    assert.ok(plan.plan.pace_ms >= 8, `${r.name}: a real keyboard cannot type with no gap at all`);
  }
});

ok('the chord regimes press chords, and bursts_and_pauses pauses every 25 keys', () => {
  const planOf = (name) => {
    const r = regimes().find((x) => x.name === name);
    return uinputPlan(script(r.mix, DEFAULT_KEYS, hash32(r.name)), r.pace,
      { pauseEvery: r.pauseEvery, pauseMs: r.pauseMs }).plan;
  };
  const presses = (name) => new Set(planOf(name).keys.map((k) => k.press));
  assert.ok(presses('revision').has('Shift+ArrowLeft'), 'revision selects back over what it wrote');
  assert.ok(presses('revision').has('Control+z'), 'revision undoes');
  assert.ok(presses('paste_blocks').has('Control+v'), 'paste_blocks pastes');

  const bursts = planOf('bursts_and_pauses');
  const paused = bursts.keys.map((k, i) => (k.pause_ms == null ? null : i)).filter((i) => i != null);
  assert.deepEqual(paused, [24, 49, 74, 99, 124, 149, 174, 199, 224, 249, 274, 299]);
  assert.ok(bursts.keys.every((k) => k.pause_ms == null || k.pause_ms === PAUSE_MS));

  assert.equal(planOf('fast_typist').pace_ms, 45);
  assert.equal(planOf('saturation_stress').pace_ms, 8, 'the injector floor stands in for a pace of 0');
});

// The pause is a number chosen against the app's own idle timers, and the app is where those
// timers live. Asked of the Rust rather than of a table copied over here, for the same reason the
// case below asks the injector: moving `TITLE_MS`, or restoring 1,400 as a rounder pause, should be
// a red case rather than eleven keys of three hundred quietly measuring the chrome's return (#349).
ok('the pause clears every timer the app arms from the last keystroke', () => {
  const num = (file, re, what) => {
    const m = readFileSync(path.join(root, file), 'utf8').match(re);
    assert.ok(m, `${what} is no longer where this case looks for it in ${file}, so the pause can no `
      + 'longer be checked against it');
    return Number(m[1].replace(/_/g, ''));
  };
  const typing = 'quill/src/chrome/typing.rs';
  const window = 'quill/src/window.rs';
  // The harness's `DRAIN_EVERY` and `TAIL` are not here: that tick repeats from the window's
  // creation rather than from a keystroke, so its phase is not the pause's to clear and no pause
  // value could clear it. Round 3 saw neither of them paint.
  const armed = [
    ['Typing::STATS_MS', num(typing, /pub const STATS_MS: i64 = ([\d_]+);/, 'STATS_MS')],
    ['Typing::TITLE_MS', num(typing, /pub const TITLE_MS: i64 = ([\d_]+);/, 'TITLE_MS')],
    ['AUTOSAVE', num(window, /const AUTOSAVE: Duration = Duration::from_secs\(([\d_]+)\)/, 'AUTOSAVE') * 1000],
    ['Preview REFRESH', num(window, /const REFRESH: Duration = Duration::from_millis\(([\d_]+)\)/, 'REFRESH')],
  ];
  for (const [name, fires] of armed) {
    // One-sided on purpose. A timer that fires *before* the pause ends takes the refresh slot the
    // key that ends the pause needed; one that fires after has already lost that slot to the key,
    // and paints between two keys 90 ms apart.
    assert.ok(PAUSE_MS < fires || PAUSE_MS - fires > REFRESH_MS,
      `the ${PAUSE_MS} ms pause ends ${(PAUSE_MS - fires).toFixed(1)} ms after ${name} (${fires} ms) `
      + `fires, inside the ${REFRESH_MS.toFixed(1)} ms refresh that timer's frame takes`);
  }
});

ok('the injector can say every press the sixteen ask for', () => {
  const wanted = new Set();
  for (const r of regimes()) {
    for (const k of uinputPlan(script(r.mix, DEFAULT_KEYS, hash32(r.name)), r.pace).plan.keys) wanted.add(k.press);
  }
  // Asked of `tools/uinput-keys.py` itself rather than of a table copied over here: the two are in
  // different languages and the only thing that makes them one keyboard is that this asks the one
  // that does the pressing. A spelling it answers None to is a regime that would stop the bench
  // half way through a run, three minutes and a compositor later.
  const said = execFileSync('python3', [
    // -B, because importing the injector to ask it a question should not leave a __pycache__ in
    // tools/ for `git status` to find afterwards.
    '-B',
    '-c',
    'import importlib.util,json,sys\n'
    + "s=importlib.util.spec_from_file_location('uk',sys.argv[1])\n"
    + 'm=importlib.util.module_from_spec(s); s.loader.exec_module(m)\n'
    + 'print(json.dumps([p for p in json.load(sys.stdin) if m.press_for(p) is None]))',
    path.join(root, 'tools/uinput-keys.py'),
  ], { input: JSON.stringify([...wanted]), encoding: 'utf8' });
  assert.deepEqual(JSON.parse(said), [], 'presses the injector has no key for');
});

// The budget is written `≤` and beating the oracle is written *under*, so a tie goes opposite ways
// on the two bars. Only an exact tie tells them apart, which is exactly why it is worth a case.
ok('a number exactly on the budget clears it, and one exactly on the oracle does not', () => {
  assert.equal(clears({ ratio: 1 }), true, 'exactly on the budget');
  assert.equal(clears({ ratio: 1, strict: true }), false, 'exactly on the oracle');
  assert.equal(clears({ ratio: 0.999, strict: true }), true, 'under the oracle');
  assert.equal(clears({ ratio: Infinity }), false, 'never measured');
});

// #65 asks for a regime's pace and its pauses to be visible in the result rather than only
// declared, so the gaps have to survive as numbers a reader can check the declaration against.
ok('write gaps say the pace, and count the pauses without averaging them away', () => {
  const at = (...ms) => ms.map((t) => ({ t_ns: t * 1e6 }));
  const paced = writeGaps([at(0, 90, 180, 270)]);
  assert.equal(paced.p50, 90, 'the pace is the median gap');
  assert.equal(paced.n, 3, 'one gap fewer than there are keys');
  assert.equal(paced.over_a_second, 0, 'nothing paused');

  const bursty = writeGaps([at(0, 8, 16, 1416, 1424)]);
  assert.equal(bursty.max, 1400, 'the pause is the widest gap');
  assert.equal(bursty.over_a_second, 1, 'and it is counted, not smoothed');

  // Two sessions, not one run: the teardown and relaunch between them is not a pace.
  const two = writeGaps([at(0, 90), at(60_000, 60_090)]);
  assert.equal(two.n, 2, 'gaps are taken within a session only');
  assert.equal(two.max, 90, 'so the launch gap never reaches the numbers');
  assert.equal(writeGaps([[]]), null, 'a session that typed nothing has no gaps');
});

// ---------- the physical panel ----------
//
// #66's mode is the one part of the bench that puts a window in front of a person, and the machine
// it has to be right on is one that cannot be arranged on demand: a panel asleep, an owner sitting
// on the workspace the run wants. So the refusals are a pure function of monitors and a workspace
// id, and those machines are handed to it here.

// Monitors as `hyprctl monitors -j` gives them, cut down to the fields the refusal reads. Each
// carries its own `activeWorkspace`, which is the field that matters: the refusal asks the panel
// what it is showing rather than asking the compositor which workspace has focus.
const on = (m, id) => ({ ...m, activeWorkspace: { id, name: String(id) } });
const DP3 = on({ name: 'DP-3', width: 3840, height: 2160, scale: 1.5, x: 0, y: 0 }, 1);
const HEADLESS = on({ name: 'HEADLESS-67', width: 3200, height: 2000, scale: 2, x: 3840, y: 0 }, 9);
const ASLEEP = on({ name: 'FALLBACK', width: 1920, height: 1080, scale: 1, x: 0, y: 0 }, 1);

ok('a panel is a monitor somebody could be looking at, and neither of the other two is', () => {
  assert.deepEqual(physicalMonitors([DP3, HEADLESS, ASLEEP]), [DP3]);
  assert.deepEqual(physicalMonitors([HEADLESS, ASLEEP]), [], 'nothing here scans out to anything');
  assert.deepEqual(physicalMonitors([]), []);
});

ok('the panel is refused with the panel asleep, on workspace 1, and on the one in use', () => {
  const asleep = panelRefusal({ monitors: [ASLEEP], workspace: PANEL_WORKSPACE });
  assert.match(asleep, /FALLBACK/, 'the owner is told which state the compositor is in');
  assert.match(asleep, /no physical output is connected/);

  const stage = panelRefusal({ monitors: [DP3, HEADLESS], workspace: PANEL_WORKSPACE });
  assert.equal(stage, null, "the Gate's own headless output does not make a panel run impossible");

  assert.match(
    panelRefusal({ monitors: [DP3], workspace: 1 }),
    /workspace 1 is the owner's/,
    'workspace 1 is refused even when the panel is showing something else',
  );
  assert.match(
    panelRefusal({ monitors: [on(DP3, PANEL_WORKSPACE)], workspace: PANEL_WORKSPACE }),
    /workspace 5 is the one in use on DP-3/,
  );
  assert.equal(panelRefusal({ monitors: [DP3], workspace: PANEL_WORKSPACE }), null);

  // The one the global `hyprctl activeworkspace` would get wrong: the panel is showing workspace 5
  // and a leftover headless output holds focus on something else. Asking the compositor at large
  // would clear the run to take a workspace that is on screen, and put it on the output that is not.
  assert.match(
    panelRefusal({
      monitors: [on(DP3, PANEL_WORKSPACE), on(HEADLESS, 9)], workspace: PANEL_WORKSPACE,
    }),
    /workspace 5 is the one in use on DP-3/,
    'the panel is asked what it is showing, not whichever monitor has focus',
  );
});

// The panel's numbers are not the budget's, and the line has to be unmistakable about it — a reader
// who takes one for a Gate result has been misled by this file, not by their own carelessness.
const PANEL = { output: 'DP-3', mode: '3840x2160', scale: 1.5, workspace: 5, idle_window_s: 8 };

ok('a panel run says informational, never pass or fail, and says where it was measured', () => {
  const decided = measure(written(QUILL), captured(QUILL));
  const reported = {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
    stage_first_client: 118.4,
    panel: PANEL,
  };
  const lines = summary('prose_end_of_draft', reported);
  for (const line of lines) {
    assert.match(line, /^gate bench prose_end_of_draft --panel: /, line);
    assert.ok(!line.includes('\n'), line);
    assert.ok(!line.includes('null') && !line.includes('undefined'), line);
  }
  const last = lines.at(-1);
  assert.match(last, /: informational — mean /);
  assert.ok(!/: (pass|fail)\b/.test(last), `a panel line has no verdict to give: ${last}`);
  assert.ok(!last.includes('budget'), 'and no budget to be read against');
  assert.match(last, /on DP-3 at 3840x2160 scale 1\.5/, 'two panel runs compare only if these match');
  assert.match(last, /never a Gate condition/);

  // The same regime, off the headless stage, is still judged the way it always was.
  const gated = summary('prose_end_of_draft', { ...reported, panel: null });
  assert.match(gated.at(-1), /^gate bench prose_end_of_draft: pass /);
  assert.match(gated.at(-1), /budget mean <= 5/);
});

ok('a panel run of several counts what it measured rather than what cleared', () => {
  const decided = measure(written(QUILL), captured(QUILL));
  const line = regimeLine('fence_flip', {
    accounting: decided.accounting,
    verdict: verdict(decided.uinput_write_to_presented_ms, 120),
    panel: PANEL,
  });
  assert.match(line, /^gate bench fence_flip --panel: informational — mean /);

  const rows = [row('prose_end_of_draft', 2, 8, 120), row('revision', 6.1, 19, 130, false)];
  const run = allSummary('--all', rows, PANEL);
  assert.match(run, /^gate bench --all --panel: informational — 2 regimes measured/);
  assert.ok(!/\b(pass|fail)\b/.test(run), `nothing here passed or failed: ${run}`);
  assert.ok(!run.includes('clear the budget'), 'the budget is not what this run was against');
  assert.match(run, /on DP-3 at 3840x2160 scale 1\.5/);

  // A regime over the budget changes nothing about a panel run's own line, which is the whole
  // point: it is a number, not a verdict, and `revision` above is well over.
  assert.match(allSummary('--all', [row('revision', 6.1, 19, 130, false)], PANEL), /informational/);
});

if (failures === 0) {
  console.log('bench selftest: pass');
} else {
  console.log(`bench selftest: fail (${failures} of ${cases})`);
  process.exit(1);
}
