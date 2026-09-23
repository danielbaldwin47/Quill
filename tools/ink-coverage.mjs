// How much ink a judged crop lays down, measured the way a critic reads it.
//
//   node tools/ink-coverage.mjs <ours-crop.png> <oracle-crop.png> --ground dark|light [--json]
//
// The `theme` round 4 verdict (`dev/progress/rounds/theme-r4.json`) gave `theme/dark` to the Design
// oracle on a rendering weight alone: at a byte-identical palette, ours laid down about 4% less
// alpha-weighted glyph area on slightly larger glyphs, and the stem of the ordered-list `1`
// covered 3.6 px against the oracle's 3.8 px. This is that measurement as a function, so a
// rasterisation candidate can be read against the oracle's numbers without spending a critic on
// it. On the round 4 pair it gives the critic's area figures to the pixel and the stem in the
// same ratio (3.72 against 3.96 px: the band below is a wider cut than the critic's, so a stem
// reads about 0.15 px heavier here on both sides). #212 read five candidates through it and found
// none within 1% of the oracle on both numbers; the table is on the ticket.
//
// Three numbers per crop, all in device pixels of the crop:
//
//   area   alpha-weighted glyph area: the sum over every pixel of its ink coverage, where coverage
//          is how far the pixel's luma has travelled from the paper's to the ink's, clamped to
//          [0, 1]. Paper and ink are the palette's own values for the ground, not read off the
//          crop, so a crop that has drifted from the palette shows up as less area rather than as a
//          different palette.
//   stem   the cross-section of the `1`'s vertical stem: the mean, over the stem's middle rows, of
//          that row's coverage summed across the glyph. A stem that holds its edges reads as its
//          nominal width; one that feathers reads wider in pixels but lighter in coverage, and this
//          is the coverage.
//   full   pixels at coverage >= 0.98: the ones a critic counts as "at full ink".
//
// The `1` is found rather than pointed at: text rows are the runs of rows carrying ink, the
// ordered-list line is the one whose second glyph is a full stop (the passage is
// `dev/ref/ia/mac-native/passage-markup.md`, and `1. ordered item` is its only line that opens that
// way), and the stem is the glyph's rows from 40% to 80% of the way down, below the flag and above
// the foot serif. Both forms print the rectangle that was taken for the `1`, so a reader can check
// the glyph that was measured.
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

import { decodePng } from './keys-assert.mjs';

// The two grounds' paper and ink, in luma, as `quill-engine/src/theme.rs` has them today. Not
// `tools/keys-assert.mjs`'s pair: that one is pinned to the palette its committed fixture was shot
// at, and this one has to be the palette a judged crop is shot at now.
export const GROUNDS = {
  light: { paper: 0xf7, ink: 0x19 },
  dark: { paper: 0x1a, ink: 0xcc },
};

// Coverage of one pixel: 0 on the paper, 1 on the ink, in between on an antialiased edge.
function coverageOf(luma, { paper, ink }) {
  const c = (luma - paper) / (ink - paper);
  return c < 0 ? 0 : c > 1 ? 1 : c;
}

// The crop as a coverage field: one Float32 per pixel, row-major.
export function coverageField(png, ground) {
  const { w, h, ch, data } = png;
  const field = new Float32Array(w * h);
  for (let i = 0, p = 0; i < w * h; i += 1, p += ch) {
    // Rec. 601 luma; the crops are grey, so any weighting reads the same, and this is the one a
    // critic's tools use.
    const luma = 0.299 * data[p] + 0.587 * data[p + 1] + 0.114 * data[p + 2];
    field[i] = coverageOf(luma, ground);
  }
  return { w, h, field };
}

// Runs of consecutive indices where `on(i)` holds, as [start, end) pairs.
function runs(n, on) {
  const out = [];
  let start = -1;
  for (let i = 0; i <= n; i += 1) {
    const hit = i < n && on(i);
    if (hit && start < 0) start = i;
    if (!hit && start >= 0) { out.push([start, i]); start = -1; }
  }
  return out;
}

// The text rows of the crop: runs of rows carrying ink, each with its coverage per column.
function textRows({ w, h, field }) {
  const rowInk = new Float32Array(h);
  for (let y = 0; y < h; y += 1) {
    let s = 0;
    for (let x = 0; x < w; x += 1) s += field[y * w + x];
    rowInk[y] = s;
  }
  return runs(h, (y) => rowInk[y] > 0.5);
}

