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
// The Preview pane is the second subject, and it is outside both oracles for a plainer reason than
// the caret's. `legacy/` has no rendered page at all — it shows Markdown as source and nothing
// else — and iA Writer for Mac's own preview is that app's design rather than this one's:
// [#263](https://github.com/danielbaldwin47/Quill/issues/263) § Out of Scope puts Preview outside
// ADR 0015's reach and makes its look the spec's own. There is no pair to put in front of anybody,
// so `split` and `full` are measured instead: where the divider stands, which paper is which side
// of it, and whether the heading at the top of the rendered page is centred in the pane it is drawn
// in.
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
export const ASSERTIONS = { ghost: ghost, split: split, full: full };

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
  split: bare('split'),
  full: bare('full'),
};

// A rule with nothing to configure, checked for an entry that thinks otherwise.
//
// The Preview rules read every number they compare out of the shot — the window's own width, the
// papers either side of the divider, the pane's centre — so there is nothing for the state to name
// but the kind. An entry naming more than that has set something nobody reads, which is a state
// quietly measuring something other than what it says.
function bare(kind) {
  return (spec) => {
    const extra = Object.keys(spec).filter((k) => k !== 'kind');
    if (extra.length) {
      throw new Error(`the ${kind} assertion takes nothing but its kind, and this one names ${extra.join(', ')}`);
    }
  };
}

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

// ---------- the Preview pane ----------

// How far a column read off the glass may sit from where the geometry puts it, in device pixels.
//
// Three, and they are the oracle's own: `ref/ia/mac-native/NOTES.md` § State 16 measures a rendered
// heading 3 px off the window centre it is centred on, and calls that the glyph rounding. The same
// rounding is here twice over — a pane's half is a whole pixel only when the window's width is
// even, and a run of ink is bounded by the antialiased edge of its first and last glyph rather than
// by the layout that placed them. Three pixels is a thousandth of the window: a divider at the
// wrong fraction, or a heading left on its measure instead of centred, misses by hundreds.
const CENTRE = 3;

// How far from the paper a pixel must sit, on its furthest channel, to be counted as ink.
//
// The two papers this reads between are 10 apart at the dark palette (`#1a1a1a` against `#101010`),
// so the threshold has to stand well clear of that or one pane's paper bleeding a pixel into the
// other would read as a glyph. Forty is four times that separation, and a heading's strokes are far
// darker than it at their core, so the extent is read from the letters rather than from whichever
// part of their skirt a lower threshold happened to catch.
const INK = 40;

