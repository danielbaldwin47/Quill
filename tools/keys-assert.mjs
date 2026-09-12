// Everything `tools/gate keys` can decide without a window: the script a Piece is typed by, and
// the pixels it is judged on — decode the PNG, find the caret's bar and the ink beside it, say
// whether the bar stands where the writing left it, and count the rows a selection was painted on.
//
//   import { decodePng, readBar, readSelectionRows, readStatsBand, judgeBurst,
//     judgeSelectionRows, judgeStatsBarAccent, judgeMove, judgeStatsBarChanged,
//     resolveScript } from './keys-assert.mjs'
//
// `decodePng(buf)` returns `{ w, h, ch, data }` — not `width`/`height`/`channels`, which three
// one-off scripts guessed and crashed on in two days (2026-09-11).
//
// The split is `tools/bench-selftest.mjs` and `tools/bench-join.mjs`'s: the half with the compositor
// in it is `keys.mjs`, and nothing here reaches for one, so `tools/gate check` runs the selftest
// over this file on every commit with no display attached.
//
// WHY THE PIXELS AND NOT THE APP
//
// #108 placed the bar from GTK's `mark-set`, which is not emitted for the insert mark carried along
// by an insertion at its own position — so the bar never followed a keystroke. An app asked where
// its caret was would have answered x=0 in perfect good faith, because the app's own belief was the
// defect. Nothing here asks it. The bar is read off the glass, and so is the ink it is measured
// against.
//
// WHY THERE IS A DECODER HERE
//
// The repo has no PNG dependency and `tools/harness.mjs`'s `pngSize` reads the IHDR only. grim
// writes 8-bit RGB or RGBA, non-interlaced, so the whole decoder is the IDAT chunks through
// `node:zlib` and the five PNG filters undone by hand — about seventy lines, against a dependency
// the Gate would then have to keep. The selftest runs it against committed shots with no display
// and no compositor, which is the point of keeping this half free of the harness.
//
// WHY A CHROMA TEST AND NOT THE ACCENT ITSELF
//
// The bar is drawn with alpha, so no pixel of it is ever the accent: in the green fixture it is
// `#8adbfc`, the accent blended over paper, and where it crosses a glyph it is the accent blended
// over ink. What is constant is that the page is greyscale — paper, ink and the greys between them
// — and the bar is the one thing on it that leans blue. So the test is chroma, not equality.

import zlib from 'node:zlib';

// ---------- the colours a judged page is drawn in ----------

/// The light scheme's ink and paper, `#1c1c1c` on `#f9f9f9`, which is what the caret Piece's keys
/// script opens. Both are parameters because a dark script hands the other pair — `#cccccc` on
/// `#1a1a1a` — and the ink test is the one thing that has to know which way round they are.
export const INK = { r: 28, g: 28, b: 28 };
export const PAPER = { r: 249, g: 249, b: 249 };

/// The dark scheme's pair, for a script that opens `--theme dark`.
export const INK_DARK = { r: 204, g: 204, b: 204 };
export const PAPER_DARK = { r: 26, g: 26, b: 26 };

// How far a pixel's blue must run ahead of its red before it is the bar rather than a grey.
//
// `Role::Accent` is `#00b5ff` and is not a parameter, because `theme.rs` draws it on both grounds:
// the caret is the one instrument the writer watches, so it is the same blue either way. Every grey
// on the page has `b - r === 0`; the bar comes out 114 over paper and 113 over ink in the green
// fixture, so 40 sits far from both answers and needs no revisiting if the alpha is ever retuned.
export const CHROMA = 40;

// Where between paper and ink a pixel starts counting as a glyph. 0.45 puts it at 127.5 on the
// light scheme, a little to the ink's side of halfway, so an antialiased skirt is not read as ink
// and the rightmost ink column is the glyph's own edge.
const INK_SHARE = 0.45;

/// One glyph advance, in device pixels, taken from the shot itself rather than written down.
///
/// The tolerance has to be "one glyph advance *at the judged size*", and the size is not a constant
/// anyone can rely on: the green fixture was shot at an advance of 38.4 device pixels and the same
/// script at the judged default size 20 draws 24.0. A number measured off the fixture would have
/// been wrong by half for every real run.
///
/// So it is derived. The Faces are monospaced, so `n` characters on one line span exactly `n - 1`
/// advances from the first glyph's ink to the last glyph's ink, *plus* the last glyph's own ink
/// width. Dividing by `n - 1` therefore always over-states the advance a little, which is the right
/// direction for something used as an upper bound. It comes out at 40.4 and 39.2 on the green
/// fixture's two shots (true 38.4) and 25.3 and 24.5 on the red one's (true 24.0).
export function glyphAdvance({ inkLeft, inkRight }, chars) {
  if (chars < 2 || inkLeft === null || inkRight === null) return null;
  return (inkRight - inkLeft) / (chars - 1);
}

// ---------- the PNG ----------

const SIGNATURE = 0x89504e47;

