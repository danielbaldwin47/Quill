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
// constructed images and recorded native captures; no window is needed to test the rules.

import { SPELL, SPELL_DARK, decodePng, isSpellInk, readBar } from './keys-assert.mjs';

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
export const ASSERTIONS = {
  ghost: ghost,
  folded: folded,
  split: split,
  full: full,
  'pdf-split': pdfSplit,
  'pdf-full': pdfFull,
  dialog: dialog,
  syntax: syntax,
  outline: outline,
  spell: spell,
};

// How each rule wants its second shot taken: the state to shoot, and what to shoot it with.
//
// Every asserted state is shot twice, because every one of these rules is a comparison of ours with
// ours (ADR 0017). What the second shot is differs by rule, and the difference is here rather than
// in the two callers: `ghost` wants the same state with the window active, `folded` wants the
// same state with Live off, which is the page whose markers the fold is measured against, and the
// two Preview rules read one frame and take the active shot they do not read, as `ghost` does.
// `dialog` removes the Export flag for a bare PDF Split reference: that is the independent reading
// of the pane surround which the dialog shot must leave unchanged.
// `syntax` removes Syntax highlight while preserving Focus and Live, so glyph coverage and the
// bright rows are measured independently of the Category colours.
// `outline` removes the menu for the bare page: where the Outline shot differs from it is the panel.
// `spell` switches Spell check off and leaves every other flag standing, so the difference between
// the two shots is the waves — or, with no dictionary, the status line's one line.
export const SECOND = {
  ghost: (s) => ({ state: s, options: { active: true } }),
  folded: (s) => ({ state: { ...s, flags: { ...s.flags, live: false } }, options: {} }),
  split: (s) => ({ state: s, options: { active: true } }),
  full: (s) => ({ state: s, options: { active: true } }),
  'pdf-split': (s) => ({ state: s, options: { active: true } }),
  'pdf-full': (s) => ({ state: s, options: { active: true } }),
  dialog: (s) => ({ state: { ...s, flags: { ...s.flags, export: null } }, options: { active: true } }),
  syntax: (s) => ({ state: { ...s, flags: { ...s.flags, syntax: 'off' } }, options: {} }),
  outline: (s) => ({ state: { ...s, flags: { ...s.flags, menu: null } }, options: {} }),
  spell: (s) => ({ state: { ...s, flags: { ...s.flags, spell: 'off' } }, options: {} }),
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
  split: bare('split'),
  full: bare('full'),
  'pdf-split': bare('pdf-split'),
  'pdf-full': bare('pdf-full'),
  dialog: bare('dialog'),
  syntax: syntaxSpec,
  outline: bare('outline'),
  spell: spellSpec,
};

// The built-ins, restated from quill-engine/src/theme.rs, Colours::{LIGHT,DARK}, which #308
// measured off the Design oracle and #319 ported (docs/design.md row Syntax colours). Restated
// rather than read, because nothing here can call into the engine and a Category is judged by the
// hex it lands on rather than by a property of it — where the caret has a chroma test to lean on
// (tools/keys-assert.mjs, WHY A CHROMA TEST AND NOT THE ACCENT ITSELF), five hues on two grounds
// have none, and telling them apart is the whole of the rule. So a hand that changes theme.rs and
// not this goes red here, which is what the restatement buys. Exported so that a selftest derives
// its pigments from this rather than copying them a fourth time.
export const SYNTAX = {
  light: { ink: [25, 25, 25], colours: {
    nouns: [187, 81, 42], verbs: [70, 117, 181], adjectives: [157, 103, 34],
    adverbs: [166, 85, 159], conjunctions: [81, 129, 47],
  } },
  dark: { ink: [204, 204, 204], colours: {
    nouns: [206, 137, 109], verbs: [130, 158, 191], adjectives: [186, 150, 89],
    adverbs: [180, 144, 176], conjunctions: [137, 164, 116],
  } },
};

function syntaxSpec(spec) {
  const extra = Object.keys(spec).filter((k) => !['kind', 'theme', 'expected', 'focus', 'live', 'protected'].includes(k));
  if (extra.length) throw new Error(`the syntax assertion has unknown fields: ${extra.join(', ')}`);
  if (!Object.hasOwn(SYNTAX, spec.theme)) throw new Error(`the syntax theme is ${JSON.stringify(spec.theme)}, expected light or dark`);
  if (!Array.isArray(spec.expected) || !spec.expected.length || spec.expected.length > 5
      || new Set(spec.expected).size !== spec.expected.length
      || spec.expected.some((k) => !Object.hasOwn(SYNTAX[spec.theme].colours, k))) {
    throw new Error(`the syntax expected Categories are ${JSON.stringify(spec.expected)}, expected distinct Category names`);
  }
  for (const key of ['focus', 'live']) {
    if (spec[key] !== undefined && typeof spec[key] !== 'boolean') throw new Error(`the syntax ${key} must be boolean`);
  }
  // A fixture can name up to sixteen protected rectangles measured from its source-off capture.
  // The cap keeps the per-pixel membership check bounded independently of image dimensions.
  if (spec.protected !== undefined && (!Array.isArray(spec.protected) || spec.protected.length > 16
      || spec.protected.some((r) => !Array.isArray(r) || r.length !== 4
        || r.some((v) => !Number.isInteger(v) || v < 0) || !r[2] || !r[3]))) {
    throw new Error('the syntax protected regions must be at most sixteen [x, y, width, height] rectangles');
  }
}

// One full-image scan finds the reference column and bright rows; one checks the pixels against
// it. Each changed pixel tries at most five pigments and sixteen protected rectangles. Thus work
// is O(width * height), with O(height) auxiliary rows; the real-frame selftest reports its cost.
// Counts require 32 opaque pixels, well below one word in the pinned scale-2 captures, but more
// than a stray coloured pixel. Absence has no allowance: even one opaque unexpected pixel fails.
const SYNTAX_MIN = 32;
// A baseline's 8-bit coverage and a repainted 8-bit channel each round once. Their composition
// differs by at most two channel values; this is quantisation tolerance, never a spatial skirt.
const SYNTAX_ROUNDING = 2;