// Split: the pair, divided down the middle, with a different paper in each half.
//
// Three facts, and a still holds all three. The divider stands at half the window, because Split
// opens at an even divide and the drag that moves it is the writer's rather than a judged state's.
// The two halves carry two papers, the Editor's and the Template's, which is what makes the
// rendered page a page and not more of the Editor — `ref/ia/mac-native/NOTES.md` § State 16
// measures the same two grounds side by side in one window. And the heading at the top of the
// rendered page is centred in the pane it is drawn in, which is the fact that catches a page laid
// out against the window rather than against the half it was put in.
//
// One frame, and `shots.dim` is it: the state's own shot, taken at the flags the state names.
// `tools/judge.mjs` shoots every asserted state a second time with the window active, because the
// ghost is about activation and needs a lit reference — Split has nothing to say about activation,
// so the second frame goes unread and is kept beside the round like the first.
//
// No colour is compared against a hex written down here. Each paper is read as the colour its half
// of the window is mostly made of, and the rule is that the two differ, because a palette that
// moves takes both of them with it — this repo has twice had a pinned hex outlive its palette.
function split(_spec, { dim }) {
  const png = decodePng(dim);
  const { w, h } = png;
  const left = paperOf(png, 0, w >> 2, 0, h);
  const right = paperOf(png, w - (w >> 2), w, 0, h);
  if (same(left, right)) {
    return no(`both halves of the window are ${hex(left)}: the pane beside the Editor is not a rendered page on its own paper`);
  }

  const edge = dividerOf(png, left, right);
  if (edge === null) {
    return no(`${hex(left)} and ${hex(right)} do not divide the window into two runs, so there is no divider between them to measure`);
  }
  const ink = firstInk(png, edge.firstRight, w, right);
  if (ink === null) {
    return no(`the pane right of x ${edge.firstRight} is ${hex(right)} from top to bottom: nothing was rendered on it`);
  }

  // The divider is a boundary between columns and the ink's centre is a column, so the two are
  // measured in the spaces they live in: half of `w` boundaries away for the one, half a pixel
  // inside the pane's last column for the other.
  const half = w / 2;
  const inkCentre = (ink.left + ink.right) / 2;
  const paneCentre = (edge.firstRight + w - 1) / 2;
  const offDivider = Math.abs(edge.divider - half);
  const offHeading = Math.abs(inkCentre - paneCentre);
  const where = `the divider stands at x ${edge.divider} of ${w}, ${((100 * edge.divider) / w).toFixed(1)} % across the window, `
    + `with ${hex(left)} paper left of it and ${hex(right)} right`;
  const heading = `the heading's ink runs x ${ink.left}..${ink.right} over rows y ${ink.top}..${ink.bottom}, `
    + `centre ${inkCentre}, against the pane's own ${paneCentre}`;
  const missed = [];
  if (offDivider > CENTRE) missed.push(`the divider is ${offDivider.toFixed(1)} px off the window's half at ${half}`);
  if (offHeading > CENTRE) missed.push(`the heading is ${offHeading.toFixed(1)} px off the centre of the pane it is drawn in`);
  return {
    ours: missed.length === 0,
    divider: edge.divider,
    papers: [hex(left), hex(right)],
    heading: inkCentre,
    why: missed.length === 0
      ? `${where}; ${heading}`
      : `${where}; ${heading} — ${missed.join(', and ')}, past the ${CENTRE} px this is measured to`,
    secondary: [
      heading,
      `both papers were read as the colour each half of the window is mostly made of, not compared against a hex written down here`,
    ],
  };
}

// Full: the rendered page where the Editor's scroller was, and nothing beside it.
//
// Two facts. One paper across the whole window — Full hides the Editor's scroller rather than
// shrinking it, so a second ground anywhere is a pane that did not go away. And the heading at the
// top of the page centred on the window's own centre, which is what the pane's centre is when the
// pane is the window: `ref/ia/mac-native/NOTES.md` § State 16 measures its own rendered heading the
// same way, 3 px off the centre it is centred on.
//
// The one frame the state names, for the reason [`split`] reads it: this is not about activation.
//
// The paper is read rather than named, so this holds at either theme and outlives the palette. What
// it cannot see is which paper it got — a Full pane showing the Editor's own ground would be one
// paper too — and that is the heading's job: the Editor left-aligns a heading on a measure centred
// in the window, so its ink lands hundreds of pixels left of centre and fails here.
function full(_spec, { dim }) {
  const png = decodePng(dim);
  const { w, h } = png;
  const bands = [
    ['left', paperOf(png, 0, w >> 2, 0, h)],
    ['right', paperOf(png, w - (w >> 2), w, 0, h)],
    ['top', paperOf(png, 0, w, 0, h >> 2)],
    ['bottom', paperOf(png, 0, w, h - (h >> 2), h)],
  ];
  const paper = bands[0][1];
  const odd = bands.filter(([, band]) => !same(band, paper));
  if (odd.length) {
    return no(`the window is not one paper: ${hex(paper)} down its left edge against `
      + `${odd.map(([side, band]) => `${hex(band)} at the ${side}`).join(', ')} — Full puts the rendered page where the Editor's scroller was and leaves nothing beside it`);
  }

  const ink = firstInk(png, 0, w, paper);
  if (ink === null) return no(`the window is ${hex(paper)} from edge to edge: nothing was rendered on it`);
  const centre = (w - 1) / 2;
  const inkCentre = (ink.left + ink.right) / 2;
  const off = Math.abs(inkCentre - centre);
  const where = `one paper, ${hex(paper)}, across all ${w}x${h} of the window`;
  const heading = `the heading's ink runs x ${ink.left}..${ink.right} over rows y ${ink.top}..${ink.bottom}, `
    + `centre ${inkCentre}, against the window's own ${centre}`;
  return {
    ours: off <= CENTRE,
    paper: hex(paper),
    heading: inkCentre,
    why: off <= CENTRE
      ? `${where}; ${heading}`
      : `${where}; ${heading}, which is ${off.toFixed(1)} px out and past the ${CENTRE} px this is measured to`,
    secondary: [
      heading,
      `the paper was read as the colour the window is mostly made of, not compared against a hex written down here`,
    ],
  };
}

