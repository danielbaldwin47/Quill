// Reproduce #326's pixel comparison from the repository root:
// node progress/page-326/measure.mjs
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { decodePng } from '../../tools/keys-assert.mjs';

function read(file) {
  const bytes = fs.readFileSync(file);
  const png = decodePng(bytes);
  const rgb = Buffer.alloc(png.w * png.h * 3);
  for (let p = 0; p < png.w * png.h; p++) {
    png.data.copy(rgb, p * 3, p * png.ch, p * png.ch + 3);
  }
  return { file, png, rgb, sha256: hash(bytes), rgbSha256: hash(rgb) };
}

function hash(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function differences(a, b, top, bottom, shift = 0) {
  if (a.png.w !== b.png.w || bottom + shift > b.png.h) throw Error('Incompatible rectangles');
  let pixels = 0;
  for (let y = top; y < bottom; y++) {
    for (let x = 0; x < a.png.w; x++) {
      const i = (y * a.png.w + x) * 3;
      const j = ((y + shift) * b.png.w + x) * 3;
      if (!a.rgb.subarray(i, i + 3).equals(b.rgb.subarray(j, j + 3))) pixels++;
    }
  }
  return pixels;
}

// Dark glyph cores only, excluding the bars; fuse disconnected descenders into
// their row across at most 12 empty scanlines. Bounds are inclusive device px.
function rows({ png: { w, h }, rgb }) {
  const bands = [];
  for (let y = 100; y < h - 60; y++) {
    const xs = [];
    for (let x = 90; x < w - 90; x++) {
      const p = (y * w + x) * 3;
      if (rgb[p] < 90 && rgb[p + 1] < 90 && rgb[p + 2] < 90) xs.push(x);
    }
    if (xs.length <= 10) continue;
    const prior = bands.at(-1);
    if (prior && y - prior.bottom <= 13) {
      prior.bottom = y;
      prior.left = Math.min(prior.left, xs[0]);
      prior.right = Math.max(prior.right, xs.at(-1));
    } else bands.push({ top: y, bottom: y, left: xs[0], right: xs.at(-1) });
  }
  return bands;
}

const old = read('shots/page/r7-narrow-ours.png');
const failed = read('shots/page/r8-narrow-ours.png');
const fresh = read('shots/page/r9-narrow-ours.png');
const parity = read('shots/oracle/page/narrow.png');
console.log(JSON.stringify({
  images: [old, failed, fresh, parity].map(image => ({
    file: image.file, width: image.png.w, height: image.png.h,
    sha256: image.sha256, rgbSha256: image.rgbSha256, rows: rows(image),
  })),
  changedPixels: {
    round7To8: differences(old, failed, 0, old.png.h),
    round8To9: differences(failed, fresh, 0, failed.png.h),
    // Full width, including the gutters. These overlapping Editor rectangles
    // exclude both bars and the old bottom-of-window text clipped by the new bar.
    round7To8Aligned: differences(old, failed, 100, 1636, 64),
  },
  alignedRectangles: { round7: [0, 100, 1920, 1536], round8: [0, 164, 1920, 1536] },
}, null, 2));