// The glyphs on one text row: runs of columns carrying ink between `y0` and `y1`, each as
// { x0, x1, y0, y1 } tightened to the rows that actually carry ink.
function glyphsOn({ w, field }, [y0, y1]) {
  const colInk = new Float32Array(w);
  for (let x = 0; x < w; x += 1) {
    let s = 0;
    for (let y = y0; y < y1; y += 1) s += field[y * w + x];
    colInk[x] = s;
  }
  return runs(w, (x) => colInk[x] > 0.2).map(([x0, x1]) => {
    let top = y1;
    let bottom = y0;
    for (let y = y0; y < y1; y += 1) {
      let s = 0;
      for (let x = x0; x < x1; x += 1) s += field[y * w + x];
      if (s > 0.2) { top = Math.min(top, y); bottom = Math.max(bottom, y + 1); }
    }
    return { x0, x1, y0: top, y1: bottom };
  });
}

// The widest and tallest a full stop can be, in device pixels at the judged size (the `.` of `1.`
// is 7 x 7 on both sides of the round 4 pair; the next-smallest marker, the `-`, is 13 wide).
const DOT_MAX = 10;

// The `1` of `1. ordered item`: the first glyph on the one text row whose second glyph is a dot.
// The passage's other marker rows put a word straight after the `-` or the `>`, and every body
// row starts with a word, so a row that opens `<glyph> .` is the ordered-list line and its first
// glyph is the `1`, foot serif and flag included.
export function findDigitOne(cov) {
  for (const row of textRows(cov)) {
    const [first, second] = glyphsOn(cov, row);
    if (!first || !second) continue;
    if (second.x1 - second.x0 > DOT_MAX || second.y1 - second.y0 > DOT_MAX) continue;
    if (first.y1 - first.y0 <= DOT_MAX) continue;
    return first;
  }
  throw new Error('no `1` found: no text row opens with a glyph followed by a full stop');
}

// The stem's cross-section: the mean, over the rows 40% to 80% of the way down the glyph, of that
// row's coverage summed across the glyph's columns. The band starts below the flag, which reaches a
// third of the way down the `1`, and ends above the foot serif.
export function stemCoverage(cov, g) {
  const { w, field } = cov;
  const gh = g.y1 - g.y0;
  const from = g.y0 + Math.round(gh * 0.4);
  const to = g.y0 + Math.round(gh * 0.8);
  let total = 0;
  for (let y = from; y < to; y += 1) {
    for (let x = g.x0; x < g.x1; x += 1) total += field[y * w + x];
  }
  return total / (to - from);
}

// The three numbers for one crop.
export function measure(buf, groundName) {
  const ground = GROUNDS[groundName];
  if (!ground) throw new Error(`--ground names ${JSON.stringify(groundName)}, and the grounds are ${Object.keys(GROUNDS).join(', ')}`);
  const cov = coverageField(decodePng(buf), ground);
  let area = 0;
  let full = 0;
  for (let i = 0; i < cov.field.length; i += 1) {
    area += cov.field[i];
    if (cov.field[i] >= 0.98) full += 1;
  }
  const one = findDigitOne(cov);
  return { area, full, stem: stemCoverage(cov, one), one, w: cov.w, h: cov.h };
}

function usage(msg) {
  if (msg) console.error(`ink-coverage: ${msg}`);
  console.error('usage: node tools/ink-coverage.mjs <ours-crop.png> <oracle-crop.png> --ground dark|light [--json]');
  process.exit(3);
}

if (import.meta.url === pathToFileURL(process.argv[1] || '').href) {
  const args = process.argv.slice(2);
  const json = args.includes('--json');
  const at = args.indexOf('--ground');
  const groundName = at >= 0 ? args[at + 1] : null;
  const files = args.filter((a, i) => !a.startsWith('--') && i !== at + 1);
  if (files.length !== 2 || !groundName) usage();
  const [ours, oracle] = files.map((f) => measure(fs.readFileSync(f), groundName));
  if (ours.w !== oracle.w || ours.h !== oracle.h) usage(`the crops are ${ours.w}x${ours.h} and ${oracle.w}x${oracle.h}, and a pair is one size`);
  const row = (name, m) => `${name.padEnd(8)} area ${m.area.toFixed(0).padStart(6)} px²  stem ${m.stem.toFixed(2)} px  full ${String(m.full).padStart(5)}  (1 at x ${m.one.x0}-${m.one.x1}, y ${m.one.y0}-${m.one.y1})`;
  if (json) {
    console.log(JSON.stringify({ ground: groundName, ours, oracle, ratio: { area: ours.area / oracle.area, stem: ours.stem / oracle.stem, full: ours.full / oracle.full } }));
  } else {
    console.log(row('ours', ours));
    console.log(row('oracle', oracle));
    console.log(`ratio    area ${(100 * ours.area / oracle.area).toFixed(1)}%  stem ${(100 * ours.stem / oracle.stem).toFixed(1)}%  full ${(100 * ours.full / oracle.full).toFixed(1)}%`);
  }
}