// The colour a rectangle of the shot is mostly made of, as `[r, g, b]`.
//
// A pane is mostly paper — a page of prose is a few per cent ink — so the commonest colour in a
// rectangle wholly inside one pane is that pane's ground, whatever the palette says it should be.
function paperOf(png, x0, x1, y0, y1) {
  const seen = new Map();
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const k = (at(png, x, y, 0) << 16) | (at(png, x, y, 1) << 8) | at(png, x, y, 2);
      seen.set(k, (seen.get(k) ?? 0) + 1);
    }
  }
  let best = 0;
  let held = -1;
  for (const [k, n] of seen) if (n > held) { best = k; held = n; }
  return [(best >> 16) & 255, (best >> 8) & 255, best & 255];
}

// Where the left paper gives way to the right, or `null` when the two do not divide the window.
//
// Read as a majority down each column rather than along one row: a column through a paragraph is
// mostly paper and a few rows of glyph, and a column that is mostly one paper is inside that pane.
// The divider is then the boundary between the last column that is mostly the left paper and the
// first that is mostly the right, so a hairline drawn between them is straddled rather than counted
// into either pane. A left-paper column found after the right pane has begun is not a divided
// window at all, and is refused rather than measured to the first crossing.
function dividerOf(png, leftPaper, rightPaper) {
  const { w, h } = png;
  let lastLeft = -1;
  let firstRight = -1;
  for (let x = 0; x < w; x += 1) {
    let l = 0;
    let r = 0;
    for (let y = 0; y < h; y += 1) {
      if (is(png, x, y, leftPaper)) l += 1;
      else if (is(png, x, y, rightPaper)) r += 1;
    }
    if (l * 2 > h) {
      if (firstRight >= 0) return null;
      lastLeft = x;
    } else if (r * 2 > h && firstRight < 0) {
      firstRight = x;
    }
  }
  if (lastLeft < 0 || firstRight < 0) return null;
  return { lastLeft, firstRight, divider: (lastLeft + 1 + firstRight) / 2 };
}

// The first run of ink below the top of a pane, as its bounding box, or `null` for a pane with none.
//
// Rows are taken while they carry ink and stopped at the first that does not. Every row of a line of
// text carries some and the air under a heading carries none, so the run that starts at the pane's
// topmost ink row is that heading, without the paragraph beneath it in the extent.
function firstInk(png, x0, x1, paper) {
  let top = -1;
  for (let y = 0; y < png.h && top < 0; y += 1) {
    for (let x = x0; x < x1; x += 1) if (inked(png, x, y, paper)) { top = y; break; }
  }
  if (top < 0) return null;
  let left = png.w;
  let right = -1;
  let bottom = top;
  for (let y = top; y < png.h; y += 1) {
    let row = false;
    for (let x = x0; x < x1; x += 1) {
      if (!inked(png, x, y, paper)) continue;
      row = true;
      if (x < left) left = x;
      if (x > right) right = x;
    }
    if (!row) break;
    bottom = y;
  }
  return { top, bottom, left, right };
}

function is(png, x, y, rgb) {
  return at(png, x, y, 0) === rgb[0] && at(png, x, y, 1) === rgb[1] && at(png, x, y, 2) === rgb[2];
}

function inked(png, x, y, paper) {
  for (let c = 0; c < 3; c += 1) if (Math.abs(at(png, x, y, c) - paper[c]) >= INK) return true;
  return false;
}

function same(a, b) {
  return a[0] === b[0] && a[1] === b[1] && a[2] === b[2];
}

function hex(rgb) {
  return `#${rgb.map((v) => v.toString(16).padStart(2, '0')).join('')}`;
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