function syntax(spec, { lit, dim }) {
  const source = decodePng(lit);
  const page = decodePng(dim);
  if (source.w !== page.w || source.h !== page.h) return no(`syntax shots differ in size: ${source.w}x${source.h} and ${page.w}x${page.h}`);
  const paper = paperOf(source, page);
  if (!paper) return no('syntax shots disagree about the paper at their corners');
  const { ink, colours } = SYNTAX[spec.theme];
  if (Math.abs(ink[0] - paper[0]) < SEPARATION) return no('syntax paper and body ink do not separate enough to read coverage');
  const protectedRegions = spec.protected ?? [];
  for (const [x, y, w, h] of protectedRegions) {
    if (x + w > page.w || y + h > page.h) return no('a syntax protected rectangle runs outside the shot');
  }
  const bright = new Uint32Array(source.h);
  const column = { left: source.w, right: -1, top: source.h, bottom: -1 };
  for (let y = 0; y < source.h; y += 1) {
    for (let x = 0; x < source.w; x += 1) {
      if (is(source, x, y, ink)) bright[y] += 1;
      if (!inked(source, x, y, paper)) continue;
      column.left = Math.min(column.left, x);
      column.right = Math.max(column.right, x);
      column.top = Math.min(column.top, y);
      column.bottom = Math.max(column.bottom, y);
    }
  }
  if (column.right < column.left) return no('syntax reference has no text column');
  const rows = bandsOf(source, { ...column, left: column.left - 1, right: column.right + 1 }, paper);
  const heading = rows[0];
  const brightRows = rows.filter((r) => {
    for (let y = r.top; y <= r.bottom; y += 1) if (bright[y] >= SYNTAX_MIN) return true;
    return false;
  });
  const allowed = new Uint8Array(source.h);
  for (const row of brightRows) allowed.fill(1, row.top, row.bottom + 1);
  if (spec.focus && (!brightRows.length || brightRows.length === rows.length)) return no('syntax Focus reference must contain both bright and dim text rows');
  // A caret extends past the glyphs and can join two ink bands. Leave its band out of the body
  // measurement, then require the heading taller than every remaining body band.
  const caret = spec.live ? readBar(source).bar : null;
  const bodyHeights = rows.slice(1)
    .filter((r) => !caret || r.bottom < caret.top || r.top > caret.bottom).map(height);
  if (spec.live && (!bodyHeights.length || height(heading) <= Math.max(...bodyHeights))) {
    return no('syntax Live reference has no heading taller than its body rows');
  }
  const entries = Object.entries(colours);
  const counts = Object.fromEntries(entries.map(([name]) => [name, 0]));
  let headingPixels = 0;
  let changed = 0;
  for (let y = 0; y < page.h; y += 1) {
    for (let x = 0; x < page.w; x += 1) {
      const rgb = [0, 1, 2].map((c) => at(page, x, y, c));
      const baseline = [0, 1, 2].map((c) => at(source, x, y, c));
      const role = entries.find(([, colour]) => sameRgb(rgb, colour));
      if (role) {
        const [name] = role;
        if (!spec.expected.includes(name)) return no(`syntax has unexpected ${name} ink at ${x},${y}`);
        if (!sameRgb(baseline, ink)) return no(`syntax ${name} ink at ${x},${y} is outside a bright source glyph`);
        counts[name] += 1;
        if (y >= heading.top && y <= heading.bottom) headingPixels += 1;
      }
      if (sameRgb(rgb, baseline)) continue;
      changed += 1;
      if (x < column.left || x > column.right || y < column.top || y > column.bottom) return no(`syntax changed pixels outside the text column at ${x},${y}`);
      if (spec.focus && !allowed[y]) return no(`syntax colour leaked outside the bright rows at ${x},${y}`);
      if (protectedRegions.some(([rx, ry, w, h]) => x >= rx && x < rx + w && y >= ry && y < ry + h)) {
        return no(`syntax changed a protected marker, code or URL pixel at ${x},${y}`);
      }
      const alpha = (baseline[0] - paper[0]) / (ink[0] - paper[0]);
      const blend = (colour, value) => colour.every((v, c) => Math.abs(value[c] - Math.round(paper[c] + alpha * (v - paper[c]))) <= SYNTAX_ROUNDING);
      if (!(alpha > 0 && alpha <= 1) || !blend(ink, baseline)
          || !entries.some(([name, colour]) => spec.expected.includes(name) && blend(colour, rgb))) {
        return no(`syntax changed glyph geometry or non-prose ink at ${x},${y}`);
      }
    }
  }
  for (const name of spec.expected) {
    if (counts[name] < SYNTAX_MIN) return no(`syntax ${name} has ${counts[name]} opaque pixels, needs at least ${SYNTAX_MIN}`);
  }
  if (spec.live && headingPixels < SYNTAX_MIN) return no(`syntax Live heading has ${headingPixels} Category pixels, needs at least ${SYNTAX_MIN}`);
  const why = `${Object.entries(counts).map(([name, n]) => `${name} ${n}`).join(', ')} opaque pixels; ${changed} changed pixels keep source glyph coverage`
    + (spec.focus ? `; colour stays in ${brightRows.length} bright rows` : '')
    + (spec.live ? `; scaled heading holds ${headingPixels} Category pixels` : '');
  return { ours: true, why, counts, column, brightRows, heading, headingPixels, changed,
    secondary: ['same-state syntax-off pixels supply the text column, glyph coverage and bright rows',
      'provisional Role values: quill-engine/src/theme.rs Colours::LIGHT and Colours::DARK; no critic (ADR 0017)'] };
}

// ---------- Spell check ----------

// The waves, measured against the same page with Spell check switched off.
//
// Nothing on the page moves when Spell check comes on: the mark is a decoration over the flattened
// runs, Pango's `error` underline in the `spell` Role (`docs/architecture.md` § Annotators and the
// keystroke path). So the difference between the state's own shot and the same state shot
// `--spell off` is the waves and nothing else, and that difference is all this reads.
//
// Three facts a still can hold:
//
//   * how many waves there are — eight on `ref/spell.md`, with the caret parked at the end of
//     `comittee` too, because a parked caret keeps its word's wave and only a word being typed
//     withholds it (that case is `tools/gate keys spell`'s) — which is the tokeniser and the dictionary's
//     answer arriving on the page, the count `quill-engine/tests/spell_checker.rs` names word by
//     word;
//   * that every wave is a thin band lying under a line of prose, over columns that line has ink
//     in, and that nothing outside the text column changed: a wave on paper or in the margin is a
//     span that reached the page in the wrong place;
//   * that a wave is drawn in the `spell` Role, at any strength: the Role's hue on the paper,
//     antialiased or dimmed with its word under Focus, over a Category colour or a selection fill.
//     The hue test is `isSpellInk`'s, the keys rule's, and the two Role values are that file's
//     `SPELL` and `SPELL_DARK`, which its selftest holds to `quill-engine/src/theme.rs`.
//
// The "no dictionary" state is the fourth: Spell check on with no dictionary for the wanted tag
// marks nothing, and the one thing its shot adds is the status line's line. So the difference is
// one band of pixels at the foot of the window, and none of it is the Role's.
//
// The words' rectangles are read off the difference rather than written into the state, because
// where a word lands is the wrap's answer and not a fact about Spell check. The mark is provisional
// until the capture in [#400](https://github.com/danielbaldwin47/Quill/issues/400) lands.

