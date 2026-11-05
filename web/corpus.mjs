// SPDX-License-Identifier: GPL-3.0-or-later

// Runs the reference scenarios through the wasm module as walkit-trace runs them natively:
// each skin from its archive with the sibling skins of the folder and Winamp's default set,
// the stand-in fonts, pictures decoded by the host (png.mjs), and the run timed on the host's
// clock. Shared by the node runner and the browser page; they differ only in how bytes arrive.

import { Walkit, specOf, crc32, crc32Bytes } from "./walkit.mjs";
import { decodePicture } from "./pictures.mjs";

/**
 * A pack: the entries of an archive as one byte string, `[u32 nameLen][name][u32 dataLen]
 * [data]...`, little endian. The server packs; the page unpacks with no inflater.
 */
export function pack(entries) {
  const enc = new TextEncoder();
  const parts = [];
  let total = 0;
  for (const [name, data] of entries) {
    const n = enc.encode(name);
    parts.push(n, data);
    total += 8 + n.length + data.length;
  }
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  let at = 0;
  for (let i = 0; i < parts.length; i += 2) {
    const n = parts[i];
    const d = parts[i + 1];
    view.setUint32(at, n.length, true);
    out.set(n, at + 4);
    at += 4 + n.length;
    view.setUint32(at, d.length, true);
    out.set(d, at + 4);
    at += 4 + d.length;
  }
  return out;
}

export function unpack(bytes) {
  const dec = new TextDecoder();
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out = new Map();
  let at = 0;
  while (at + 8 <= bytes.length) {
    const nl = view.getUint32(at, true);
    const name = dec.decode(bytes.subarray(at + 4, at + 4 + nl));
    at += 4 + nl;
    const dl = view.getUint32(at, true);
    out.set(name, bytes.subarray(at + 4, at + 4 + dl));
    at += 4 + dl;
  }
  return out;
}

/** The sibling name of a `.wal` file: its name without the extension, lowercased, no spaces. */
export function siblingName(file) {
  return file.replace(/\.wal$/i, "").replace(/ /g, "").toLowerCase();
}

/**
 * Runs every scenario. `source` gives: `scenarios()` -> the JSON list, `wasm()` -> bytes,
 * `archive(file)` -> Map of entries of a `.wal` in the skins folder (or null), `walFiles()`
 * -> the `.wal` names in the folder, `defaults()` -> Map or null, `fonts()` -> Map or null,
 * `inflate(bytes)` -> zlib-inflated bytes (sync or a Promise), `now()` -> ms. `report(line)`
 * hears one line per scenario. Returns [{name, lines, frames, crc, stats, text?}].
 */
export async function runCorpus(source, report = () => {}, keepText = false) {
  const scenarios = await source.scenarios();
  // pictures decoded ahead, by content (the same file name lives in several skins)
  const pictures = new Map();
  const keyOf = (bytes) => `${bytes.length}:${crc32Bytes(bytes)}`;
  const w = await Walkit.load(await source.wasm(), {
    decode: (file, bytes) => pictures.get(keyOf(bytes)) || null,
    now: source.now,
  });
  const decodeAll = async (entries) => {
    for (const [name, data] of entries) {
      if (!/\.(png|bmp|gif)$/i.test(name)) continue;
      const key = keyOf(data);
      if (pictures.has(key)) continue;
      const pic = await decodePicture(data, source.inflate);
      if (pic) pictures.set(key, pic);
    }
  };
  const fileIds = new Map();
  const filesOf = async (name, entries) => {
    if (fileIds.has(name)) return fileIds.get(name);
    await decodeAll(entries);
    const id = w.filesFrom(entries);
    fileIds.set(name, id);
    return id;
  };
  const walFiles = await source.walFiles();
  const defaults = await source.defaults();
  const defaultsId = defaults ? await filesOf("<defaults>", defaults) : -1;
  const fonts = await source.fonts();
  const fontsId = fonts ? await filesOf("<fonts>", fonts) : -1;
  const results = [];
  for (const sc of scenarios) {
    const entries = await source.archive(sc.wal);
    if (!entries) {
      report(`${sc.name}: no ${sc.wal}`);
      continue;
    }
    const skinId = await filesOf(sc.wal, entries);
    const siblings = {};
    if (!sc.noSiblings) {
      // every archive of the folder, the skin itself included (Big Bento reads its scripts that way)
      for (const f of walFiles) {
        const e = await source.archive(f);
        if (e) siblings[siblingName(f)] = await filesOf(f, e);
      }
    }
    const stack = w.stack(skinId, sc.noDefaults ? -1 : defaultsId, siblings);
    const { text, stats } = w.record(stack, fontsId, specOf(sc));
    const r = { name: sc.name, lines: text.split("\n").length - 1, frames: stats.frames, crc: crc32(text), stats };
    if (keepText) r.text = text;
    results.push(r);
    report(`${sc.name}: ${r.lines} lines, ${r.frames} frames, crc ${r.crc}, load ${stats.load_ms.toFixed(1)} ms, tick mean ${(stats.tick_mean_ms * 1000).toFixed(1)} us, draw mean ${(stats.draw_mean_ms * 1000).toFixed(0)} us`);
  }
  return results;
}
