// The join and the statistics behind `tools/gate bench`, and nothing that needs a compositor, a
// window or a keyboard.
//
// The bench measures one thing: from the `write(2)` that put a key on `/dev/uinput` to the moment
// the compositor said the frame carrying it turned into light. Neither end is observable from the
// same place. `tools/uinput-keys.py` stamps `CLOCK_MONOTONIC` immediately before each write and
// says so afterwards; the app, under `--measure`, writes one JSON line per key with the
// presentation time of the frame that carried it, which `GdkFrameTimings` gives in the *same*
// `CLOCK_MONOTONIC` domain, in microseconds (`docs/research/native-harness.md` §4.1). So the
// measurement is a subtraction, and everything hard about it is deciding which key is which.
//
// It is a file of its own, and every function here is a function of its arguments, because this is
// the half of the bench the owner has to be able to check. A number that came out of a run nobody
// can re-derive is not evidence. `tools/bench-selftest.mjs` runs it on hand-written captures with
// the two things that actually happen — a stray key of the owner's, and a key no frame carried —
// and `tools/gate check` runs that.

import { mulberry32 } from './regimes.mjs';

// ---------- the bar ----------

// The Gate's hard budget, `docs/agents/gate.md` § Ticket tier. Missing it fails the ticket; it is
// not what wins the latency Piece.
export const BUDGET = { mean_ms: 5, worst_ms: 16, cold_ms: 250 };

// The Parity oracle's own numbers, from the JavaScript app as it won its gauntlet. Printed beside
// ours because beating *these* is what wins the Piece — the budget is only the floor.
export const ORACLE = { mean_ms: 2.43, worst_ms: 15.61, uinput_to_presented_ms: 12.17, cold_ms: 370 };

// What GDK adds to an evdev code to get a keycode. Verified in the research: 'u' is evdev 22 and
// GDK 30. This is the whole of how a key the bench wrote is recognised in what the app saw.
export const KEYCODE_OFFSET = 8;

// ---------- statistics, as `legacy/tools/latency.mjs` computes them ----------
//
// Deliberately the same shapes as the legacy bench's, down to the rounding and the bootstrap's
// seed, so that a native number and an oracle number can be put side by side without anyone having
// to ask whether they were computed the same way. They were.

const r2 = (x) => (x == null || Number.isNaN(x) ? null : Math.round(x * 100) / 100);

// Nearest-rank, which is what a p99 of 300 samples should be: the 3rd-worst sample, not an
// interpolation between two samples that were never measured.
function pct(sorted, p) {
  if (!sorted.length) return null;
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))];
}

export function stat(arr) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x)).sort((x, y) => x - y);
  if (!a.length) return null;
  const mean = a.reduce((s, x) => s + x, 0) / a.length;
  const sd = Math.sqrt(a.reduce((s, x) => s + (x - mean) ** 2, 0) / a.length);
  return {
    n: a.length, min: r2(a[0]), p50: r2(pct(a, 0.5)), p90: r2(pct(a, 0.9)), p95: r2(pct(a, 0.95)),
    p99: r2(pct(a, 0.99)), max: r2(a[a.length - 1]), mean: r2(mean), sd: r2(sd),
  };
}

/// The milliseconds between one `write(2)` to `/dev/uinput` and the next.
///
/// A regime's pace and its pauses are declared in the result's `definition`, and a declaration is
/// not evidence: what the injector *did* is in the timestamps it recorded. `p50` is what separates
/// the 8, 45 and 90 ms regimes, and `max` with `over_a_second` is where `bursts_and_pauses` shows
/// the 1.4 s it waits every 25 keys.
///
/// Taken within a session and never across two: the interval between the last key of one launch
/// and the first of the next is a teardown and a launch, not a pace.
export function writeGaps(sessions) {
  const gaps = sessions.flatMap(
    (sent) => sent.slice(1).map((key, i) => (key.t_ns - sent[i].t_ns) / 1e6),
  );
  if (!gaps.length) return null;
  return { ...stat(gaps), over_a_second: gaps.filter((g) => g >= 1000).length };
}

// A p99 without an interval is a rumour. Percentile bootstrap, 2000 resamples, fixed seed so that
// the interval is a property of the samples rather than of the run that computed it.
export function ci(arr, p, resamples = 2000) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x));
  if (a.length < 20) return null;
  const rnd = mulberry32(0x51ac1);
  const out = [];
  for (let r = 0; r < resamples; r += 1) {
    const s = new Array(a.length);
    for (let i = 0; i < a.length; i += 1) s[i] = a[(rnd() * a.length) | 0];
    s.sort((x, y) => x - y);
    out.push(pct(s, p));
  }
  out.sort((x, y) => x - y);
  return { lo95: r2(pct(out, 0.025)), hi95: r2(pct(out, 0.975)) };
}