// How far apart two changed pixels may sit and still be one wave, in device px. A wave crossing a
// descender can leave a few columns unchanged; the space between two words is about 25.
const SPELL_GAP = 10;
// The narrowest wave worth the name, in device px: `Teh`, the shortest misspelling in the passage,
// is three cells, about 75 device px at the judged step.
const SPELL_RUN = 40;
// The tallest a wave may be before it is a fill rather than an underline, in device px.
const SPELL_THICK = 16;
// The share of a wave's changed pixels that must be the Role's hue. The rest are where the wave's
// antialiased edge blends with a Category colour or a selection fill rather than the neutral paper,
// which moves the hue.
const SPELL_SHARE = 0.5;
// The share of a wave's columns the prose above it must carry ink in: a word, not paper.
const SPELL_COVER = 0.25;
// How far below a line of prose's ink a wave may lie and still be that line's, in device px.
const SPELL_BELOW = 16;
// The least a pixel's red must lead its green and blue by to be read as the wave, and how far its
// `(r - g) / (r - b)` may stray from the Role's. Lower than `isSpellInk`'s chroma floor, because a
// wave dimmed with its sentence under Focus on the light ground leads by less than that one's 40;
// the paper is neutral, so a dimmed or antialiased wave keeps the Role's ratio.
const WAVE_HUED = { chroma: 12, hue: 0.25 };
// The tallest the status line's band may be, in device px, and the share of the window above it.
const STATUS_TALL = 120;
const STATUS_FOOT = 0.75;

function spellSpec(spec) {
  const extra = Object.keys(spec).filter((k) => !['kind', 'theme', 'words', 'status'].includes(k));
  if (extra.length) throw new Error(`the spell assertion has unknown fields: ${extra.join(', ')}`);
  if (!['light', 'dark'].includes(spec.theme)) throw new Error(`the spell theme is ${JSON.stringify(spec.theme)}, expected light or dark`);
  if (!Number.isInteger(spec.words) || spec.words < 0 || spec.words > 64) {
    throw new Error(`the spell words are ${JSON.stringify(spec.words)}, and a count of waves is a whole number from 0 to 64`);
  }
  if (spec.status !== undefined && typeof spec.status !== 'boolean') throw new Error('the spell status must be boolean');
  if (spec.status && spec.words) throw new Error('the spell status state marks nothing, so its words are 0');
}

// One full-image scan finds the changed pixels, gathered per row into runs; the waves are grown
// from those runs. Work is O(width * height) with O(waves) auxiliary state.
function spell(spec, { lit, dim }) {
  const source = decodePng(lit);
  const page = decodePng(dim);
  if (source.w !== page.w || source.h !== page.h) return no(`spell shots differ in size: ${source.w}x${source.h} and ${page.w}x${page.h}`);
  const role = spec.theme === 'dark' ? SPELL_DARK : SPELL;
  const runs = [];
  const changedBox = { left: page.w, right: -1, top: page.h, bottom: -1 };
  let changed = 0;
  let hued = 0;
  for (let y = 0; y < page.h; y += 1) {
    let start = -1;
    let last = -1;
    for (let x = 0; x <= page.w; x += 1) {
      const hit = x < page.w && !sameRgb([0, 1, 2].map((c) => at(page, x, y, c)), [0, 1, 2].map((c) => at(source, x, y, c)));
      if (hit) {
        changed += 1;
        if (isSpellInk(page, x, y, role, WAVE_HUED)) hued += 1;
        changedBox.left = Math.min(changedBox.left, x);
        changedBox.right = Math.max(changedBox.right, x);
        changedBox.top = Math.min(changedBox.top, y);
        changedBox.bottom = Math.max(changedBox.bottom, y);
        if (start < 0) start = x;
        last = x;
        continue;
      }
      if (start >= 0 && (x >= page.w || x - last > SPELL_GAP)) {
        runs.push({ y, left: start, right: last });
        start = -1;
      }
    }
  }

  if (spec.status) {
    if (!changed) return no('spell with no dictionary changed no pixel: the status line carries no line');
    if (hued) return no(`spell with no dictionary drew ${hued} pixels of the spell Role at ${box(changedBox)}`);
    const tall = changedBox.bottom - changedBox.top + 1;
    if (tall > STATUS_TALL) return no(`spell with no dictionary changed a band ${tall} device px tall at ${box(changedBox)}, taller than one status line`);
    if (changedBox.top < page.h * STATUS_FOOT) return no(`spell with no dictionary changed pixels at ${box(changedBox)}, above the window's foot`);
    return { ours: true, why: `no wave; the status line's ${changed} changed pixels in one band at ${box(changedBox)}`, changed, band: changedBox,
      secondary: ['same-state --spell off pixels supply the page without the status line', 'no critic (ADR 0017)'] };
  }
  if (!spec.words) {
    return changed ? no(`spell expects no wave and changed ${changed} pixels at ${box(changedBox)}`)
      : { ours: true, why: 'no wave and no changed pixel', changed, secondary: ['no critic (ADR 0017)'] };
  }
  if (!changed) return no('spell changed no pixel at all: the page carries no wave');

  const paper = paperOf(source, page);
  if (!paper) return no('spell shots disagree about the paper at their corners');
  const column = { left: source.w, right: -1, top: source.h, bottom: -1 };
  for (let y = 0; y < source.h; y += 1) {
    for (let x = 0; x < source.w; x += 1) {
      if (!inked(source, x, y, paper)) continue;
      column.left = Math.min(column.left, x);
      column.right = Math.max(column.right, x);
      column.top = Math.min(column.top, y);
      column.bottom = Math.max(column.bottom, y);
    }
  }
  if (column.right < column.left) return no('spell reference has no text column');
  const lines = bandsOf(source, { ...column, left: column.left - 1, right: column.right + 1 }, paper);
  // A wave's last period can run a few px past its word's last ink, so the column is widened by a gap.
  if (changedBox.left < column.left - SPELL_GAP || changedBox.right > column.right + SPELL_GAP
      || changedBox.top < column.top || changedBox.bottom > column.bottom + SPELL_BELOW) {
    return no(`spell changed pixels at ${box(changedBox)}, outside the text column at ${box(column)}`);
  }

  // The waves: runs that touch, row to row, are one wave, grown in row order and then settled.
  const waves = [];
  for (const run of runs) {
    const found = waves.find((m) => run.y <= m.bottom + 1 && run.left <= m.right + SPELL_GAP && run.right >= m.left - SPELL_GAP);
    if (found) {
      found.bottom = Math.max(found.bottom, run.y);
      found.left = Math.min(found.left, run.left);
      found.right = Math.max(found.right, run.right);
    } else waves.push({ top: run.y, bottom: run.y, left: run.left, right: run.right });
  }
  for (let joined = true; joined;) {
    joined = false;
    for (let i = 0; i < waves.length && !joined; i += 1) {
      for (let j = i + 1; j < waves.length && !joined; j += 1) {
        const a = waves[i];
        const b = waves[j];
        if (a.top > b.bottom + 1 || b.top > a.bottom + 1) continue;
        if (a.left > b.right + SPELL_GAP || b.left > a.right + SPELL_GAP) continue;
        a.top = Math.min(a.top, b.top);
        a.bottom = Math.max(a.bottom, b.bottom);
        a.left = Math.min(a.left, b.left);
        a.right = Math.max(a.right, b.right);
        waves.splice(j, 1);
        joined = true;
      }
    }
  }
  if (waves.length !== spec.words) {
    return no(`spell holds ${waves.length} waves (${waves.map(box).join('; ')}) and the state expects ${spec.words}`);
  }

  for (const wave of waves) {
    const where = box(wave);
    const tall = wave.bottom - wave.top + 1;
    const wide = wave.right - wave.left + 1;
    if (tall > SPELL_THICK) return no(`a spell wave is ${tall} device px tall at ${where}, and an underline is a band`);
    if (wide < SPELL_RUN) return no(`a spell wave is ${wide} device px wide at ${where}, narrower than any misspelled word`);
    const line = lines.find((l) => wave.top <= l.bottom + SPELL_BELOW && wave.bottom >= l.top);
    if (!line) return no(`a spell wave at ${where} lies under no line of prose`);
    let inkedColumns = 0;
    for (let x = wave.left; x <= wave.right; x += 1) {
      for (let y = line.top; y <= line.bottom; y += 1) {
        if (!inked(source, x, y, paper)) continue;
        inkedColumns += 1;
        break;
      }
    }
    if (inkedColumns < wide * SPELL_COVER) {
      return no(`a spell wave at ${where} lies under ink in ${inkedColumns} of its ${wide} columns, and a wave lies under a word`);
    }
    let own = 0;
    let all = 0;
    for (let y = wave.top; y <= wave.bottom; y += 1) {
      for (let x = wave.left; x <= wave.right; x += 1) {
        if (sameRgb([0, 1, 2].map((c) => at(page, x, y, c)), [0, 1, 2].map((c) => at(source, x, y, c)))) continue;
        all += 1;
        if (isSpellInk(page, x, y, role, WAVE_HUED)) own += 1;
      }
    }
    if (own < all * SPELL_SHARE) return no(`a spell wave at ${where} is the spell Role in ${own} of its ${all} changed pixels`);
  }
  const why = `${waves.length} waves under their lines of prose, ${hued} of ${changed} changed pixels in the spell Role, none outside the text column`;
  return { ours: true, why, waves, changed, hued,
    secondary: ['same-state --spell off pixels supply the text column and the lines of prose',
      'the mark is provisional until the capture in #400 lands; no critic (ADR 0017)'] };
}