/// The pixels of a grim shot: `{ w, h, ch, data }`, `data` being `w * h * ch` bytes, RGB or RGBA.
export function decodePng(buf) {
  if (buf.length < 24 || buf.readUInt32BE(0) !== SIGNATURE) throw new Error('not a PNG');
  let off = 8;
  let w = 0;
  let h = 0;
  let depth = 0;
  let ctype = 0;
  let interlace = 0;
  const idat = [];
  while (off + 8 <= buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('latin1', off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === 'IHDR') {
      w = data.readUInt32BE(0);
      h = data.readUInt32BE(4);
      depth = data[8];
      ctype = data[9];
      interlace = data[12];
    } else if (type === 'IDAT') {
      idat.push(data);
    } else if (type === 'IEND') {
      break;
    }
    off += 12 + len;
  }
  // Everything grim writes is one of these two. Anything else is a shot from somewhere else, and
  // saying so is better than measuring a bar out of misread bytes.
  if (depth !== 8 || (ctype !== 2 && ctype !== 6) || interlace !== 0) {
    throw new Error(`a PNG this decoder does not read (depth ${depth}, colour type ${ctype}, `
      + `interlace ${interlace}); grim writes 8-bit RGB or RGBA, non-interlaced`);
  }
  if (!idat.length) throw new Error('a PNG with no IDAT');

  const ch = ctype === 2 ? 3 : 4;
  const stride = w * ch;
  const raw = zlib.inflateSync(Buffer.concat(idat));
  if (raw.length < h * (stride + 1)) throw new Error('a PNG whose IDAT is shorter than its IHDR');
  const out = Buffer.alloc(h * stride);
  const zero = Buffer.alloc(stride);
  let p = 0;
  for (let y = 0; y < h; y += 1) {
    const filter = raw[p];
    p += 1;
    const line = raw.subarray(p, p + stride);
    p += stride;
    const cur = out.subarray(y * stride, y * stride + stride);
    const prev = y ? out.subarray((y - 1) * stride, y * stride) : zero;
    for (let x = 0; x < stride; x += 1) {
      const a = x >= ch ? cur[x - ch] : 0;
      const b = prev[x];
      const c = x >= ch ? prev[x - ch] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const guess = a + b - c;
        const da = Math.abs(guess - a);
        const db = Math.abs(guess - b);
        const dc = Math.abs(guess - c);
        v += (da <= db && da <= dc) ? a : (db <= dc ? b : c);
      } else if (filter !== 0) {
        throw new Error(`a PNG row filtered ${filter}, which is not one of the five`);
      }
      cur[x] = v & 0xff;
    }
  }
  return { w, h, ch, data: out };
}

// ---------- the bar, and the ink it stands after ----------

export const lum = ({ r, g, b }) => (r + g + b) / 3;

// One pixel, in the two terms both scans below ask it for. `chroma` leans one way only, because
// the accent leans one way only: `Role::Accent` is a single colour in `theme.rs` — the same blue on
// paper and on the dark ground — so there is no second direction to carry.
export function pixel(png, x, y) {
  const i = (y * png.w + x) * png.ch;
  const r = png.data[i];
  const g = png.data[i + 1];
  const b = png.data[i + 2];
  return { g, b, chroma: b - r, lum: (r + g + b) / 3 };
}

// The one test for "this pixel is the accent and not a grey", shared by the bar and the selection.
//
// One predicate covers both because both are `Role::Accent`: the bar is it at full alpha and the
// selection's fill is it at .22, which over paper comes out `#c2eafa` — chroma 56, on the same
// side of `CHROMA` as the bar's own 113 and 114.
export const leansBlue = (p) => p.chroma >= CHROMA && p.b > p.g;

/// Where the caret's bar is, and where the ink on its rows ends.
///
/// `null` for `bar` when nothing on the page leans blue — which is not the same as a bar in the
/// wrong place, and is reported as its own thing so a shot caught in the blink's dark half is
/// never read as a defect.
export function readBar(png, { ink = INK, paper = PAPER } = {}) {
  const edge = lum(ink) + (lum(paper) - lum(ink)) * INK_SHARE;
  const inkIsDarker = lum(paper) > lum(ink);
  const cols = new Set();
  const rows = new Set();
  let n = 0;
  for (let y = 0; y < png.h; y += 1) {
    for (let x = 0; x < png.w; x += 1) {
      if (leansBlue(pixel(png, x, y))) {
        cols.add(x);
        rows.add(y);
        n += 1;
      }
    }
  }
  if (!n) return { bar: null, pixels: 0 };

  const cs = [...cols].sort((a, b) => a - b);
  const rs = [...rows].sort((a, b) => a - b);
  const bar = { left: cs[0], right: cs[cs.length - 1], top: rs[0], bottom: rs[rs.length - 1] };
  // One bar, in one piece. Two runs of blue means either a second caret or something on the page
  // this test was never meant to find, and either way the measurement below would be meaningless.
  const oneRun = cs.length === bar.right - bar.left + 1 && rs.length === bar.bottom - bar.top + 1;

  let inkLeft = null;
  let inkRight = null;
  for (let y = bar.top; y <= bar.bottom; y += 1) {
    for (let x = 0; x < png.w; x += 1) {
      const p = pixel(png, x, y);
      if (leansBlue(p)) continue;
      if (inkIsDarker ? p.lum < edge : p.lum > edge) {
        if (inkLeft === null || x < inkLeft) inkLeft = x;
        if (inkRight === null || x > inkRight) inkRight = x;
      }
    }
  }
  return { bar, oneRun, pixels: n, inkLeft, inkRight };
}

