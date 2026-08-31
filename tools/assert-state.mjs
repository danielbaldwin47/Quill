// The judged states no critic decides: arithmetic read off ours' own pixels.
//
//   import { assertState, ASSERTIONS } from './assert-state.mjs'
//
// A blind pair asks a critic which of two apps did a thing better. That question needs two apps
// that both do the thing. `caret/unfocused` has not had two since the caret's column left the
// Parity oracle: iA drops the caret entirely when the window deactivates
// (`ref/ia/mac-native/VERDICTS.md` 0013.8), so the Design oracle cannot hold the state either, and
// `legacy/` holds it with the column `docs/design.md` row Caret column overruled. A critic put in
// front of that pair is choosing between our design and the one it replaced, and it chose the old
// one in round 8 — on the 5 device px of paper the centring gave up, which is
// [#147](https://github.com/danielbaldwin47/Quill/issues/147).
//
// So the state stops being a pair.
// [ADR 0017](../docs/adr/0017-a-judged-state-neither-oracle-can-arbitrate.md) makes it arithmetic,
// the way `tools/gate judge latency` already is: the ghost is facts about ours, and facts are
// measured rather than preferred.
//
// Everything here is a pure function of decoded PNGs, so `tools/judge-selftest.mjs` runs it over
// pixels it paints itself and no window is needed to know whether the rule holds.

import { decodePng, readBar } from './keys-assert.mjs';

// How far the measured alpha may sit from the one the state names.
//
// The ghost is solved out of 8-bit pixels, so the answer is quantised before it is compared: at the
// light palette the two usable channels give 0.2996 and 0.3036 for one ghost at 0.3. Two hundredths
// is wide enough that rounding never fails a correct build and narrow enough that the next alpha
// on the ladder — 0.25, or 0.35 — cannot pass for this one.
const TOLERANCE = 0.02;

// The smallest separation between the lit bar and the paper, per channel, that can carry an alpha.
//
// Alpha is solved as the share of the way the ghost lies from the paper towards the lit bar, and a
// channel where the two nearly agree divides by nearly nothing: at `#00bfff` on `#f7f7f7` the blue
// channel separates by 8, so one unit of rounding moves the answer by an eighth. Such a channel is
// dropped rather than weighted, because a mean is not a fix for a term that is noise.
const SEPARATION = 32;

// The assertions a state may name, by the word it names them with.
export const ASSERTIONS = { ghost: ghost };

// One asserted state's answer: `{ ours, why, secondary }`, `ours` being whether the rule held.
//
// `shots` is the state's own shot and the same state shot lit, both as raw PNG buffers, and
// `spec` is the `assert` entry from `shots/oracle/states.json`. Throws when the state names an
// assertion this file has not got, which `tools/gate judge` turns into a refusal before it shoots.
export function assertState(spec, shots) {
  validate(spec);
  return ASSERTIONS[spec.kind](spec, shots);
}

// Everything about an `assert` entry that can be known without a window, thrown as one message.
//
// Split out from the run so `tools/gate judge` can call it over every asserted state before it
// opens the first: a state naming a rule nobody wrote, or an alpha that is not one, is a state that
// cannot be judged, and finding that out at the third state has already spent two critics.
export function validate(spec) {
  const kind = spec?.kind;
  if (!Object.prototype.hasOwnProperty.call(ASSERTIONS, kind)) {
    throw new Error(`the assertion is ${JSON.stringify(kind)}, and this file has ${Object.keys(ASSERTIONS).join(', ')}`);
  }
  CHECKS[kind](spec);
}

// What each rule needs of its own entry, beyond the name.
const CHECKS = {
  ghost({ alpha }) {
    if (typeof alpha !== 'number' || !(alpha > 0) || !(alpha < 1)) {
      throw new Error(`the ghost's alpha is ${JSON.stringify(alpha)}, and an alpha is between 0 and 1`);
    }
  },
};

// ---------- the ghost ----------