// A rule with nothing to configure, checked for an entry that thinks otherwise.
//
// The Preview rules and the Export dialog's read every number they compare out of the shot — the
// window's own width, the papers either side of the divider, the pane's centre, the rectangle the
// dialog's ground fills — so there is nothing for the state to name but the kind. An entry naming
// more than that has set something nobody reads, which is a state quietly measuring something other
// than what it says.
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
// A folded marker is drawn in the ground it stands on rather than at a faint alpha — `quill::tags`'
// `hidden` says why an alpha is impossible — so it lays down the paper's own channels and this only
// has to clear the encoder's own noise; an antialiased glyph edge clears it easily.
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
const PANE_INK = 40;

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
  const left = groundOf(png, 0, w >> 2, 0, h);
  const right = groundOf(png, w - (w >> 2), w, 0, h);
  if (sameRgb(left, right)) {
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
    ['left', groundOf(png, 0, w >> 2, 0, h)],
    ['right', groundOf(png, w - (w >> 2), w, 0, h)],
    ['top', groundOf(png, 0, w, 0, h >> 2)],
    ['bottom', groundOf(png, 0, w, h - (h >> 2), h)],
  ];
  const paper = bands[0][1];
  const odd = bands.filter(([, band]) => !sameRgb(band, paper));
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

// ---------- the Preview pane in PDF mode ----------

// PDF Split: the page column beside the Editor, the divider where Split puts it.
//
// The same three questions [`split`] asks of the rendered sheet, asked of a column of pages. The
// divider stands at half the window, for the reason it does there. The pane carries a ground of its
// own, and this one is the surround the pages stand on rather than a paper: the drawer paints every
// page in the Template's light palette whatever the theme is wearing, and the surround under them
// is one fixed neutral (#293 § The page column), so the pane reads light beside a dark Editor —
// which is the fact held here, as a comparison of the two grounds the shot gave up and not against
// a hex written down. And the page stands in the column with a gutter each side, centred in the
// pane it is drawn in to the tolerance the sheet's heading is held to.
//
// No colour is named. The Editor's paper is the colour its quarter of the window is mostly made of;
// the page's paper is the colour the half of the window the pane fills is mostly made of, the pages
// being nearly all of a column; the surround is what the rest of that half is mostly made of. A
// palette that moves takes all three with it.
//
// One frame, `shots.dim`, for the reason [`split`] reads one: this is not about activation.
function pdfSplit(_spec, { dim }) {
  const png = decodePng(dim);
  const { w, h } = png;
  const editor = groundOf(png, 0, w >> 2, 0, h);
  const { paper, surround } = columnOf(png, w >> 1, w);
  if (surround === null) {
    return no(`the window right of its half is ${hex(paper)} throughout: no page is standing on the surround`);
  }
  if (sameRgb(editor, surround)) {
    return no(`the pages stand on ${hex(surround)}, the ground the Editor is made of: the column has no surround of its own`);
  }

  const edge = dividerOf(png, editor, surround);
  if (edge === null) {
    return no(`${hex(editor)} and ${hex(surround)} do not divide the window into two runs, so there is no divider between them to measure`);
  }
  const { box, ink } = pageIn(png, paper, edge.firstRight, w);

  const half = w / 2;
  const paneCentre = (edge.firstRight + w - 1) / 2;
  const pageCentre = (box.left + box.right) / 2;
  const gutters = [box.left - edge.firstRight, w - 1 - box.right];
  const offDivider = Math.abs(edge.divider - half);
  const offPage = Math.abs(pageCentre - paneCentre);
  const where = `the divider stands at x ${edge.divider} of ${w}, ${((100 * edge.divider) / w).toFixed(1)} % across the window, `
    + `with ${hex(editor)} paper left of it and ${hex(paper)} pages on a ${hex(surround)} surround right`;
  const page = `the page runs x ${box.left}..${box.right}, gutters ${gutters[0]} and ${gutters[1]}, `
    + `centre ${pageCentre} against the pane's own ${paneCentre}`;
  const missed = [];
  if (offDivider > CENTRE) missed.push(`the divider is ${offDivider.toFixed(1)} px off the window's half at ${half}`);
  if (luma(surround) <= luma(editor)) {
    missed.push(`the surround is no lighter than the Editor's own paper, so the pane is wearing the theme rather than standing light beside it`);
  }
  if (gutters[0] <= 0 || gutters[1] <= 0) {
    missed.push(`the page reaches the edge of the pane instead of standing in it with a gutter each side`);
  }
  if (offPage > CENTRE) missed.push(`the page is ${offPage.toFixed(1)} px off the centre of the pane it stands in`);
  if (ink === null) missed.push(`the page is blank: nothing was drawn on it`);
  return {
    ours: missed.length === 0,
    divider: edge.divider,
    grounds: [hex(editor), hex(surround), hex(paper)],
    page: [box.left, box.top, box.right - box.left + 1, box.bottom - box.top + 1],
    why: missed.length === 0
      ? `${where}; ${page}; ${inkOn(ink)}`
      : `${where}; ${page} — ${missed.join(', and ')}, past the ${CENTRE} px this is measured to`,
    secondary: [
      `${page}; ${inkOn(ink)}`,
      `all three grounds were read as the colour their part of the window is mostly made of, not compared against a hex written down here`,
    ],
  };
}