// ---------- the selection, as the rows it was painted on ----------

/// The selection's row bands, top to bottom: `{ top, bottom, left, right }` per display row the
/// selection colour was painted on, and how many pixels carry it.
///
/// WHY THE BANDS ARE TOLD APART BY THEIR ENDS AND NOT BY A GAP
///
/// There is no gap. `Editor::band` gives every row the full pitch and `Editor::selection` stacks
/// one fill per display row, so row *k*'s fill ends on the device pixel row *k+1*'s begins — a scan
/// for empty rows between them would answer "one band" for a selection of any height, and #146's
/// missing last row would not move that number. What does differ is where each row's fill starts
/// and stops: a selection is a fill of the container, so only its two ends are measured off glyphs
/// — the first row starts at the anchor and the last stops at the focus, while every row between
/// them runs the container's whole width (#168). So a band here is a run of consecutive scanlines
/// whose selection colour starts and ends in the same two columns, and the three shapes a
/// multi-row selection has are three bands.
///
/// The seam between two bands costs nothing: where the two fills share a device pixel row it
/// carries both, so its span is the union of theirs — and an interior row spans the container,
/// which contains the row above it and the row below it, so that union is the interior row's own
/// span and the seam joins its band rather than standing as a third.
export function readSelectionRows(png) {
  const bands = [];
  let pixels = 0;
  for (let y = 0; y < png.h; y += 1) {
    let left = null;
    let right = null;
    for (let x = 0; x < png.w; x += 1) {
      if (!leansBlue(pixel(png, x, y))) continue;
      if (left === null) left = x;
      right = x;
      pixels += 1;
    }
    if (left === null) continue;
    const open = bands[bands.length - 1];
    if (open && open.bottom === y - 1 && open.left === left && open.right === right) {
      open.bottom = y;
    } else {
      bands.push({ top: y, bottom: y, left, right });
    }
  }
  return { bands, pixels };
}

// ---------- the stats bar, as the band the chrome reserves at the foot ----------

// The stats bar's height in logical pixels, as the chrome lays it out. Measured off the committed
// shots: where a Document scrolls, the chrome draws a full-width `#dfdfdf` separator along the
// bar's top edge, at device row 1748 of an 1800-row window in ours and in the Parity oracle's
// alike — 52 device rows at `defaults.scale` 2, which is also the rectangle `stats-bar-*.png` were
// cut at. The height is written in logical pixels and multiplied by the scale the shot was taken
// at, so it holds at every scale rather than at the one it was measured from.
const BAR_HEIGHT = 26;

// The scale a shot with nothing to say about it was taken at: `defaults.scale` in
// `shots/oracle/states.json`, which every judged shot and both fixture crops use. `keys.mjs`
// passes the running state's own scale rather than leaning on this.
const SCALE = 2;

// How far a pixel's luminance must run from the paper's before it is not paper. The bar's ink is
// far past it; 6 clears the dither a scaled shot can leave on a flat ground, and the 2 luminance
// between our `#f7f7f7` paper and the oracle's `#f9f9f9`, without reaching either.
export const PAPER_MARGIN = 6;

