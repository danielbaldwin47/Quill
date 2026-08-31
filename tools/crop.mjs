// The Design oracle's side of a pair: a `mac-native` capture, a rectangle in it, and the matching
// rectangle in ours.
//
//   import { cropPng, resolveOpponent, CAPTURES } from './crop.mjs'
//
// A judged state in `shots/oracle/states.json` may name an `opponent` instead of being judged
// against the Parity oracle's frozen shot ([ADR 0015](../docs/adr/0015-the-design-oracle-outranks-the-parity-oracle.md)).
// What it names is one of the captures of iA Writer for Mac running natively — the Design oracle,
// measured in `ref/ia/mac-native/` — and the crop of it that holds the behaviour the state is
// about. This module reads that entry, works out the matching crop of ours, and cuts both.
//
// WHY OURS NEEDS ITS OWN RECTANGLE. The Design oracle's window is 3024 x 1898 device px and ours is
// 2880 x 1800, both with the container centred, so a rectangle measured in one does not name the
// same text in the other. A state therefore says either where the crop is in ours, or `"centre"` —
// same size, at the same offset from ours' window centre as the opponent's crop sits from the
// capture's window centre. The centre rule is the one for a crop the container's own centring
// carries; a crop the chrome's height decides — a top bar is not the same height in the two apps —
// names its rectangle in ours outright. A capture taken as a region rather than as the whole window
// says so with `window` and `at`, since otherwise its own edges would be read as the window's.
//
// THE PNG. The repo has no image dependency: `decodePng` in `tools/keys-assert.mjs` is its one
// decoder, written for grim's 8-bit RGB and RGBA, and `screencapture` writes the same. Encoding
// back is the filter-none form of the same thing, which is a deflate and three chunks, and it
// lives here rather than beside the decoder because moving the decoder would mean reaching into
// what `tools/gate keys` imports; a `tools/png.mjs` holding both is the tidier home and the
// ticket that next touches the keys tooling can make it.
import zlib from 'node:zlib';
import fs from 'node:fs';
import path from 'node:path';

import { pngSize } from './harness.mjs';
import { decodePng } from './keys-assert.mjs';

// Where a capture a state may name lives. Named by file rather than by path, so a state cannot
// point the opponent at a marketing still in `ref/ia/shots/` — the frames three of this repo's
// wrong readings came off (ADR 0015).
export const CAPTURES = 'ref/ia/shots/mac-native';

// ---------- the entry a state carries ----------

// One rectangle out of a state, as four whole non-negative numbers with a positive size.
function rect(where, value) {
  if (!Array.isArray(value) || value.length !== 4) throw new Error(`${where} is not [x, y, w, h]`);
  const [x, y, w, h] = value;
  for (const n of value) {
    if (!Number.isInteger(n) || n < 0) throw new Error(`${where} is ${JSON.stringify(value)}, and a rectangle is four whole device px`);
  }
  if (w === 0 || h === 0) throw new Error(`${where} is ${JSON.stringify(value)}, which has no area`);
  return [x, y, w, h];
}

// One width-and-height or one origin out of a state, as two whole non-negative device px, or the
// default when the state is silent.
function twoPx(where, value, fallback) {
  if (value === undefined) return fallback;
  if (!Array.isArray(value) || value.length !== 2 || value.some((n) => !Number.isInteger(n) || n < 0)) {
    throw new Error(`${where} is ${JSON.stringify(value)}, and this is two whole device px`);
  }
  return value;
}

// Refuses a rectangle that is not wholly inside a `w` x `h` image, naming what it ran past.
function mustFit(where, [x, y, w, h], size, what) {
  if (x + w > size.w || y + h > size.h) {
    throw new Error(`${where} runs to ${x + w} x ${y + h}, past ${what} (${size.w} x ${size.h})`);
  }
}