// PDF Full: the column where the Editor's scroller was, its pages down the middle of the window.
//
// Three facts, and a still holds all three. The window carries two grounds and no more — the
// surround, and the paper the pages are drawn on — because Full hides the Editor rather than
// shrinking it. The page stands in the column with a gutter each side and centred on the window,
// which is what the pane's centre is when the pane is the window. And the column's air is above the
// first page as well as between any two: [`Stack::top_of`] in `quill/src/column.rs` puts one page
// margin over the first page and the same gap between each pair, so the gap this reads over page
// one is the gap between pages one and two.
//
// Both pages are in the shot because the state is shot for it: `preview/pdf-full` names a narrower
// window and `zoom: 75` (shots/oracle/states.json), which stands the second page's top edge below
// the first's foot with the air between them, the page still the ground most of the window is made
// of. At the judged 1440 px width and fit width a page is some 1900 px tall against a 900 px
// window, and no second page would reach the glass — so the count is asserted here rather than
// left to what happens to show.
//
// Nothing here reads an edge between the pane and the Editor's paper, because at the light palette
// this state is shot in there is none to read: the surround and the light theme's paper are one
// colour (`quill-engine/src/theme.rs`). What separates the two is the page's own white, the gutter
// either side of it, and the air the column stacks with — all of them inside the pane.
function pdfFull(_spec, { dim }) {
  const png = decodePng(dim);
  const { w, h } = png;
  const { paper, surround } = columnOf(png, 0, w);
  if (surround === null) {
    return no(`the window is ${hex(paper)} from edge to edge: no page is standing on the surround`);
  }
  const { box, ink } = pageIn(png, paper, 0, w);

  const pages = pagesIn(png, box, paper);
  const gaps = pages.slice(1).map((page, i) => page.top - pages[i].bottom - 1);
  const centre = (w - 1) / 2;
  const pageCentre = (box.left + box.right) / 2;
  const gutters = [box.left, w - 1 - box.right];
  const off = Math.abs(pageCentre - centre);
  const where = `${hex(paper)} pages on a ${hex(surround)} surround across all ${w}x${h} of the window`;
  const page = `the page runs x ${box.left}..${box.right}, gutters ${gutters[0]} and ${gutters[1]}, `
    + `centre ${pageCentre} against the window's own ${centre}, its top edge ${box.top} px down the column`
    + (gaps.length ? `, ${pages.length} pages with ${gaps.join(', ')} px between them` : ', one page of the column in view');
  const missed = [];
  if (gutters[0] <= 0 || gutters[1] <= 0) {
    missed.push(`the page reaches the edge of the window instead of standing in the column with a gutter each side`);
  }
  if (off > CENTRE) missed.push(`the page is ${off.toFixed(1)} px off the window's centre`);
  if (box.top <= 0) missed.push(`the first page starts at the top of the column, with none of the gap the column stacks its pages with over it`);
  if (pages.length < 2) {
    missed.push(`the window holds ${pages.length} page and the state is shot at the width and zoom that stand two in it, so there is no gap between two pages to read`);
  }
  for (const gap of gaps) {
    if (Math.abs(gap - box.top) > CENTRE) {
      missed.push(`a gap of ${gap} px between two pages against the ${box.top} px over the first, and the column stacks them with one gap`);
    }
  }
  if (ink === null) missed.push(`the page is blank: nothing was drawn on it`);
  return {
    ours: missed.length === 0,
    grounds: [hex(surround), hex(paper)],
    page: [box.left, box.top, box.right - box.left + 1, box.bottom - box.top + 1],
    gap: box.top,
    why: missed.length === 0
      ? `${where}; ${page}; ${inkOn(ink)}`
      : `${where}; ${page} — ${missed.join(', and ')}, past the ${CENTRE} px this is measured to`,
    secondary: [
      `${page}; ${inkOn(ink)}`,
      `both grounds were read as the colour their part of the window is mostly made of, not compared against a hex written down here`,
    ],
  };
}

// The two grounds a column of pages stands in, between `x0` and `x1`: the paper and the surround.
//
// The paper is the colour the column is mostly made of, the pages being nearly all of a column, and
// the surround is what the rest of it is mostly made of. `surround` is `null` when the column is
// one colour throughout, which is a column with no pages drawn on it.
function columnOf(png, x0, x1) {
  const paper = groundOf(png, x0, x1, 0, png.h);
  return { paper, surround: groundOf(png, x0, x1, 0, png.h, paper) };
}

// The page of `paper` between `x0` and `x1`: its rectangle, and the first ink on it.
//
// The rectangle is every pixel of exactly the paper's colour — one page's, or the union of what
// shows of each when the window holds more than one, which is the same rectangle in x either way.
// The ink is read inside that rectangle, so the surround, whose white is a handful of units off the
// paper's and well inside [`PANE_INK`], is never mistaken for a glyph.
function pageIn(png, paper, x0, x1) {
  const box = extentOf(png, paper, x0, x1);
  const ink = box.right < box.left ? null : firstInk(png, box.left, box.right + 1, paper);
  return { box, ink };
}

// The runs of rows inside `box` that are page rather than the air between two pages.
//
// A row of paper carries the page's paper at both its edges, because the margin is the widest thing
// on a page and nothing is drawn in it but the header and the footer; a row of air carries the
// surround there. So the runs are the pages the window holds, and what falls between two of them is
// the gap the column stacks with.
function pagesIn(png, box, paper) {
  const found = [];
  let open = null;
  for (let y = box.top; y <= box.bottom; y += 1) {
    if (!is(png, box.left, y, paper) || !is(png, box.right, y, paper)) {
      if (open) found.push(open);
      open = null;
      continue;
    }
    if (!open) open = { top: y, bottom: y };
    open.bottom = y;
  }
  if (open) found.push(open);
  return found;
}

