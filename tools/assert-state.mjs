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
export const ASSERTIONS = { ghost: ghost, folded: folded };

// How each rule wants its second shot taken: the state to shoot, and what to shoot it with.
//
// Every asserted state is shot twice, because every one of these rules is a comparison of ours with
// ours (ADR 0017). What the second shot is differs by rule, and the difference is here rather than
// in the two callers: `ghost` wants the same state with the window active, and `folded` wants the
// same state with Live off, which is the page whose markers the fold is measured against.
export const SECOND = {
  ghost: (s) => ({ state: s, options: { active: true } }),
  folded: (s) => ({ state: { ...s, flags: { ...s.flags, live: false } }, options: {} }),
};

// The second shot one asserted state asks for: `{ state, options }` for `shootState`.
export function secondShot(spec, s) {
  validate(spec);
  return SECOND[spec.kind](s);
}

// One asserted state's answer: `{ ours, why, secondary }`, `ours` being whether the rule held.
//
// `shots` is `{ dim, lit }`, both raw PNG buffers: `dim` is the state's own shot and `lit` the
// second one [`SECOND`] asked for — the same state with the window active for `ghost`, and the same
// state with Live off for `folded`. `spec` is the `assert` entry from `shots/oracle/states.json`. Throws when the state names an
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
  folded({ scale }) {
    if (typeof scale !== 'number' || !(scale > 1)) {
      throw new Error(`the fold's heading scale is ${JSON.stringify(scale)}, and a rung of the Live ladder is over 1`);
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

// ---------- the fold ----------

// How far the measured ladder may sit from the rung the state names.
//
// A tenth, which is half the distance between two rungs: 1.6 and 1.4 cannot pass for each other,
// and neither can body size at 1.0. It is that wide because the ladder is applied to the type and
// the type is hinted — a judged shot pins hinted metrics, so the cell is a whole pixel at both
// sizes and the ratio comes back quantised: the 1.6 rung measures about 1.52 at the default step,
// which is 20 hinted pixels over 13.
const LADDER = 0.1;

// How far from the paper a channel must be before a pixel counts as ink.
//
// A folded marker is drawn in ink at alpha 0, which is no ink at all rather than a faint one, so
// this only has to clear the encoder's own noise; an antialiased glyph edge clears it easily.
const INK = 8;

// How far a glyph's own ink may stand from the column it was laid out at, as a share of the pitch.
//
// A body column is a pen position, and a glyph's ink is not obliged to begin there: a `W` at a
// heading's size reaches a few pixels to the left of it, and a hyphen begins a few to the right. The
// narrowest marker that can hang is `# ` at two cells, which is most of a pitch, so a tenth of one
// tells a bearing from a marker with room to spare either way.
const SKIRT = 0.1;

// Where two bands are one block, as a share of the pitch.
//
// Two rows of one paragraph are one pitch apart; two blocks have a blank line between them and are
// two pitches apart. Half way between the two is the only place this line can go.
const BLOCK = 1.5;

// The page with Live on against the same page with Live off.
//
// Three facts, and they are the three a still can hold. The block the caret is in is the writer's
// to edit, so its rows are the same pixels folded or not. Every marker outside it is off the page:
// nothing is left hanging in the gutter, and no block's words moved, because a marker that is not
// hung is folded to its own ground and its cells keep their advance for the furniture #274 draws in
// them. And a heading is set by the ladder: its ink is `scale` times the ink of the same heading
// unfolded, and its row is taller than a body row.
//
// The middle fact used to be the opposite one — that a list item's words moved *off* the body
// column, because the cells its bullet stood in were empty. That was true for exactly one ticket:
// #273 folded the markers and #274 draws a dot, a number, a task box and a hairline back into the
// cells they left, so a still can no longer see those cells as empty and must not claim to.
//
// Nothing is compared against a length written down here. The body column, the pitch, the blocks
// and the heading's two sizes are all read out of the two shots, for the reason `ghost` reads its
// colours out of them: this repo has twice found a pinned constant outliving what it was copied
// from.
function folded({ scale: want }, { lit, dim }) {
  const source = decodePng(lit);
  const page = decodePng(dim);
  if (source.w !== page.w || source.h !== page.h) {
    return no(`the folded shot is ${page.w}x${page.h} and the source one ${source.w}x${source.h}`);
  }
  const paper = paperOf(source, page);
  if (paper === null) {
    return no('the two shots do not agree about the paper at their four corners, so ink cannot be told from it');
  }

  const before = blocksOf(source, paper);
  const after = blocksOf(page, paper);
  if (before.length < 2 || after.length < 2) {
    return no(`the page is ${before.length} blocks of ink with the markers on it and ${after.length} with them folded, and this reads a passage`);
  }
  if (before.length !== after.length) {
    return no(`the fold changed how many blocks the page has: ${before.length} with the markers on it, ${after.length} with them folded`);
  }

  const found = readBar(page);
  if (!found.bar) return no('the folded shot has no caret in it, so which block is the writer’s cannot be read');
  const middle = Math.floor((found.bar.top + found.bar.bottom) / 2);
  const index = after.findIndex((block) => middle >= block.top && middle <= block.bottom);
  if (index < 0) return no(`the caret at row ${middle} stands in no block of ink, so it names no open block`);

  // The body column, read off the caret's own block: it is the one block Live folds nothing in, so
  // its words begin where every unmarked block's words begin.
  const column = after[index].left;
  if (before[index].left !== column) {
    return no(`the caret’s block begins at column ${before[index].left} with the markers on the page and ${column} with them folded: the fold moved the words the writer is editing`);
  }

  // The block the caret is in is the writer's to edit, so it is the same pixels either way. Read
  // row for row from each block's own first inked row, because the block below a heading set larger
  // does not stand at the same height on the two pages.
  const rows = same(source, before[index], page, after[index]);
  if (rows !== null) return no(`the caret’s block is not the same pixels folded and unfolded: ${rows}`);

  const skirt = Math.round(pitch(before) * SKIRT);
  const hung = Math.min(...before.map((block) => block.left));
  if (hung >= column - skirt) {
    return no(`nothing hangs in the gutter with the markers on the page (every block begins at ${column} or further in), so there is no fold here to measure`);
  }
  const left = Math.min(...after.map((block) => block.left));
  if (left < column - skirt) {
    return no(`a marker is still hanging in the gutter: ink at column ${left}, where the body column is ${column}`);
  }
  const shifted = after.findIndex((block, i) => before[i].left >= column - skirt && Math.abs(block.left - before[i].left) > skirt);
  if (shifted >= 0) {
    return no(`the fold moved a block’s words: block ${shifted} begins at column ${before[shifted].left} with the markers on the page and ${after[shifted].left} with them folded, and a marker that hangs in no gutter keeps its cells`);
  }

  const grew = after[1].top - after[0].top - (before[1].top - before[0].top);
  // The heading's own words at the two sizes, and not its whole block: with the markers on the page
  // the block carries a `#` as well, whose ink is not the same height as the letters beside it.
  const words = (img, block) => words_(img, paper, block, column);
  const one = words(source, before[0]);
  const other = words(page, after[0]);
  if (one === null || other === null) {
    return no('the first block on the page has no ink on the body column, so the heading’s two sizes cannot be measured');
  }
  const got = height(other) / height(one);
  const off = Math.abs(got - want);
  const ladder = `the heading is set at ${got.toFixed(3)} of its unfolded ink, against the ${want} rung of the Live ladder`;
  const cells = `the gutter is empty and every block that hung nothing in it begins where it began with the markers on the page, so the cells a bullet, a number or a task box stood in kept their advance`;
  const row = `the heading’s row is ${grew} device px taller than a body row`;
  if (off > LADDER) {
    return no(`${ladder}, which is ${off.toFixed(3)} out and past the ${LADDER} this is measured to`);
  }
  if (grew <= 0) {
    return no(`${ladder}, but its row is no taller than a body row: the page below it stands where it stood unfolded`);
  }
  return {
    ours: true,
    scale: got,
    why: `${ladder}; ${row}; ${cells}; and the caret’s block is the same pixels folded and unfolded`,
    secondary: [cells, row, 'the body column, the pitch and the two heading sizes are read off the two shots, not from a length written down here'],
  };
}

// The paper both shots are drawn on, or `null` where their corners do not agree on one.
//
// A judged state that asserts a fold is shot with the bars off, so the four corners are page and
// nothing else. Two shots that disagree there are not two shots of one state.
function paperOf(a, b) {
  let paper = null;
  for (const [x, y] of [[1, 1], [a.w - 2, 1], [1, a.h - 2], [a.w - 2, a.h - 2]]) {
    for (const img of [a, b]) {
      const here = [0, 1, 2].map((c) => at(img, x, y, c));
      if (paper === null) paper = here;
      else if (here.some((v, c) => v !== paper[c])) return null;
    }
  }
  return paper;
}

// The blocks of ink on `img`, top to bottom: `{ top, bottom, left }` per block.
//
// A row of text is a run of scanlines carrying ink; two rows of one paragraph start one pitch
// apart and two blocks have a blank line between them, so they start two pitches apart. The rows
// are grouped into blocks at [`BLOCK`] pitches, the pitch being the one the rows themselves
// measure. `left` is the column that block's ink begins at, which
// is what says whether a marker is still standing in front of its words.
function blocksOf(img, paper) {
  const rows = [];
  for (let y = 0; y < img.h; y += 1) {
    let left = -1;
    for (let x = 0; x < img.w && left < 0; x += 1) {
      if ([0, 1, 2].some((c) => Math.abs(at(img, x, y, c) - paper[c]) > INK)) left = x;
    }
    rows.push(left);
  }
  const bands = [];
  let open = null;
  rows.forEach((left, y) => {
    if (left >= 0) {
      if (open === null) open = { top: y, bottom: y, left };
      else open = { ...open, bottom: y, left: Math.min(open.left, left) };
    } else if (open !== null) {
      bands.push(open);
      open = null;
    }
  });
  if (open !== null) bands.push(open);
  if (bands.length < 2) return bands;

  // The pitch is the closest two rows on the page stand: every row of one paragraph is one pitch
  // from the next, and nothing on a page of prose is closer than that. The smallest step rather
  // than the commonest, because how many rows a paragraph runs to changes with the fold and a
  // count of them would be a measurement of this passage rather than of the leading.
  const steps = bands.slice(1).map((band, i) => band.top - bands[i].top);
  const pitch = Math.min(...steps);
  const blocks = [];
  let last = null;
  for (const band of bands) {
    if (last === null || band.top - last.top >= pitch * BLOCK) blocks.push({ ...band });
    else {
      const block = blocks[blocks.length - 1];
      block.bottom = band.bottom;
      block.left = Math.min(block.left, band.left);
    }
    last = band;
  }
  return blocks;
}

// How tall a block of ink is, in device pixels.
const height = ({ top, bottom }) => bottom - top + 1;

// The pitch these blocks are set on: the closest two rows of ink stand, blocks and all.
const pitch = (blocks) => Math.min(...blocks.slice(1).map((block, i) => block.top - blocks[i].top));

// The rows of `block` carrying ink at or right of `column`, or `null` where it carries none.
//
// A heading's words without the `#` in front of them, which is the only way the same words can be
// measured at their two sizes: the marker is folded away on one page and standing on the other.
function words_(img, paper, block, column) {
  let top = null;
  let bottom = null;
  for (let y = block.top; y <= block.bottom; y += 1) {
    let ink = false;
    for (let x = column; x < img.w && !ink; x += 1) {
      if ([0, 1, 2].some((c) => Math.abs(at(img, x, y, c) - paper[c]) > INK)) ink = true;
    }
    if (ink) {
      if (top === null) top = y;
      bottom = y;
    }
  }
  return top === null ? null : { top, bottom };
}

// Why two blocks are not the same pixels, or `null` when they are.
//
// Compared row for row from each block's own top, because a block under a heading set larger sits
// further down the page: what is being asked is whether the fold changed the block, not where the
// page put it.
function same(a, one, b, other) {
  if (height(one) !== height(other)) {
    return `it is ${height(one)} device px of ink unfolded and ${height(other)} folded`;
  }
  const stride = a.w * a.ch;
  for (let i = 0; i < height(one); i += 1) {
    const here = a.data.subarray((one.top + i) * stride, (one.top + i + 1) * stride);
    const there = b.data.subarray((other.top + i) * stride, (other.top + i + 1) * stride);
    if (Buffer.compare(Buffer.from(here), Buffer.from(there)) !== 0) {
      return `row ${i} of it differs, at y ${one.top + i} unfolded and y ${other.top + i} folded`;
    }
  }
  return null;
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