/// The stats bar's band: the rows the chrome reserves for the bar at the foot of the window,
/// `{ top, bottom }` — or `null` when the shot has no bar in it.
///
/// WHY A LOGICAL HEIGHT AND NOT THE SEPARATOR
///
/// #391 took the band from the full-width `#dfdfdf` rule along the bar's top edge, on the reading
/// that the chrome draws it and so it moves with the chrome. It does not: the rule is a *scroll*
/// separator, drawn only while the Document overflows its view. The oracle's `bars` state has it
/// and the oracle's `selection` state, over the shorter passage, has none — and neither does ours,
/// in any state that does not scroll. `tools/gate keys chrome` types on `ref/short.md`, which never
/// scrolls, so both of #391's rules refused before asserting anything and its selftest could not
/// catch it: every fixture it pinned was cut from a scrolling shot.
///
/// The band has to be tight, because the accent is not rare on this page: a selection's fill leans
/// the same blue the `Selection` label is drawn in — `leansBlue` covers both, and one predicate for
/// both is the point of having it — so a band generous enough to be safe at another window size
/// would answer yes to the selection the burst had just made and say nothing about the bar at all.
/// A *device*-pixel height would be a pin on the chrome's layout at one scale and wrong at the
/// next, which is what #391 rightly refused; a *logical* height taken times the shot's own scale is
/// the same measurement written so that it survives the scale changing. At scale 2 it is the rows
/// the separator marks when there is one, and the rows the bar occupies when there is not.
///
/// WHAT `null` MEANS
///
/// Two readings, and neither can be told from "a bar showing nothing" by geometry alone, so both
/// are refusals rather than passes. The band is **all paper**: the chrome is off over a Document
/// that does not reach the foot, or the bar is there with nothing checked. Ink reaches the
/// window's **last row**: the bar never draws there — it keeps a clear 20 rows of padding beneath
/// its text in every committed shot — but a page with the chrome off is clipped by the window edge
/// and does, which is what every caret, focus and markup shot looks like from here.
///
/// What neither test catches is the chrome off over a Document whose last line happens to stop
/// inside the band without touching the edge. No committed shot does that, and the one script that
/// names these rules opens with `"chrome": "on"`, but it is the gap in what pixels can say.
export function readStatsBand(png, { paper = PAPER, scale = SCALE } = {}) {
  const ground = lum(paper);
  const rows = Math.round(BAR_HEIGHT * scale);
  // A crop no taller than the bar is the bar, already cut from a shot — which is what the fixtures
  // are, and what lets the same finder read them and the windows they came from.
  const top = png.h <= rows ? 0 : png.h - rows;
  const inked = (y) => {
    for (let x = 0; x < png.w; x += 1) {
      if (Math.abs(pixel(png, x, y).lum - ground) > PAPER_MARGIN) return true;
    }
    return false;
  };
  if (inked(png.h - 1)) return null;
  for (let y = top; y < png.h - 1; y += 1) {
    if (inked(y)) return { top, bottom: png.h - 1 };
  }
  return null;
}

// ---------- the assertions ----------

/// After a burst, the bar stands just right of the ink.
///
/// `chars` is how many characters are on the line the burst has just written, which is what the
/// advance is derived from. `read` is passed in when the caller has already taken it off the shot.
export function judgeBurst(png, { chars, colours, read = readBar(png, colours) } = {}) {
  if (!read.bar) {
    return { pass: false, read, said: 'no caret bar on the page (nothing drawn in the accent)' };
  }
  if (!read.oneRun) {
    return {
      pass: false,
      read,
      said: `the accent is not one run (x ${read.bar.left}..${read.bar.right}, `
        + `y ${read.bar.top}..${read.bar.bottom}, ${read.pixels} px)`,
    };
  }
  if (read.inkRight === null) {
    return {
      pass: false,
      read,
      said: `no ink on the bar's rows (y ${read.bar.top}..${read.bar.bottom}), so there is `
        + 'nothing the bar could be following',
    };
  }
  const advance = glyphAdvance(read, chars);
  if (advance === null || !(advance > 0)) {
    return { pass: false, read, said: `${chars} characters is too few to measure an advance from` };
  }
  const gap = read.bar.left - read.inkRight;
  // Not `> 0`: the bar standing exactly on the last glyph's rightmost column is the bar in the
  // right place, and no reading of the pixels can tell that apart from a zero right side bearing.
  const pass = gap >= 0 && gap < advance;
  return {
    pass,
    read,
    gap,
    advance,
    said: `bar.left ${read.bar.left}, ink.right ${read.inkRight}, gap ${gap} px; expected `
      + `0 <= gap < ${advance.toFixed(1)} (one glyph advance over ${chars} characters)`,
  };
}

/// After a burst that ends in a selection, every row of it was painted — the bottom one included.
///
/// `rows` is how many display rows the burst's selection covers, written down in the script beside
/// the text that makes them. Counting the bands rather than looking at the last one is what makes
/// this catch an off-by-one at *either* end: a walk that stops a row early and one that starts a
/// row late both come out one band short, and neither is visible in a still of the middle of a
/// Document — which is why #146 survived six rounds and eighteen critics.
///
/// It reads the page itself rather than taking the `read` the run carries from burst to burst:
/// that one is [`readBar`]'s, and a selection is not a bar.
export function judgeSelectionRows(png, { rows } = {}) {
  const read = readSelectionRows(png);
  if (!(rows > 0)) {
    return {
      pass: false,
      read,
      said: `a burst asserting selection-rows has to say how many rows it selects, and this one `
        + `says ${JSON.stringify(rows)}`,
    };
  }
  if (!read.bands.length) {
    return {
      pass: false,
      read,
      said: 'nothing on the page is drawn in the selection colour, so no row of the selection '
        + 'was painted at all',
    };
  }
  const where = read.bands
    .map((b) => `y ${b.top}..${b.bottom} x ${b.left}..${b.right}`)
    .join('; ');
  return {
    pass: read.bands.length === rows,
    read,
    said: `${read.bands.length} row bands in the selection colour, expected ${rows} (${where})`,
  };
}