// What a page's first block of ink reads as, for a rule's reading.
function inkOn(ink) {
  return ink === null
    ? 'no ink on the page'
    : `the first ink on it runs x ${ink.left}..${ink.right} over rows y ${ink.top}..${ink.bottom}`;
}

// How light a ground reads, as the mean of its channels.
//
// The grounds this stands between are neutral greys — the theme's paper and the column's fixed
// surround — so a mean says which of two is the lighter without a colour space to argue about.
function luma(rgb) {
  return (rgb[0] + rgb[1] + rgb[2]) / 3;
}

// ---------- the Export dialog ----------

// How near the window's own centre the dialog's box has to stand, in device px.
//
// Wider than [`CENTRE`]: the divider and the heading are drawn by the app inside one surface, and
// this is a second surface placed by the compositor against the first, whose height and width are
// the dialog's natural ones and can be odd. Half a logical pixel at scale 2 is 1 device px, and a
// box of odd extent centred in one of even extent is half a pixel out by construction.
const DIALOG_CENTRE = 8;

// How many bands of ink the dialog carries when its Options expander is open.
//
// A band is a run of rows carrying ink with air above and below it ([`bandsOf`]), which is what one
// row of a dialog is: the file name, the folder button, the Options label, the Export button. Shut,
// that is the four of them. Open, the PDF expander adds Paper, Text size, three Template toggles,
// Title page, Header, Footer and Save as defaults beneath the label. The number is a floor rather
// than a count, because two rows that meet leave one band and this is not a test of the row
// spacing; eight is more bands than a shut dialog can make however its rows fall together.
const DIALOG_BANDS = 8;

// How many rows of the dialog's own top margin its sides are read along ([`dialogBox`]), in device
// px: fewer than the margin the dialog keeps above its first control (24 in the judged still, 10 in
// the selftest's fixture), and more than one so an antialiased row cannot end the walk early.
const DIALOG_EDGE = 6;

// The Export dialog over the page, its Options expander open.
//
// Neither oracle holds this one either: `legacy/` has no Export dialog at all, and iA Writer for
// Mac's own is that app's dialog rather than this one's, so it is measured instead of shown to
// anybody (ADR 0017).
//
// Four facts, all read from the dialog shot against its bare `pdf-split` reference. The Editor's
// ground in the left quarter is darker than the pane surround: that is the app's scrim, scoped to
// the Editor. The pane surround in the right quarter is byte-for-byte the reference's surround:
// the dialog has not dimmed what it exists to preview. A light page stands on that surround, which
// says the pane is in PDF mode. The dialog's ground makes a rectangle centred on the window and it
// carries the bands of an open expander. The dialog and pane surround are the same light ground in
// this state, so [`dialogBox`] reads the centred rectangle by a majority of its rows and columns;
// it does not ask a globally shared colour for one impossible extent.
//
// What the entry says and where the switches stand are reported rather than held: the seeded file
// name's ink is in the topmost band, and the count of bands is the count of rows. A still cannot
// read a word, and this rule does not pretend to.
//
// No colour is compared against a hex written down here. The pair carries every ground the rule
// reads, so a palette that moves takes the measurement with it.
function dialog(_spec, { dim, lit }) {
  const png = decodePng(dim);
  const reference = decodePng(lit);
  const { w, h } = png;
  if (reference.w !== w || reference.h !== h) {
    return no(`the dialog shot is ${w}x${h} and its bare PDF Split reference is ${reference.w}x${reference.h}`);
  }

  const editor = groundOf(png, 0, w >> 2, 0, h >> 4);
  const pane = columnOf(png, (w * 3) >> 2, w);
  const referencePane = columnOf(reference, (w * 3) >> 2, w);
  if (pane.surround === null || referencePane.surround === null) {
    return no(`no page is standing in the pane beside it, so the pane is not showing what the dialog is about`);
  }
  if (!sameRgb(pane.surround, referencePane.surround)) {
    return no(`the pane surround ${hex(pane.surround)} changed from ${hex(referencePane.surround)} in bare PDF Split`);
  }
  if (luma(editor) >= luma(pane.surround)) {
    return no(`the ${hex(editor)} Editor is not darker than the ${hex(pane.surround)} pane surround`);
  }
  const sheet = referencePane.surround;
  const sheetBox = dialogBox(png, sheet);
  if (sheetBox.right < sheetBox.left || sheetBox.bottom < sheetBox.top) {
    return no(`no centred rectangle of the pane's ${hex(sheet)} surround was found: no dialog stands over the page`);
  }
  const inside = sheetBox.left > 0 && sheetBox.right < w - 1
    && sheetBox.top > 0 && sheetBox.bottom < h - 1;
  if (!inside) {
    return no(`the ${hex(sheet)} dialog ground runs ${box(sheetBox)} of a ${w}x${h} window: it reaches an edge, so it is not a dialog standing over the page`);
  }

  const bands = bandsOf(png, sheetBox, sheet);
  const boxCentre = [(sheetBox.left + sheetBox.right) / 2, (sheetBox.top + sheetBox.bottom) / 2];
  const centre = [(w - 1) / 2, (h - 1) / 2];
  const off = [Math.abs(boxCentre[0] - centre[0]), Math.abs(boxCentre[1] - centre[1])];
  const where = `the dialog is ${hex(sheet)}, ${box(sheetBox)} of a ${w}x${h} window, `
    + `centre ${boxCentre[0]},${boxCentre[1]} against the window's own ${centre[0]},${centre[1]}`;
  const rows = `${bands.length} bands of ink inside it, the topmost ${box(bands[0] ?? sheetBox)}`;
  const paneReading = `a ${hex(pane.paper)} page stands on a ${hex(pane.surround)} surround right of it`;
  const missed = [];
  if (off[0] > DIALOG_CENTRE || off[1] > DIALOG_CENTRE) {
    missed.push(`it stands ${off[0].toFixed(1)},${off[1].toFixed(1)} px off the window's centre`);
  }
  if (bands.length < DIALOG_BANDS) {
    missed.push(`it carries ${bands.length} bands of ink and an open expander carries at least ${DIALOG_BANDS}, so the Options are shut`);
  }
  if (luma(pane.paper) <= luma(pane.surround)) {
    missed.push(`the page beside it is no lighter than the ${hex(pane.surround)} it stands on, so it is not a page on the column's surround`);
  }
  return {
    ours: missed.length === 0,
    dialog: [
      sheetBox.left,
      sheetBox.top,
      sheetBox.right - sheetBox.left + 1,
      sheetBox.bottom - sheetBox.top + 1,
    ],
    grounds: [hex(editor), hex(sheet)],
    bands: bands.length,
    pane: [hex(pane.paper), hex(pane.surround)],
    why: missed.length === 0
      ? `${where}; ${rows}; ${paneReading}; the Editor is ${hex(editor)}`
      : `${where}; ${rows}; ${paneReading}; the Editor is ${hex(editor)} — ${missed.join(', and ')}, past the ${DIALOG_CENTRE} px this is measured to`,
    secondary: [
      `${rows}; ${paneReading}`,
      `the pane surround is held to bare PDF Split and every ground is read from the pair, not compared against a hex written down here`,
    ],
  };
}

