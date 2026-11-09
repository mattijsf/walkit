// SPDX-License-Identifier: GPL-3.0-or-later

// Compares two PNGs pixel by pixel, such as the page's check frame and the same frame drawn
// by another host:
//
//   node web/pngdiff.mjs <a.png> <b.png> [--tolerance 0] [--out diff.png]
//
// Prints the sizes, how many pixels differ by more than the tolerance in any channel, and the
// box around them; exits 1 when the sizes differ or any pixel does. With `--out`, writes the
// first picture dimmed with every differing pixel in red.

import { readFileSync, writeFileSync } from "node:fs";
import { deflateSync, inflateSync } from "node:zlib";
import { decodePicture } from "./pictures.mjs";

const args = process.argv.slice(2);
const files = [];
let tolerance = 0;
let out = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--tolerance") tolerance = Number(args[++i]);
  else if (args[i] === "--out") out = args[++i];
  else files.push(args[i]);
}
if (files.length !== 2) {
  console.error("usage: node web/pngdiff.mjs <a.png> <b.png> [--tolerance 0]");
  process.exit(2);
}
const [a, b] = await Promise.all(files.map((f) => decodePicture(new Uint8Array(readFileSync(f)), (x) => inflateSync(x))));
if (!a || !b) {
  console.error("not a picture");
  process.exit(2);
}
if (a.w !== b.w || a.h !== b.h) {
  console.log(`sizes differ: ${a.w}x${a.h} and ${b.w}x${b.h}`);
  process.exit(1);
}
const marks = out ? new Uint8Array(a.w * a.h) : null;
let differing = 0;
let l = a.w;
let t = a.h;
let r = -1;
let bt = -1;
for (let y = 0; y < a.h; y++) {
  for (let x = 0; x < a.w; x++) {
    const p = a.argb[y * a.w + x];
    const q = b.argb[y * a.w + x];
    if (p === q) continue;
    let off = false;
    for (const shift of [24, 16, 8, 0]) if (Math.abs(((p >>> shift) & 0xff) - ((q >>> shift) & 0xff)) > tolerance) off = true;
    if (!off) continue;
    differing++;
    if (marks) marks[y * a.w + x] = 1;
    l = Math.min(l, x);
    t = Math.min(t, y);
    r = Math.max(r, x);
    bt = Math.max(bt, y);
  }
}
if (out) writeFileSync(out, png(a, marks));
const total = a.w * a.h;
console.log(`${a.w}x${a.h}: ${differing} of ${total} pixels differ (${((100 * differing) / total).toFixed(2)} %)${differing ? ` within ${l},${t} to ${r},${bt}` : ""}`);
process.exit(differing ? 1 : 0);

/** `pic` dimmed to a third, every marked pixel red, as a PNG. */
function png(pic, marks) {
  const raw = new Uint8Array((pic.w * 4 + 1) * pic.h);
  for (let y = 0; y < pic.h; y++) {
    raw[y * (pic.w * 4 + 1)] = 0;
    for (let x = 0; x < pic.w; x++) {
      const v = pic.argb[y * pic.w + x];
      const o = y * (pic.w * 4 + 1) + 1 + x * 4;
      if (marks[y * pic.w + x]) {
        raw[o] = 255;
        raw[o + 1] = 0;
        raw[o + 2] = 0;
      } else {
        raw[o] = ((v >>> 16) & 0xff) / 3;
        raw[o + 1] = ((v >>> 8) & 0xff) / 3;
        raw[o + 2] = (v & 0xff) / 3;
      }
      raw[o + 3] = 255;
    }
  }
  const chunk = (type, data) => {
    const t = Buffer.from(type, "ascii");
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([t, data])));
    return Buffer.concat([len, t, data, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(pic.w, 0);
  ihdr.writeUInt32BE(pic.h, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) {
    c ^= b;
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return (c ^ 0xffffffff) >>> 0;
}
