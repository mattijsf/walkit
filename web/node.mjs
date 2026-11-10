// SPDX-License-Identifier: GPL-3.0-or-later

// Runs the trace scenarios through the wasm module in node and writes each run's trace,
// for a byte compare with the traces walkit-trace writes on a desktop:
//
//   node web/node.mjs <scenarios.json> <out dir> --wasm <file> [--skins <dir>] [--wasabi <zip>]
//                     [--fonts <dir>] [--stats <file>]

import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { join, basename } from "node:path";
import { inflateRawSync, inflateSync } from "node:zlib";
import { performance } from "node:perf_hooks";
import { unzip } from "./zip.mjs";
import { runCorpus } from "./corpus.mjs";

function usage() {
  console.error("usage: node web/node.mjs <scenarios.json> <out dir> --wasm <file> [--skins <dir>] [--wasabi <zip>] [--fonts <dir>] [--stats <file>]");
  process.exit(2);
}

const args = process.argv.slice(2);
const opts = {};
const plain = [];
for (let i = 0; i < args.length; i++) {
  if (args[i].startsWith("--")) {
    opts[args[i].slice(2)] = args[i + 1];
    i++;
  } else plain.push(args[i]);
}
if (plain.length !== 2 || !opts.wasm) usage();
const [scenariosPath, outDir] = plain;
const skins = opts.skins || join(scenariosPath, "..", "skins");
mkdirSync(outDir, { recursive: true });

const archives = new Map();
const archive = (file) => {
  if (archives.has(file)) return archives.get(file);
  const path = join(skins, file);
  const entries = existsSync(path) ? unzip(new Uint8Array(readFileSync(path)), inflateRawSync) : null;
  archives.set(file, entries);
  return entries;
};
const fontsDir = opts.fonts;
const fonts = fontsDir
  ? new Map(readdirSync(fontsDir).filter((f) => f.endsWith(".ttf")).map((f) => [`fonts/${f}`, new Uint8Array(readFileSync(join(fontsDir, f)))]))
  : null;

const started = performance.now();
const results = await runCorpus(
  {
    scenarios: () => JSON.parse(readFileSync(scenariosPath, "utf8")),
    wasm: () => readFileSync(opts.wasm),
    archive,
    walFiles: () => readdirSync(skins).filter((f) => f.endsWith(".wal")),
    wasabi: () => (opts.wasabi && existsSync(opts.wasabi) ? unzip(new Uint8Array(readFileSync(opts.wasabi)), inflateRawSync) : null),
    fonts: () => fonts,
    inflate: (b) => inflateSync(b),
    now: () => performance.now(),
  },
  (line) => console.log(line),
  true
);
for (const r of results) writeFileSync(join(outDir, `${r.name}.trace`), r.text);
if (opts.stats) {
  const out = {
    engine: `walkit-web ${basename(opts.wasm)} on node ${process.version}`,
    total_ms: performance.now() - started,
    scenarios: results.map((r) => ({ ...r.stats, crc: r.crc })),
  };
  writeFileSync(opts.stats, JSON.stringify(out, null, 1));
}