// ---------- the Outline in the Palette ----------

// The least a channel moves, between the Outline shot and the bare page, for a pixel to count as
// the panel's: a popover's shadow fades into the paper, and its last steps are a unit or two that
// would push the panel's box out to where nothing is drawn.
const OUTLINE_CHANGED = 8;

// A band shorter than this is a rule, not a row: the hairline under the field is two pixels at the
// judged scale, and no row of type is under four.
const OUTLINE_RULE = 4;

// The Palette on the Outline (#397): a panel over the page holding the open Document's headings,
// the second stepped in under the first.
//
// Two facts, and they are the two a still can hold. A panel stands over the page, wholly inside
// the window — found as where the Outline shot differs from the bare page, which is the second
// shot [`SECOND`] asks for, so no colour written down here says what a panel is. And under its
// field the panel carries at least two bands of ink, the second's starting to the right of the
// first's: ref/sample.md's two headings are a level 1 and a level 2, and the step is the indent.
// Each band's ink is read against the band's own ground, because the selected row is drawn on the
// accent with its words in white: against the panel's ground the highlight would be the ink and
// its left edge the row's margin rather than its words.
//
// A still cannot read a word, and this rule does not pretend to: which row is selected, and what
// the rows say, is the Hand test's.
function outline(_spec, { dim, lit }) {
  const png = decodePng(dim);
  const page = decodePng(lit);
  const { w, h } = png;
  if (page.w !== w || page.h !== h) {
    return no(`the Outline shot is ${w}x${h} and its bare page is ${page.w}x${page.h}`);
  }
  const changed = changedBox(png, page);
  if (changed.right < changed.left) {
    return no(`the Outline shot is the bare page pixel for pixel: no panel stands over it`);
  }
  const inside = changed.left > 0 && changed.right < w - 1 && changed.top > 0 && changed.bottom < h - 1;
  if (!inside) {
    return no(`what differs from the bare page runs ${box(changed)} of a ${w}x${h} window: it reaches an edge, so it is not a panel standing over the page`);
  }
  const ground = groundOf(png, changed.left, changed.right + 1, changed.top, changed.bottom + 1);
  const panel = mostlyBox(png, ground, changed);
  if (panel.right < panel.left || panel.bottom < panel.top) {
    return no(`no rectangle of the ${hex(ground)} ground stands inside ${box(changed)}, where the shot differs from the bare page`);
  }
  // The corners are rounded: the ground's first column on the panel's top row is the radius, and
  // the bands are read inside it so a corner's curve is not a row.
  let radius = 0;
  while (panel.left + radius < panel.right && !is(png, panel.left + radius, panel.top, ground)) radius += 1;
  const scanned = { ...panel, left: panel.left + radius, right: panel.right - radius };
  const bands = bandsOf(png, scanned, ground).filter((band) => band.bottom - band.top + 1 >= OUTLINE_RULE);
  const rows = bands.map((band) => ({ ...band, left: inkLeft(png, scanned, ground, band) }));
  const field = rows.shift();
  const where = `the panel is ${hex(ground)}, ${box(panel)} of a ${w}x${h} window, its corners ${radius} px`;
  const firsts = rows.slice(0, 2).map((row, k) => `the ${k === 0 ? 'first' : 'second'}'s ink from x ${row.left}`);
  const read = `${rows.length} row${rows.length === 1 ? '' : 's'} under its field${firsts.length ? `, ${firsts.join(', ')}` : ''}`;
  const missed = [];
  if (!field) {
    missed.push(`it carries no band of ink at all`);
  } else if (rows.length < 2) {
    missed.push(`it carries ${rows.length} row${rows.length === 1 ? '' : 's'} under its field, and the sample's two headings are two`);
  } else if (rows[1].left <= rows[0].left) {
    missed.push(`the second row's ink starts at x ${rows[1].left}, not right of the first's at x ${rows[0].left}, so the level-2 heading is not stepped in under the level-1`);
  }
  return {
    ours: missed.length === 0,
    panel: [panel.left, panel.top, panel.right - panel.left + 1, panel.bottom - panel.top + 1],
    ground: hex(ground),
    rows: rows.map((row) => [row.left, row.top]),
    why: missed.length === 0 ? `${where}; ${read}` : `${where}; ${read} — ${missed.join(', and ')}`,
    secondary: [
      read,
      `the panel is where the shot differs from the bare page, and its ground is read off the pair rather than compared against a hex written down here`,
    ],
  };
}

// The bounding box of every pixel where `a` and `b` differ by [`OUTLINE_CHANGED`] on a channel.
function changedBox(a, b) {
  const { w, h } = a;
  let left = w;
  let right = -1;
  let top = h;
  let bottom = -1;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      let moved = false;
      for (let c = 0; c < 3 && !moved; c += 1) moved = Math.abs(at(a, x, y, c) - at(b, x, y, c)) >= OUTLINE_CHANGED;
      if (!moved) continue;
      if (x < left) left = x;
      if (x > right) right = x;
      if (y < top) top = y;
      if (y > bottom) bottom = y;
    }
  }
  return { left, right, top, bottom };
}

// The rectangle inside `box` whose rows and columns are mostly `ground`: the panel proper, with the
// shadow round it left out, and a page paper that happens to be the panel's colour left out too,
// since under the shadow it is not that colour.
function mostlyBox(png, ground, box) {
  const width = box.right - box.left + 1;
  let top = png.h;
  let bottom = -1;
  for (let y = box.top; y <= box.bottom; y += 1) {
    let held = 0;
    for (let x = box.left; x <= box.right; x += 1) if (is(png, x, y, ground)) held += 1;
    if (held * 2 <= width) continue;
    if (top === png.h) top = y;
    bottom = y;
  }
  if (bottom < top) return { left: png.w, right: -1, top: png.h, bottom: -1 };
  const height = bottom - top + 1;
  let left = png.w;
  let right = -1;
  for (let x = box.left; x <= box.right; x += 1) {
    let held = 0;
    for (let y = top; y <= bottom; y += 1) if (is(png, x, y, ground)) held += 1;
    if (held * 2 <= height) continue;
    if (left === png.w) left = x;
    right = x;
  }
  return { left, right, top, bottom };
}