// How far the two edges of a container measured off the glass may miss each other by, in device
// pixels. The container is centred in the view, so its left edge and the pixel past its right one
// sum to the view's width — but `quill_engine::typography::column` centres it by halving what the
// container leaves over and rounding that, so a view leaving an odd number over seats the container
// half a logical pixel off centre and its two edges sum to one logical pixel either side of the
// view's width. Nothing else moves them: an integer logical edge times a scale of 2 is already an
// integer device one, so the snap to the glass adds none of its own. One logical pixel at scale 2
// is two device pixels, and that is the whole of the play.
//
// A band's `right` is the last pixel the fill painted and the container's right edge is the one
// after it, so every sum below is `left + right + 1`.
const SPREAD = 2;

// The two guards both container assertions open with: the row count the burst wrote down, and the
// bands actually painted. `least` is the fewest rows the shape needs one of — three for a row
// between the two ends, two for a newline held below the first row. `said` is null when the bands
// are the ones to measure.
function openBands(png, rows, { assertion, least, wants }) {
  const read = readSelectionRows(png);
  const where = read.bands.map((b) => `x ${b.left}..${b.right}`).join('; ');
  if (!(rows >= least)) {
    return {
      read,
      where,
      said: `a burst asserting ${assertion} has to say how many rows it selects, and ${wants}, not `
        + `${JSON.stringify(rows)}`,
    };
  }
  if (read.bands.length !== rows) {
    return {
      read,
      where,
      said: `${read.bands.length} row bands in the selection colour, expected ${rows} (${where})`,
    };
  }
  return { read, where, said: null };
}

// Are these two columns the container's own edges? The one absolute measurement either assertion
// makes, and what tells a band that reached the container from one that merely reached the widest
// ink on the page.
function centredOnTheView(png, { left, right, where, what }) {
  const edges = left + right + 1;
  return {
    pass: Math.abs(edges - png.w) <= SPREAD,
    said: `${what} x ${left}..${right}, whose edges sum to ${edges} in a ${png.w} px view; a `
      + `container centred in it sums to ${png.w} within ${SPREAD} (${where})`,
  };
}

/// After a burst that ends in a multi-row selection, the rows between the first and the last fill
/// the container edge to edge.
///
/// A selection is a fill of the container rather than of the ink it covers (`docs/design.md`
/// row Multi-row fill, the Design oracle's `09-dark`), so only its two ends are measured off glyphs:
/// the first row starts at the anchor, the last stops at the focus, and every row between them
/// runs the container's whole width. That is read off the bands alone, which is all a still of the
/// glass carries — an interior band starts where the last row starts and stops where the first row
/// stops, and reaches past both: left of the anchor into the gutter, and right of the focus. A
/// painter filling each row to its own ink cannot do either, however many rows it paints.
///
/// The container is centred in the view, so its two edges sum to the view's width. That is the one
/// absolute number here, and it is what says the band reached the container's edges rather than
/// merely the widest ink on the page.
export function judgeSelectionFill(png, { rows } = {}) {
  const { read, where, said } = openBands(png, rows, {
    assertion: 'selection-container-wide',
    least: 3,
    wants: 'an interior row wants three or more',
  });
  if (said) return { pass: false, read, said };
  const first = read.bands[0];
  const last = read.bands[rows - 1];
  const astray = read.bands.slice(1, -1).find(
    (b) => b.left !== last.left || b.right !== first.right
      || b.left >= first.left || b.right <= last.right,
  );
  if (astray) {
    return {
      pass: false,
      read,
      said: `an interior row is filled x ${astray.left}..${astray.right}, where the container is `
        + `x ${last.left}..${first.right} — the first row runs from the anchor at ${first.left} `
        + `and the last to the focus at ${last.right} (${where})`,
    };
  }
  const answer = centredOnTheView(png, {
    left: last.left, right: first.right, where, what: 'the interior rows fill',
  });
  return { ...answer, read };
}

/// After a burst whose selection ends past a row's newline, that row is filled to the container's
/// right edge.
///
/// A newline has no advance to highlight and is a held character all the same. The oracle drew a
/// half-em stub past the last glyph for it and the port inherited one; the Design oracle fills the
/// row to the container's right edge instead (`docs/design.md` row Held newline,
/// `10-newline-only`),
/// which is the same edge every row the selection runs past reaches.
///
/// The burst holds the newline of its **last** row, so that row was also entered from above and
/// spans the whole container: it starts left of the first row's anchor and stops where the first
/// row stops, and those two edges sum to the view's width. A stub would stop a half em past ink
/// that ends nowhere near either.
export function judgeSelectionNewline(png, { rows } = {}) {
  const { read, where, said } = openBands(png, rows, {
    assertion: 'selection-newline-to-edge',
    least: 2,
    wants: 'a held newline below the first row wants two or more',
  });
  if (said) return { pass: false, read, said };
  const first = read.bands[0];
  const last = read.bands[rows - 1];
  if (last.right !== first.right || last.left >= first.left) {
    return {
      pass: false,
      read,
      said: `the row holding the newline is filled x ${last.left}..${last.right}, and the row `
        + `above it x ${first.left}..${first.right} — a row the selection runs past reaches the `
        + `container's right edge, and one entered from above starts at its left (${where})`,
    };
  }
  const answer = centredOnTheView(png, {
    left: last.left, right: last.right, where, what: "the newline's row fills",
  });
  return { ...answer, read };
}

