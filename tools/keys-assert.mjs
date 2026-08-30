// What a typed shot is judged on: decode the PNG, find the caret's bar and the ink beside it, and
// say whether the bar stands where the writing left it — `tools/gate keys`'s pure half.
//
//   import { decodePng, readBar, judgeBurst, judgeMove } from './keys-assert.mjs'
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

/// `Role::Accent` in `quill-engine/src/theme.rs`: `#00b5ff`, the same blue on both grounds.
export const ACCENT = { r: 0, g: 181, b: 255 };

/// The light scheme's ink and paper, `#1c1c1c` on `#f9f9f9`, which is what the caret Piece's keys
/// script opens. Both are parameters because a dark script would hand the other pair.
export const INK = { r: 28, g: 28, b: 28 };
export const PAPER = { r: 249, g: 249, b: 249 };

// How far a pixel's blue must run ahead of its red before it is the bar rather than a grey. Every
// grey on the page has `b - r === 0`; the bar is 114 over paper and 113 over ink in the green
// fixture, so 40 sits far from both answers and needs no revisiting if the alpha is ever retuned.
const CHROMA = 40;

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

const lum = ({ r, g, b }) => (r + g + b) / 3;

/// Where the caret's bar is, and where the ink on its rows ends.
///
/// `null` for `bar` when nothing on the page leans blue — which is not the same as a bar in the
/// wrong place, and is reported as its own thing so a shot caught in the blink's dark half is
/// never read as a defect.
export function readBar(png, { accent = ACCENT, ink = INK, paper = PAPER } = {}) {
  const leansBlue = accent.b > accent.r;
  const edge = lum(ink) + (lum(paper) - lum(ink)) * INK_SHARE;
  const inkIsDarker = lum(paper) > lum(ink);

  const cols = new Set();
  const rows = new Set();
  let n = 0;
  for (let y = 0; y < png.h; y += 1) {
    for (let x = 0; x < png.w; x += 1) {
      const i = (y * png.w + x) * png.ch;
      const r = png.data[i];
      const g = png.data[i + 1];
      const b = png.data[i + 2];
      const chroma = leansBlue ? b - r : r - b;
      if (chroma >= CHROMA && (leansBlue ? b > g : r > g)) {
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
      const i = (y * png.w + x) * png.ch;
      const r = png.data[i];
      const g = png.data[i + 1];
      const b = png.data[i + 2];
      const chroma = leansBlue ? b - r : r - b;
      if (chroma >= CHROMA) continue;
      const l = (r + g + b) / 3;
      if (inkIsDarker ? l < edge : l > edge) {
        if (inkLeft === null || x < inkLeft) inkLeft = x;
        if (inkRight === null || x > inkRight) inkRight = x;
      }
    }
  }
  return { bar, oneRun, pixels: n, inkLeft, inkRight };
}

// ---------- the two assertions ----------

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