// The first column of `band` carrying ink: over the accent a selected row is drawn on, inside
// that row's own span, where the band holds one; over the panel's `ground` everywhere else. A
// selected row's words are white, which on a light panel is the panel's own colour, so they are
// ink only against the accent under them.
function inkLeft(png, panel, ground, band) {
  const x0 = panel.left + 1;
  const x1 = panel.right;
  const highlight = highlightOf(png, ground, band, x0, x1);
  const [from, to, over] = highlight === null
    ? [x0, x1, ground]
    : [highlight.left, highlight.right + 1, highlight.rgb];
  for (let x = from; x < to; x += 1) {
    for (let y = band.top; y <= band.bottom; y += 1) if (inked(png, x, y, over)) return x;
  }
  return x1;
}

// The colour a selected row stands on and the columns it spans, or `null` for a row on the panel's
// own ground: the one colour other than `ground` that runs unbroken over more than half the
// panel's width on some row of the band, which words never do.
function highlightOf(png, ground, band, x0, x1) {
  for (let y = band.top; y <= band.bottom; y += 1) {
    let x = x0;
    while (x < x1) {
      const rgb = [at(png, x, y, 0), at(png, x, y, 1), at(png, x, y, 2)];
      let end = x;
      while (end < x1 && is(png, end, y, rgb)) end += 1;
      if (!sameRgb(rgb, ground) && (end - x) * 2 > x1 - x0) {
        let left = x0;
        while (left < x && !is(png, left, y, rgb)) left += 1;
        let right = x1 - 1;
        while (right > end && !is(png, right, y, rgb)) right -= 1;
        return { rgb, left, right };
      }
      x = end;
    }
  }
  return null;
}

// The dialog rectangle made by `ground`, where that colour is also the pane's surround.
//
// A row through the dialog is mostly its ground inside the middle half of the window; a row above
// or below it carries only the pane's gutter there, so the first and last such rows are its top
// and bottom. The sides are read along the dialog's own top margin — the [`DIALOG_EDGE`] rows
// under its top, which are ground clear across it, where a row through the controls is not: a
// column through the switches is less than half ground over the dialog's height, and a walk over
// the whole height would stop there. Out from the window's centre, each way, to the first column
// that is not mostly ground in that band: the dimmed Editor on the left, the page's white on the
// right. The pane's far gutter is that ground too, but the page stands between it and the dialog,
// so the walk never joins them; and a dialog that runs to either edge of the window reads as
// reaching it, which is what the caller's `inside` refuses.
function dialogBox(png, ground) {
  const { w, h } = png;
  const x0 = w >> 2;
  const x1 = w - x0;
  let top = h;
  let bottom = -1;
  for (let y = 0; y < h; y += 1) {
    let held = 0;
    for (let x = x0; x < x1; x += 1) if (is(png, x, y, ground)) held += 1;
    if (held * 2 <= x1 - x0) continue;
    if (top === h) top = y;
    bottom = y;
  }
  if (bottom < top) return { left: w, right: -1, top: h, bottom: -1 };

  const margin = Math.min(DIALOG_EDGE, bottom - top + 1);
  const mostly = (x) => {
    let held = 0;
    for (let y = top; y < top + margin; y += 1) if (is(png, x, y, ground)) held += 1;
    return held * 2 > margin;
  };
  const middle = w >> 1;
  if (!mostly(middle)) return { left: w, right: -1, top: h, bottom: -1 };
  let left = middle;
  while (left > 0 && mostly(left - 1)) left -= 1;
  let right = middle;
  while (right < w - 1 && mostly(right + 1)) right += 1;
  return { left, right, top, bottom };
}

// The bounding box of every pixel of `rgb`, or a box of nothing when the shot holds none.
//
// `x0` and `x1` narrow it to one column of the window, which is what a rule measuring the pane
// beside the Editor wants: the pages are the pane's, and a pixel of the same colour on the Editor's
// side of the divider is not part of them.
function extentOf(png, rgb, x0 = 0, x1 = png.w) {
  const { w, h } = png;
  let left = w;
  let right = -1;
  let top = h;
  let bottom = -1;
  for (let y = 0; y < h; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      if (!is(png, x, y, rgb)) continue;
      if (x < left) left = x;
      if (x > right) right = x;
      if (y < top) top = y;
      if (y > bottom) bottom = y;
    }
  }
  return { left, right, top, bottom };
}

// The runs of rows inside `box` that carry ink over `paper`, each as its own bounding box.
//
// A row of a dialog is a run of inked rows with air above and below it, so the runs are the rows the
// dialog is made of — a count that says whether an expander is open without saying where its rows
// are. The box's own outermost columns are left out, because a border drawn round the dialog is ink
// on every row and would make the whole of it one band.
function bandsOf(png, box, paper) {
  const x0 = box.left + 1;
  const x1 = box.right;
  const found = [];
  let open = null;
  for (let y = box.top; y <= box.bottom; y += 1) {
    let left = x1;
    let right = -1;
    for (let x = x0; x < x1; x += 1) {
      if (!inked(png, x, y, paper)) continue;
      if (x < left) left = x;
      if (x > right) right = x;
    }
    if (right < 0) {
      if (open) found.push(open);
      open = null;
      continue;
    }
    if (!open) open = { top: y, bottom: y, left, right };
    open.bottom = y;
    if (left < open.left) open.left = left;
    if (right > open.right) open.right = right;
  }
  if (open) found.push(open);
  return found;
}

// The colour a rectangle of the shot is mostly made of, as `[r, g, b]`.
//
// A pane is mostly paper — a page of prose is a few per cent ink — so the commonest colour in a
// rectangle wholly inside one pane is that pane's ground, whatever the palette says it should be.
//
// `not` leaves one colour out, which is how a rectangle holding two grounds gives up the second:
// a column of pages is mostly page, and what the rest of it is mostly made of is the surround the
// pages stand on. `null` comes back when the rectangle holds nothing but `not`.
function groundOf(png, x0, x1, y0, y1, not = null) {
  const skip = not === null ? -1 : (not[0] << 16) | (not[1] << 8) | not[2];
  const seen = new Map();
  for (let y = y0; y < y1; y += 1) {
    for (let x = x0; x < x1; x += 1) {
      const k = (at(png, x, y, 0) << 16) | (at(png, x, y, 1) << 8) | at(png, x, y, 2);
      if (k === skip) continue;
      seen.set(k, (seen.get(k) ?? 0) + 1);
    }
  }
  let best = 0;
  let held = -1;
  for (const [k, n] of seen) if (n > held) { best = k; held = n; }
  if (held < 0) return null;
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
  for (let c = 0; c < 3; c += 1) if (Math.abs(at(png, x, y, c) - paper[c]) >= PANE_INK) return true;
  return false;
}

function sameRgb(a, b) {
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