/// After a burst, the stats bar is showing the accent — or is showing none of it.
///
/// The burst says which with `accent`, because both readings are assertions the script makes: the
/// burst that selects asserts the `Selection` label is there, and the one that clears the selection
/// asserts it is gone. A burst that says neither is refused rather than passed, as a burst
/// asserting rows without a count is.
///
/// Only the band is looked at, never the page. The accent in the bar is one word of a label and
/// the accent on the page is the fill of the very selection that put it there, so a page-wide count
/// would answer yes to the selection alone and never once to the bar — which would leave the rule
/// green on a bar that never noticed.
///
/// Text is not read. Whether the numbers beside the label are the selection's is
/// `stats-bar-changed`'s question, and neither rule ever asks a crop what it says.
export function judgeStatsBarAccent(png, { accent, colours, scale } = {}) {
  if (typeof accent !== 'boolean') {
    return {
      pass: false,
      said: 'a burst asserting stats-bar-accent has to say whether it expects the accent, and '
        + `this one says ${JSON.stringify(accent)}`,
    };
  }
  const band = readStatsBand(png, { ...colours, scale });
  if (!band) {
    return {
      pass: false,
      said: 'no stats bar on the page: the band the chrome reserves at the foot is bare, or the '
        + 'page is drawn into the window\'s last row, which the bar never is',
    };
  }
  let pixels = 0;
  let left = null;
  let right = null;
  for (let y = band.top; y <= band.bottom; y += 1) {
    for (let x = 0; x < png.w; x += 1) {
      if (!leansBlue(pixel(png, x, y))) continue;
      pixels += 1;
      if (left === null || x < left) left = x;
      if (right === null || x > right) right = x;
    }
  }
  return {
    pass: accent ? pixels > 0 : pixels === 0,
    band,
    pixels,
    said: `${pixels} px of the accent in the bar's band (y ${band.top}..${band.bottom}`
      + `${pixels ? `, x ${left}..${right}` : ''}); expected ${accent ? 'the accent' : 'none of it'}`,
  };
}

/// Between the bursts, the bar moved right.
export function judgeMove(before, after) {
  if (!before.bar || !after.bar) {
    return { pass: false, said: 'a burst with no bar to compare' };
  }
  const pass = after.bar.left > before.bar.left;
  return {
    pass,
    said: `bar.left ${before.bar.left} then ${after.bar.left}; expected the second to be greater`,
  };
}

/// Between the bursts, the stats bar's band is not the band it was.
///
/// What a selection does to the bar is text — a label and the numbers beside it — and text is the
/// one thing a crop is never read for here. So what is asserted is that the band changed: a bar
/// that ignored the selection leaves every pixel of it where it was, and one that answered cannot.
/// Paired with `stats-bar-accent`, which says the label appeared, that is as close to "the numbers
/// followed" as ink alone gets, and the two together are what a keys run has that a still does not.
///
/// It takes the two pages rather than the two bars, because a burst that ends in a selection puts
/// the caret out entirely (ADR 0014) and so leaves no bar to compare — and that pair is the whole
/// reason this rule exists. [`BETWEEN_BURSTS`] is what carries the difference.
export function judgeStatsBarChanged(before, after, { colours, scale } = {}) {
  const a = readStatsBand(before, { ...colours, scale });
  const b = readStatsBand(after, { ...colours, scale });
  if (!a || !b) {
    const which = !a && !b ? 'neither page' : (a ? 'the second page' : 'the first page');
    return { pass: false, said: `no stats bar on ${which}, so there is no band to compare` };
  }
  // A band that moved or changed shape has changed by that alone, and comparing two rectangles of
  // different sizes pixel for pixel would be meaningless rather than merely wrong.
  if (before.w !== after.w || before.h !== after.h || a.top !== b.top) {
    return {
      pass: true,
      said: `the band itself moved: ${before.w}x${before.h} px at y ${a.top}..${a.bottom}, then `
        + `${after.w}x${after.h} px at y ${b.top}..${b.bottom}`,
    };
  }
  let differing = 0;
  for (let y = a.top; y <= a.bottom; y += 1) {
    for (let x = 0; x < before.w; x += 1) {
      const i = (y * before.w + x) * before.ch;
      const j = (y * after.w + x) * after.ch;
      if (before.data[i] !== after.data[j]
        || before.data[i + 1] !== after.data[j + 1]
        || before.data[i + 2] !== after.data[j + 2]) differing += 1;
    }
  }
  const total = before.w * (a.bottom - a.top + 1);
  return {
    pass: differing > 0,
    differing,
    said: `${differing} of ${total} px of the bar's band differ (y ${a.top}..${a.bottom})`,
  };
}

// ---------- the Spell check wave, under the word the caret stands after ----------