// The same bootstrap for the mean, because the bar this Piece is judged against is a mean.
export function ciMean(arr, resamples = 2000) {
  const a = arr.filter((x) => typeof x === 'number' && !Number.isNaN(x));
  if (a.length < 20) return null;
  const rnd = mulberry32(0x3ea11);
  const out = [];
  for (let r = 0; r < resamples; r += 1) {
    let s = 0;
    for (let i = 0; i < a.length; i += 1) s += a[(rnd() * a.length) | 0];
    out.push(s / a.length);
  }
  out.sort((x, y) => x - y);
  return { lo95: r2(pct(out, 0.025)), hi95: r2(pct(out, 0.975)) };
}

// ---------- the join ----------

/// Pairs the keys the bench wrote with the keys the app saw.
///
/// Greedily, by `keycode === code + KEYCODE_OFFSET`, and never by index. Index alignment is the
/// obvious implementation and it is wrong: one stray key — the owner's `Super`, which turned up
/// mid-run in the research's own session — shifts every pair after it, and the run then reports
/// hundreds of confidently mismatched latencies rather than one missing key.
///
/// Two things can go wrong and they are told apart:
///
/// - The app saw a key the bench did not write. It is skipped: the scan walks past it and the
///   sent key still finds its own.
/// - The bench wrote a key the app never saw. The scan runs out without a match, and — this is the
///   part that matters — the cursor is *not* advanced, so the next sent key starts looking from
///   where this one did and the rest of the run still pairs correctly.
///
/// `sent` is `tools/uinput-keys.py`'s `events`: `{ i, code, shift, t_ns }`, in the order written.
/// `seen` is the app's capture file, one object per line, in the order the app saw them.
export function align(sent, seen) {
  const pairs = [];
  const missing = [];
  let cursor = 0;
  for (const key of sent) {
    const want = key.code + KEYCODE_OFFSET;
    let at = cursor;
    while (at < seen.length && seen[at].keycode !== want) at += 1;
    if (at >= seen.length) {
      // Never seen. Leave the cursor where it was: this key is missing, not the ones after it.
      missing.push(key);
      continue;
    }
    pairs.push({ sent: key, seen: seen[at] });
    cursor = at + 1;
  }
  const paired = new Set(pairs.map((p) => p.seen));
  return { pairs, missing, stray: seen.filter((k) => !paired.has(k)) };
}

/// The nanoseconds from the `write(2)` to the presentation, in milliseconds, or `null` when the
/// compositor never said the frame was presented.
export function latencyMs(pair) {
  if (pair.seen.present_us == null) return null;
  return (pair.seen.present_us * 1000 - pair.sent.t_ns) / 1e6;
}

/// The same, from the first GTK handler rather than from the kernel: what the app itself is
/// responsible for, with the compositor's own delivery taken out. Reported, never judged.
export function handlerMs(pair) {
  if (pair.seen.present_us == null || pair.seen.handler_us == null) return null;
  return (pair.seen.present_us - pair.seen.handler_us) / 1e3;
}

/// Whether every key can be accounted for, in the words `legacy/tools/latency.mjs` uses.
///
/// The invariant the Gate asks for is keys sent = keys seen = keys with a presentation time. A run
/// that misses it has not measured slowly, it has measured something else, so the bench refuses
/// rather than reporting a mean over whichever keys happened to survive.
export function accounting(sent, seen, joined, planned = sent.length) {
  const presented = joined.pairs.filter((p) => p.seen.present_us != null);
  const keys_sent = sent.length;
  return {
    keys_planned: planned,
    keys_sent,
    keys_seen_by_the_app: seen.length,
    keys_aligned: joined.pairs.length,
    keys_with_a_presentation_time: presented.length,
    keys_the_app_never_saw: joined.missing.length,
    keys_the_bench_never_sent: joined.stray.length,
    keys_sharing_a_frame: sharingAFrame(joined.pairs),
    // Planned is in here, and it is the half that is easy to leave out. A run that lost focus after
    // a hundred keys wrote a hundred, saw a hundred and presented a hundred: every count agrees
    // with every other, and the run is still two thirds missing. Without the plan to check them
    // against, the accounting would call that whole and the bench would report a mean over
    // whichever third of a regime happened to be typed.
    every_keystroke_accounted_for:
      planned > 0
      && keys_sent === planned
      && joined.pairs.length === keys_sent
      && presented.length === keys_sent,
  };
}

