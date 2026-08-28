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

/// The whole of what `tools/gate bench` prints, as an array of lines.
///
/// Three, in the order they are read: what the run consisted of, what it measured, and whether that
/// clears the budget with the oracle's numbers beside it. The last one is the verdict, because the
/// Gate's rule is that every command ends in the line the owner reads.
export function summary(regime, decided) {
  const count = decided.accounting;
  const said = decided.verdict;
  const lines = [
    `gate bench ${regime}: ${count.keys_sent} keys sent`
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
    lines.push(`gate bench ${regime}: the stage's first client cold-started in `
      + `${say(r2(decided.stage_first_client))} ms and is not measured — ours is the launch after it`);
  }
  lines.push(
    `gate bench ${regime}: ${said.pass ? 'pass' : 'fail'} — mean ${say(said.mean_ms)} ms`
    + `, worst ${say(said.worst_ms)} ms, p50 ${say(said.p50_ms)} ms, p99 ${say(said.p99_ms)} ms`
    + `, cold ${say(said.cold_ms)} ms`
    + ` (budget mean <= ${BUDGET.mean_ms}, worst <= ${BUDGET.worst_ms}, cold <= ${BUDGET.cold_ms} ms;`
    + ` oracle ${ORACLE.uinput_to_presented_ms}, ${ORACLE.worst_ms}, ${ORACLE.cold_ms} ms)`,
  );
  return lines;
}