/// `Role::Spell` on the two grounds, as `quill-engine/src/theme.rs` pins it:
/// `(Scheme::Light, Role::Spell, "#e5372b")` and `(Scheme::Dark, Role::Spell, "#e5534b")`.
///
/// Provisional until the capture #400 measures the mark, like the Role itself; the day the theme's
/// two lines move, these two move with them.
export const SPELL = { r: 0xe5, g: 0x37, b: 0x2b };
export const SPELL_DARK = { r: 0xe5, g: 0x53, b: 0x4b };

// How far a pixel's `(r - g) / (r - b)` may stray from the Role's own before it is not the wave.
const SPELL_INK_HUE = 0.2;

// The gap, as a share of the caret bar's height, that ends a word when walking left from the bar.
// Measured off `tools/keys-fixture/spell-*.png` at the judged defaults: the bar is 74 device px
// tall, the widest gap between two letters of `comittee` is 8 px and one advance is about 25 px,
// so a space's gap is at least an advance. A quarter of the bar, 18 px, sits between the two.
const WORD_GAP = 0.25;

// A pixel's three channels, which `pixel` does not carry red of.
function rgb(png, x, y) {
  const i = (y * png.w + x) * png.ch;
  return { r: png.data[i], g: png.data[i + 1], b: png.data[i + 2] };
}

/// Whether the pixel at (x, y) is `spell`'s ink, at any strength.
///
/// The wave is antialiased onto a neutral grey, and a blend with a grey moves every channel toward
/// it by the same share: `r - g` and `r - b` shrink together, so their ratio is the Role's own at
/// every strength and a chroma floor is all that tells a faint skirt from the paper. No grey and no
/// accent pixel leans red, so nothing else on a keys page answers it.
///
/// `chroma` is how far red must lead green and blue, and `hue` how far the ratio may stray; a
/// caller reading a fainter wave than a keys page carries passes its own.
export function isSpellInk(png, x, y, spell = SPELL, { chroma = CHROMA, hue = SPELL_INK_HUE } = {}) {
  const { r, g, b } = rgb(png, x, y);
  if (r - Math.max(g, b) < chroma) return false;
  const want = (spell.r - spell.g) / (spell.r - spell.b);
  return Math.abs((r - g) / (r - b) - want) <= hue;
}

/// The ink of the word the caret's bar stands after, on the bar's rows: `{ left, right }` in device
/// columns, or `null` when there is no ink left of the bar.
///
/// Walks left from the bar over the space the caret may sit after, then over the word's ink until
/// a gap as wide as a space. The gap is a share of the bar's height rather than a glyph advance,
/// because the row is a wrapped paragraph and no advance can be derived from it. A red pixel is not
/// ink here, though it is darker than the edge: it is the thing being looked for.
export function readWordBeforeBar(png, read, { ink = INK, paper = PAPER } = {}) {
  const { bar } = read;
  const edge = lum(ink) + (lum(paper) - lum(ink)) * INK_SHARE;
  const inkIsDarker = lum(paper) > lum(ink);
  const inked = (x) => {
    for (let y = bar.top; y <= bar.bottom; y += 1) {
      const p = rgb(png, x, y);
      if (Math.abs(p.r - p.g) >= CHROMA || Math.abs(p.b - p.r) >= CHROMA) continue;
      if (inkIsDarker ? lum(p) < edge : lum(p) > edge) return true;
    }
    return false;
  };
  const gap = Math.max(1, Math.round((bar.bottom - bar.top + 1) * WORD_GAP));
  let x = bar.left - 1;
  while (x >= 0 && !inked(x)) x -= 1;
  if (x < 0) return null;
  const right = x;
  let left = x;
  let run = 0;
  for (x -= 1; x >= 0; x -= 1) {
    if (inked(x)) {
      left = x;
      run = 0;
    } else {
      run += 1;
      if (run >= gap) break;
    }
  }
  return { left, right };
}

/// After a burst, the word the caret stands after wears the Spell wave — or wears none of it.
///
/// The burst says which with `wave`, as `stats-bar-accent` is told with `accent`: typing a
/// misspelling asserts no wave while the caret is still in the word, and the space after it
/// asserts the wave. Only the columns of that one word are looked at, never its row: the row is a
/// wrapped paragraph that can hold another misspelling left of it, and the rule is about the word
/// being typed.
export function judgeSpellWave(png, { wave, colours, read } = {}) {
  if (typeof wave !== 'boolean') {
    return {
      pass: false,
      said: 'a burst asserting spell-wave has to say whether it expects the wave, and this one '
        + `says ${JSON.stringify(wave)}`,
    };
  }
  const reading = read || readBar(png, colours);
  if (!reading.bar) return { pass: false, said: 'no caret bar on the page to find the word by' };
  const word = readWordBeforeBar(png, reading, colours);
  if (!word) return { pass: false, said: 'no ink left of the caret bar, so no word to look under' };
  const spell = colours && colours.paper === PAPER_DARK ? SPELL_DARK : SPELL;
  // The bar's rows are the row's whole pitch, and the wave is drawn inside them: y 590..593 under a
  // bar at 536..609 in `spell-space.png`'s page.
  const { bar } = reading;
  let pixels = 0;
  let top = null;
  let bottom = null;
  for (let y = bar.top; y <= bar.bottom; y += 1) {
    for (let x = word.left; x <= word.right; x += 1) {
      if (!isSpellInk(png, x, y, spell)) continue;
      pixels += 1;
      if (top === null) top = y;
      bottom = y;
    }
  }
  return {
    pass: wave ? pixels > 0 : pixels === 0,
    word,
    pixels,
    said: `${pixels} px of the spell Role under the word at x ${word.left}..${word.right} `
      + `(y ${bar.top}..${bar.bottom}${pixels ? `, wave at y ${top}..${bottom}` : ''}); `
      + `expected ${wave ? 'the wave' : 'none of it'}`,
  };
}