// How many keys landed on a frame another key had already claimed. Not a fault: a burst faster than
// the refresh interval genuinely shares a frame, and both keys were genuinely presented then. It is
// recorded because a run where most keys share frames is a run measuring the refresh rate.
function sharingAFrame(pairs) {
  const seenFrames = new Set();
  let shared = 0;
  for (const pair of pairs) {
    const frame = pair.seen.frame;
    if (frame == null) continue;
    if (seenFrames.has(frame)) shared += 1;
    else seenFrames.add(frame);
  }
  return shared;
}

/// Everything measured about one session, from the two sides of the join.
export function measure(sent, seen, planned = sent.length) {
  const joined = align(sent, seen);
  const samples = joined.pairs.map(latencyMs).filter((ms) => ms != null);
  const handler = joined.pairs.map(handlerMs).filter((ms) => ms != null);
  return {
    accounting: accounting(sent, seen, joined, planned),
    uinput_write_to_presented_ms: stat(samples),
    mean_ci95: ciMean(samples),
    p99_ci95: ci(samples, 0.99),
    first_handler_to_presented_ms: stat(handler),
    samples_ms: samples.map(r2),
  };
}

/// Whether the run clears the Gate's hard budget.
///
/// `cold` may be `null` — a launch with no `$QUILL_T0_NS` has no cold start to clear — and that is
/// a miss rather than a pass, because the Gate asks for three numbers and a run that produced two
/// has not answered it.
export function verdict(stats, cold) {
  const mean = stats?.mean ?? null;
  const worst = stats?.max ?? null;
  const clears_mean = mean != null && mean <= BUDGET.mean_ms;
  const clears_worst = worst != null && worst <= BUDGET.worst_ms;
  const clears_cold = cold != null && cold <= BUDGET.cold_ms;
  return {
    mean_ms: mean,
    worst_ms: worst,
    p50_ms: stats?.p50 ?? null,
    p99_ms: stats?.p99 ?? null,
    cold_ms: cold == null ? null : r2(cold),
    clears_mean,
    clears_worst,
    clears_cold,
    pass: clears_mean && clears_worst && clears_cold,
  };
}

// ---------- the lines the owner reads ----------

// A number as a line says it, or an em dash where there is no number at all. Never `null` and never
// `NaN`: a line the owner reads should say "not measured" in a way that reads as English.
const say = (x) => (x == null ? '—' : String(x));

/// What a line calls the run it is about: the regime, or the flag that chose the regimes, with
/// `--panel` on it when the numbers came off the physical display.
const labelled = (what, panel) => (panel ? `${what} --panel` : what);

/// The clause every panel line ends in, and the reason there is a `--panel` in the label at all.
///
/// Said in full on every line rather than once at the end of the run, because a panel number read
/// without it is a number somebody will hold against the budget — and it cannot be: the panel is
/// fractional-scale, so the window's buffer is not the one the budget was set on. The output, its
/// mode and its scale are in the line for the same reason they are in the fingerprint: two panel
/// runs are only comparable with each other, and only when those three agree.
const informational = (panel) => ` (on ${panel.output} at ${panel.mode} scale ${panel.scale};`
  + ' the physical panel is informational and never a Gate condition,'
  + " and these are not the headless output's numbers)";

/// The five numbers a bench line says, in the one order they are ever said in.
///
/// One regime's line and a whole run's line are read against each other — a `--all` run is twelve
/// of the second under the first — so the shape they share is written once here. What follows the
/// numbers is what differs: a single run names the bars, a regime in a run of twelve leaves them
/// to `allSummary`, and a panel run has no bars to name because it is not judged against any.
const numbers = (regime, said, panel) => `gate bench ${labelled(regime, panel)}: `
  + `${panel ? 'informational' : (said.pass ? 'pass' : 'fail')}`
  + ` — mean ${say(said.mean_ms)} ms, worst ${say(said.worst_ms)} ms`
  + `, p50 ${say(said.p50_ms)} ms, p99 ${say(said.p99_ms)} ms, cold ${say(said.cold_ms)} ms`;