// The `opponent` of one judged state, resolved against the capture on disk and the size ours is
// shot at: the capture's path in the repo, the rectangle in it, and the rectangle in ours.
//
// Everything a run could refuse for is refused here, before a window opens — a capture that is not
// there, a rectangle that runs off the edge of one — because `tools/gate judge` checks every state
// before it shoots the first, and a crop that cannot be cut is one more of those.
export function resolveOpponent(root, state, opponent, ours) {
  const where = `state ${state}'s opponent`;
  if (!opponent || typeof opponent !== 'object' || Array.isArray(opponent)) {
    throw new Error(`${where} is not an object naming a capture and a crop`);
  }
  if (typeof opponent.capture !== 'string' || !opponent.capture || opponent.capture.includes('/')) {
    throw new Error(`${where} names ${JSON.stringify(opponent.capture)}, and a capture is a file name under ${CAPTURES}/`);
  }
  const capture = path.join(CAPTURES, opponent.capture);
  const file = path.join(root, capture);
  if (!fs.existsSync(file)) throw new Error(`${where} names ${capture}, which is not a capture on disk`);
  const size = pngSize(fs.readFileSync(file));

  const crop = rect(`${where}'s crop`, opponent.crop);
  mustFit(`${where}'s crop`, crop, size, 'the capture');

  // Where the capture sits in the Design oracle's window, for the states shot as a region rather
  // than whole: `window` is the window's device size and `at` the capture's origin in it. Only the
  // centre rule reads either, and a capture that is the whole window needs neither.
  const [ww, wh] = twoPx(`${where}'s window`, opponent.window, [size.w, size.h]);
  const [ax, ay] = twoPx(`${where}'s at`, opponent.at, [0, 0]);
  if (ax + size.w > ww || ay + size.h > wh) {
    throw new Error(`${where} puts a ${size.w} x ${size.h} capture at ${ax}, ${ay} in a ${ww} x ${wh} window, which does not hold it`);
  }

  const mine = opponent.ours === 'centre'
    ? centred(crop, [ax, ay, ww, wh], ours)
    : rect(`${where}'s ours`, opponent.ours);
  mustFit(`${where}'s crop of ours`, mine, ours, 'ours');
  return { capture, crop, ours: mine };
}

// The crop of ours the centre rule means: the same size, its centre the same distance from ours'
// window centre as the opponent's crop's centre is from the capture's window centre.
function centred([cx, cy, cw, ch], [ix, iy, iw, ih], ours) {
  const dx = ix + cx + cw / 2 - iw / 2;
  const dy = iy + cy + ch / 2 - ih / 2;
  const x = Math.round(ours.w / 2 + dx - cw / 2);
  const y = Math.round(ours.h / 2 + dy - ch / 2);
  if (x < 0 || y < 0) throw new Error(`the centre rule puts the crop at ${x}, ${y}, which is off ours`);
  return [x, y, cw, ch];
}

// ---------- the pixels ----------

// The rectangle of a PNG, as a PNG. The colour type is kept: ours is grim's RGB and a capture is
// screencapture's RGBA, and a pair of two sizes is the one thing a critic must not be shown.
export function cropPng(buf, [x, y, w, h]) {
  const img = decodePng(buf);
  if (x + w > img.w || y + h > img.h) {
    throw new Error(`a crop to ${x + w} x ${y + h} of an image ${img.w} x ${img.h}`);
  }
  const out = Buffer.alloc(w * h * img.ch);
  for (let row = 0; row < h; row += 1) {
    const from = ((y + row) * img.w + x) * img.ch;
    img.data.copy(out, row * w * img.ch, from, from + w * img.ch);
  }
  return encodePng({ w, h, ch: img.ch, data: out });
}

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

// CRC-32 as PNG asks for it, from the table the format's own specification prints.
const TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i += 1) c = TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

// The pixels back as a PNG, every row filtered none: what is written here is read again by the
// decoder above and looked at by a critic, and neither is better off for a filter search.
export function encodePng({ w, h, ch, data }) {
  const stride = w * ch;
  const raw = Buffer.alloc(h * (stride + 1));
  for (let y = 0; y < h; y += 1) {
    raw[y * (stride + 1)] = 0;
    data.copy(raw, y * (stride + 1) + 1, y * stride, y * stride + stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = ch === 3 ? 2 : 6;
  return Buffer.concat([
    SIGNATURE,
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}