// ---------- the script a Piece is typed by ----------
//
// The assertions a script may name, by the phrase the failing line prints. They live beside the
// functions they name so that adding one is an edit to this file and to the script in states.json,
// and to nothing else.

export const AFTER_BURST = {
  'bar-after-ink': judgeBurst,
  'selection-rows': judgeSelectionRows,
  'selection-container-wide': judgeSelectionFill,
  'selection-newline-to-edge': judgeSelectionNewline,
  'stats-bar-accent': judgeStatsBarAccent,
  'spell-wave': judgeSpellWave,
};

/// The between-bursts assertions, each with what it reads.
///
/// `reads` is `'bar'` for a rule that compares the two [`readBar`] readings and `'page'` for one
/// that compares the two pages. The difference is not bookkeeping: a burst that ends in a selection
/// puts the caret out (ADR 0014), so a `'bar'` rule has nothing to compare across such a pair and
/// is passed over with a line in the log, while a `'page'` rule is asked for exactly those pairs.
/// Until `stats-bar-changed` there was one rule and the skip was the loop's; it is the rule's now.
export const BETWEEN_BURSTS = {
  'bar-moved-right': { judge: judgeMove, reads: 'bar' },
  'stats-bar-changed': { judge: judgeStatsBarChanged, reads: 'page' },
};

/// How a burst's shot is waited for, by the name a burst may say in `settle`.
///
/// `caret` is the two typing bursts': a page with the caret on it, re-shot until the blink is
/// caught lit. `still` is a page with no caret on it — what a burst that ends in a selection
/// leaves, because a selection puts the caret out entirely (ADR 0014) — where two agreeing
/// captures are the whole rule, and a run that waited for a lit bar would spend its tries and then
/// refuse a page that is exactly right.
///
/// The names are here rather than in `keys.mjs` so that a script naming a settle rule the command
/// has not got is refused by `resolveScript`, with no window open, in the same breath as an
/// assertion it has not got. `keys.mjs` holds the functions.
export const SETTLES = ['caret', 'still'];

/// Where the scripts are, said once so the command and its error messages agree.
export const STATES = 'shots/oracle/states.json';

/// The bursts and assertions listed for a Piece, with the judged defaults filled in around the
/// state it opens.
///
/// Pure, and here rather than in `keys.mjs`, so that the selftest can hold the scripts to their
/// shape without importing the half that opens a window.
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

  // `chars` is how many characters have been typed by the end of the burst. It is written down in
  // the script and checked against the running total here, so that the day a script's arithmetic
  // and its text disagree the file says which one moved instead of quietly shifting a tolerance.
  // A burst's chords are not in it: `keys` spells what the keyboard does, and `Control+a` puts no
  // character on the page.
  //
  // `bar-after-ink` reads it as how many characters stand on the line, which it is only while the
  // script stays on one line — the two typing bursts do, and the burst that presses Enter asserts
  // the selection instead. A burst wanting both would need a count of its own, and this check is
  // where it would be given one.
  let running = 0;
  for (const burst of bursts) {
    running += [...(burst.text || '')].length;
    if (burst.chars !== running) {
      throw new Error(`${piece}: burst ${burst.name} says chars ${burst.chars}, but ${running} `
        + 'characters have been typed by the end of it');
    }
    if (burst.settle !== undefined && !SETTLES.includes(burst.settle)) {
      throw new Error(`${piece}: burst ${burst.name} settles ${burst.settle} `
        + `(this command knows ${SETTLES.join(', ')})`);
    }
  }
  for (const [table, names, what] of [
    [AFTER_BURST, bursts.flatMap((b) => b.assert || []), 'assertion'],
    [BETWEEN_BURSTS, script.between || [], 'between-bursts assertion'],
  ]) {
    for (const name of names) {
      if (!table[name]) {
        throw new Error(`${piece}: no ${what} called ${name} `
          + `(this command knows ${Object.keys(table).join(', ')})`);
      }
    }
  }
  return { ...script, bursts, flags: { ...states.defaults, ...(script.state || {}) } };
}