// The caret when the window is not active: the same bar, faded.
//
// Two facts, and they are the two a still can hold. The bar keeps its column, its width and its
// band — deactivating a window moves nothing, so a ghost in a different place is the defect this
// would catch. And it is the lit bar at `alpha` over the paper, which is what "30 % ghost" means.
//
// The third fact — that the blink has stopped — is not here, because a still cannot hold it and
// does not need to: `--deterministic` freezes the blink on, and `Caret::alpha` returning `GHOST`
// three cycles into the quiet is asserted display-free in `quill/src/caret.rs`.
//
// Nothing is compared against a colour written down here. The accent, the paper and the alpha are
// all read out of the two shots, because this repo has twice found a pinned hex outliving the
// palette it was copied from.
function ghost({ alpha: want }, { lit, dim }) {
  const a = decodePng(lit);
  const b = decodePng(dim);
  if (a.w !== b.w || a.h !== b.h) {
    return no(`the lit shot is ${a.w}x${a.h} and the ghosted one ${b.w}x${b.h}`);
  }

  const bright = readBar(a);
  const faint = readBar(b);
  if (!bright.bar) return no('the lit shot has no caret in it at all');
  if (!faint.bar) return no('the ghosted shot has no caret in it at all');
  if (!bright.oneRun || !faint.oneRun) {
    return no(`the bar is not one run of colour (lit ${bright.oneRun}, ghosted ${faint.oneRun})`);
  }

  const same = ['left', 'right', 'top', 'bottom'].every((k) => bright.bar[k] === faint.bar[k]);
  if (!same) return no(`the bar moves when the window deactivates: ${box(bright.bar)} lit, ${box(faint.bar)} ghosted`);

  const y = Math.floor((bright.bar.top + bright.bar.bottom) / 2);
  const paper = groundAt(a, b, bright.bar, y);
  if (paper === null) {
    return no(`the bar does not stand in clear ground at row ${y}, so what it is drawn over cannot be read`);
  }
  const got = alphaOf(a, b, bright.bar, y, paper);
  if (got === null) {
    return no(`no channel separates the lit bar from the ground rgb(${paper}) enough to solve an alpha`);
  }
  const off = Math.abs(got - want);
  const why = `the ghost is the lit bar at alpha ${got.toFixed(3)} over the paper, against the ${want} docs/design.md row Caret on window deactivation names`;
  const where = `the bar is ${box(bright.bar)} lit and ghosted alike — deactivating the window moves it by nothing`;
  return {
    ours: off <= TOLERANCE,
    alpha: got,
    why: off <= TOLERANCE ? `${why}; ${where}` : `${why}, which is ${off.toFixed(3)} out and past the ${TOLERANCE} this is measured to`,
    secondary: [where, `alpha solved per channel from the two shots, not from a hex written down here`],
  };
}

// How many clear columns are wanted either side of the bar before its ground is called known.
const CLEAR = 3;

// The colour the bar is drawn over, or `null` if that cannot be read off these two shots.
//
// Neither shot shows what is under the bar: the lit one covers it at full alpha and the ghosted one
// mixes with it. So the ground is read from beside the bar instead, and the reading is only allowed
// when the bar demonstrably stands in a clear run — [`CLEAR`] columns each side of it, all the same
// colour, and that colour identical in both shots. That last is what keeps the bar's own
// antialiased skirt out of the sample: a column the caret touched at all differs between lit and
// ghosted, so it cannot pass. A run of paper either side is then the reason to believe the pixels
// between them are paper too.
//
// A caret standing on ink has no clear run, and this returns `null` rather than solving an alpha
// against the wrong ground — which is the honest answer, because a bar over a glyph genuinely
// cannot be read this way and a state that moved onto one should say so rather than drift.
function groundAt(a, b, bar, y) {
  const cols = [];
  for (let i = 1; i <= CLEAR; i += 1) cols.push(bar.left - i, bar.right + i);
  let ground = null;
  for (const x of cols) {
    if (x < 0 || x >= a.w) return null;
    const here = [0, 1, 2].map((c) => at(a, x, y, c));
    if (here.some((v, c) => v !== at(b, x, y, c))) return null;
    if (ground === null) ground = here;
    else if (here.some((v, c) => v !== ground[c])) return null;
  }
  return ground;
}

// The alpha the ghosted bar is the lit bar at, over `paper`.
//
// Read at the bar's middle column so an antialiased edge column is never the sample. `null` when no
// channel separates the lit bar from the ground enough to divide by.
function alphaOf(a, b, bar, y, paper) {
  const x = Math.floor((bar.left + bar.right) / 2);
  const shares = [];
  for (let c = 0; c < 3; c += 1) {
    const spread = at(a, x, y, c) - paper[c];
    if (Math.abs(spread) < SEPARATION) continue;
    shares.push((at(b, x, y, c) - paper[c]) / spread);
  }
  if (!shares.length) return null;
  return shares.reduce((t, s) => t + s, 0) / shares.length;
}

function at({ data, w, ch }, x, y, c) {
  return data[(y * w + x) * ch + c];
}

function box({ left, right, top, bottom }) {
  return `x ${left}..${right}, y ${top}..${bottom}`;
}

function no(why) {
  return { ours: false, why, secondary: [] };
}