/// The whole of what `tools/gate bench` prints, as an array of lines.
///
/// Three, in the order they are read: what the run consisted of, what it measured, and whether that
/// clears the budget with the oracle's numbers beside it. The last one is the verdict, because the
/// Gate's rule is that every command ends in the line the owner reads.
export function summary(regime, decided) {
  const count = decided.accounting;
  const said = decided.verdict;
  const panel = decided.panel ?? null;
  const what = labelled(regime, panel);
  const lines = [
    `gate bench ${what}: ${count.keys_sent} keys sent`
    + (count.keys_sent === count.keys_planned ? '' : ` of ${count.keys_planned} planned`)
    + `, ${count.keys_seen_by_the_app} seen, ${count.keys_with_a_presentation_time} presented`
    + (count.keys_the_bench_never_sent
      ? ` (${count.keys_the_bench_never_sent} seen that the bench never sent, skipped by the join)` : '')
    + (count.every_keystroke_accounted_for ? '' : ' — not every keystroke is accounted for'),
  ];
  // Said out loud rather than left in the JSON: warming the stage is this command deciding which
  // launch the cold-start budget judges, and a decision that changes a pass into a fail belongs in
  // front of the owner rather than three levels into a result file.
  if (decided.stage_first_client != null) {
    lines.push(`gate bench ${what}: the stage's first client cold-started in `
      + `${say(r2(decided.stage_first_client))} ms and is not measured — ours is the launch after it`);
  }
  lines.push(panel
    ? numbers(regime, said, panel) + informational(panel)
    : numbers(regime, said)
      + ` (budget mean <= ${BUDGET.mean_ms}, worst <= ${BUDGET.worst_ms}, cold <= ${BUDGET.cold_ms} ms;`
      + ` oracle ${ORACLE.uinput_to_presented_ms}, ${ORACLE.worst_ms}, ${ORACLE.cold_ms} ms)`);
  return lines;
}

/// One regime's line in a run of several.
///
/// A run of twelve is read down the left-hand edge, so each regime says the same five numbers in
/// the same order and nothing else: the budget and the oracle are said once, at the end, by
/// `allSummary`. The accounting is not repeated either — a regime whose keys do not add up never
/// reaches this line, because the bench refuses it — except that a run kept for the record says so
/// where it would otherwise read as a clean fail.
export function regimeLine(regime, decided) {
  const said = decided.verdict;
  return numbers(regime, said, decided.panel ?? null)
    + (decided.accounting.every_keystroke_accounted_for ? '' : ' — not every keystroke is accounted for');
}

/// The last line of a run of several: pass only when every regime in it cleared the budget.
///
/// `ran` is what was asked for — `--all`, or `--regimes a,b` — because a run of two that passed
/// and a run of twelve that passed are not the same evidence, and the line the owner reads should
/// not need the command scrolled back to to tell them apart.
export function allSummary(ran, rows, panel = null) {
  // A panel run has nothing to pass or fail: the budget is set on the headless output, so counting
  // how many of these regimes cleared it would be inventing a verdict out of numbers taken
  // somewhere else. It says how many it measured, and where.
  if (panel) {
    return `gate bench ${labelled(ran, panel)}: informational — `
      + `${rows.length} regime${rows.length === 1 ? '' : 's'} measured${informational(panel)}`;
  }
  const failed = rows.filter((r) => !r.pass).map((r) => r.regime);
  return `gate bench ${ran}: ${failed.length ? 'fail' : 'pass'} — `
    + `${rows.length - failed.length} of ${rows.length} regimes clear the budget`
    + (failed.length ? `, not ${failed.join(', ')}` : '')
    + ` (mean <= ${BUDGET.mean_ms}, worst <= ${BUDGET.worst_ms}, cold <= ${BUDGET.cold_ms} ms;`
    + ` oracle ${ORACLE.uinput_to_presented_ms}, ${ORACLE.worst_ms}, ${ORACLE.cold_ms} ms)`;
}

// ---------- the latency Piece's verdict ----------
//
// The other eight Pieces are won by a critic looking at two pictures. This one is won by
// subtraction, so the rule is written out here, next to the two sets of numbers it is a rule about,
// and `tools/gate judge latency` does the file-reading and the ledger-writing around it.

