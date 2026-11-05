// SPDX-License-Identifier: GPL-3.0-or-later

// The page's side of the corpus run: bytes come from serve.mjs as packs, zlib streams are
// undone with the browser's DecompressionStream, and the results go back to the server.

import { runCorpus, unpack } from "./corpus.mjs";

const log = document.getElementById("log");
const status = document.getElementById("status");
const say = (line) => {
  log.textContent += line + "\n";
};

async function inflate(bytes) {
  const ds = new DecompressionStream("deflate");
  const writer = ds.writable.getWriter();
  writer.write(bytes);
  writer.close();
  return new Uint8Array(await new Response(ds.readable).arrayBuffer());
}

const fetchBytes = async (url) => {
  const r = await fetch(url);
  if (!r.ok) return null;
  return new Uint8Array(await r.arrayBuffer());
};

const archives = new Map();
const archive = async (file) => {
  if (!archives.has(file)) {
    const b = await fetchBytes(`/pack/skin/${encodeURIComponent(file)}`);
    archives.set(file, b ? unpack(b) : null);
  }
  return archives.get(file);
};

async function run() {
  log.textContent = "";
  status.textContent = `running in ${navigator.userAgent}`;
  const started = performance.now();
  const results = await runCorpus(
    {
      scenarios: async () => (await fetch("/scenarios.json")).json(),
      wasm: () => fetchBytes("/walkit.wasm"),
      archive,
      walFiles: async () => (await fetch("/wal-files.json")).json(),
      defaults: async () => {
        const b = await fetchBytes("/pack/defaults");
        return b ? unpack(b) : null;
      },
      fonts: async () => {
        const b = await fetchBytes("/pack/fonts");
        return b ? unpack(b) : null;
      },
      inflate,
      now: () => performance.now(),
    },
    say
  );
  const out = {
    engine: `walkit-web in ${navigator.userAgent}`,
    isolated: self.crossOriginIsolated,
    total_ms: performance.now() - started,
    scenarios: results.map((r) => ({ ...r.stats, crc: r.crc })),
  };
  status.textContent = `done: ${results.length} scenarios in ${(out.total_ms / 1000).toFixed(1)} s`;
  if (document.getElementById("post").checked) {
    await fetch("/result", { method: "POST", body: JSON.stringify(out) });
    status.textContent += ", results posted";
  }
  window.walkitResults = out;
}

document.getElementById("run").addEventListener("click", run);
if (new URLSearchParams(location.search).has("auto")) run();