/// Every bar one regime is held to, furthest from its bar first.
///
/// Three for every regime — the Gate's hard budget — and two more for the headline regime, which is
/// the one the Piece is won on: the oracle's own keystroke and cold-start numbers, which are what
/// beating the Parity oracle means. A number that was never measured is not a number that passed,
/// so it is placed further from its bar than any measured number can be.
export function against(row, headline) {
  const bars = [
    { what: 'mean', ours: row.mean_ms, bar: BUDGET.mean_ms, whose: 'the budget' },
    { what: 'worst', ours: row.worst_ms, bar: BUDGET.worst_ms, whose: 'the budget' },
    { what: 'cold start', ours: row.cold_ms, bar: BUDGET.cold_ms, whose: 'the budget' },
  ];
  if (headline) {
    bars.push({ what: 'mean', ours: row.mean_ms, bar: ORACLE.uinput_to_presented_ms, whose: 'the oracle', strict: true });
    bars.push({ what: 'cold start', ours: row.cold_ms, bar: ORACLE.cold_ms, whose: 'the oracle', strict: true });
  }
  return bars
    .map((b) => ({ ...b, regime: row.regime, ratio: b.ours == null ? Infinity : b.ours / b.bar }))
    .sort((a, b) => b.ratio - a.ratio);
}

/// Whether one bar is cleared, and the two rules are not the same rule.
///
/// The budget is written with a `≤` — `docs/agents/gate.md` says "≤ 5 ms mean, ≤ 16 ms worst" — so
/// a number exactly on it has cleared it. Beating the oracle is written as *under*: #41's rule is
/// "the headline regime's mean under the oracle's 12.17 ms uinput → presented and cold start under
/// 370 ms". A tie is therefore theirs, because drawing with the app we are trying to beat is not
/// beating it. The distinction only ever decides an exact tie, which is why it is said here once
/// rather than left to each reader of a ratio.
export const clears = (bar) => (bar.strict ? bar.ratio < 1 : bar.ratio <= 1);

// The ledger's two words for how far a verdict was from the bar. This one is arithmetic rather than
// a critic's impression, so it is a distance: within a quarter of the bar either side of it is
// narrow, and anything further out is clear.
const marginOf = (ratio) => (ratio > 0.75 && ratio <= 1.25 ? 'narrow' : 'clear');

const asPercent = (ratio) => (Number.isFinite(ratio) ? `${Math.round(ratio * 100)}% of it` : 'no number at all');

/// The latency Piece's verdict over a whole `bench --all` summary.
///
/// Ours when every regime clears the budget and the headline regime is under the oracle's own
/// keystroke and cold-start numbers — which is the rule as `docs/agents/gate.md` states it, with
/// the budget as the floor and the oracle as what winning is measured against. One state per
/// regime, because that is what a round has room to record and what a reader would want to see.
export function latencyVerdict(summary) {
  const rows = summary.regimes || [];
  const headline = rows.find((r) => r.regime === summary.headline) || null;

  const states = rows.map((row) => {
    const bars = against(row, row.regime === summary.headline);
    const tight = bars[0];
    return {
      name: row.regime,
      result: row.file,
      winner: bars.every(clears) ? 'ours' : 'theirs',
      mean_ms: row.mean_ms,
      worst_ms: row.worst_ms,
      p50_ms: row.p50_ms,
      p99_ms: row.p99_ms,
      cold_ms: row.cold_ms,
      closest_bar: `${tight.what} ${say(tight.ours)} ms against ${tight.whose}'s ${tight.bar} ms`,
      at: asPercent(tight.ratio),
    };
  });

  const tightest = rows
    .flatMap((row) => against(row, row.regime === summary.headline))
    .sort((a, b) => b.ratio - a.ratio)[0] || null;

  const gap = tightest === null
    ? 'The run measured no regimes, so there is nothing to hold to a bar.'
    : `${tightest.regime} is ${clears(tightest) ? 'the closest to a bar' : 'past a bar'}: `
      + `${tightest.what} ${say(tightest.ours)} ms against ${tightest.whose}'s ${tightest.bar} ms, `
      + `${asPercent(tightest.ratio)}.`;

  const gapTheirs = headline === null
    ? `The run has no ${summary.headline} in it, so there is nothing to put beside the Parity `
      + `oracle's ${ORACLE.uinput_to_presented_ms} ms uinput → presented and ${ORACLE.cold_ms} ms cold.`
    : `The Parity oracle is ${ORACLE.uinput_to_presented_ms} ms uinput → presented and `
      + `${ORACLE.cold_ms} ms cold; ours at ${summary.headline} is ${say(headline.mean_ms)} ms and `
      + `${say(headline.cold_ms)} ms.`;

  return {
    winner: states.length && states.every((s) => s.winner === 'ours') ? 'ours' : 'theirs',
    margin: tightest === null ? 'clear' : marginOf(tightest.ratio),
    gap,
    gapTheirs,
    verdict: (summary.lines || []).join('\n'),
    states,
    tightest,
  };
}
