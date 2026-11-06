// SPDX-License-Identifier: GPL-3.0-or-later

// A local server for the browser page: serves the page, the wasm module, the scenarios, and
// each archive unzipped as a pack (see corpus.mjs), and keeps the results a page posts.
// The skins and the default set stay where they are on this machine; nothing is copied.
//
//   node web/serve.mjs --wasm <file> --scenarios <json> --skins <dir> [--defaults <zip>]
//                      [--fonts <dir>] [--results <file>] [--snapshots <dir>] [--port 8765] [--host 127.0.0.1]
//
// `--host 0.0.0.0` opens the page to the local network, so a phone on the same wifi can show
// the player at `http://<this machine>:8765/player.html`.
//
// The player page (`/player.html`) posts the PNG of a frame to `/snapshot/<name>` when
// `--snapshots` names a folder to keep them in.

import { createServer } from "node:http";
import { readFileSync, readdirSync, existsSync, appendFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, dirname, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateRawSync } from "node:zlib";
import { unzip } from "./zip.mjs";
import { pack } from "./corpus.mjs";

const args = process.argv.slice(2);
const opts = { port: "8765", host: "127.0.0.1" };
for (let i = 0; i < args.length; i += 2) opts[args[i].slice(2)] = args[i + 1];
if (!opts.wasm || !opts.scenarios || !opts.skins) {
  console.error("usage: node web/serve.mjs --wasm <file> --scenarios <json> --skins <dir> [--defaults <zip>] [--fonts <dir>] [--results <file>] [--snapshots <dir>] [--port 8765] [--host 127.0.0.1]");
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const types = { ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json" };
const packs = new Map();
const packOf = (key, make) => {
  if (!packs.has(key)) packs.set(key, make());
  return packs.get(key);
};

const server = createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  const send = (status, body, type = "application/octet-stream") => {
    res.writeHead(status, { "content-type": type, "cross-origin-opener-policy": "same-origin", "cross-origin-embedder-policy": "require-corp", "cache-control": "no-store" });
    res.end(body);
  };
  try {
    if (req.method === "POST" && url.pathname.startsWith("/snapshot/")) {
      const name = decodeURIComponent(url.pathname.slice("/snapshot/".length)).replace(/[^A-Za-z0-9_.-]/g, "_");
      const chunks = [];
      req.on("data", (c) => chunks.push(c));
      req.on("end", () => {
        if (!opts.snapshots) return send(404, "no snapshots folder", "text/plain");
        mkdirSync(opts.snapshots, { recursive: true });
        const path = join(opts.snapshots, `${name}.png`);
        writeFileSync(path, Buffer.concat(chunks));
        console.log(`snapshot -> ${path}`);
        send(200, path, "text/plain");
      });
      return;
    }
    if (req.method === "POST" && url.pathname === "/result") {
      let body = "";
      req.on("data", (c) => (body += c));
      req.on("end", () => {
        if (opts.results) appendFileSync(opts.results, body + "\n");
        console.log(`result ${body.length} bytes${opts.results ? ` -> ${opts.results}` : ""}`);
        send(200, "ok", "text/plain");
      });
      return;
    }
    if (url.pathname === "/" || url.pathname === "/index.html") return send(200, readFileSync(join(here, "index.html")), "text/html");
    if (url.pathname === "/walkit.wasm") return send(200, readFileSync(opts.wasm), "application/wasm");
    if (url.pathname === "/scenarios.json") return send(200, readFileSync(opts.scenarios), "application/json");
    if (url.pathname === "/wal-files.json") return send(200, JSON.stringify(readdirSync(opts.skins).filter((f) => f.endsWith(".wal"))), "application/json");
    if (url.pathname.startsWith("/pack/skin/")) {
      const file = decodeURIComponent(url.pathname.slice("/pack/skin/".length));
      const path = join(opts.skins, file);
      if (!existsSync(path) || file.includes("..")) return send(404, "no such skin", "text/plain");
      return send(200, packOf(`skin:${file}`, () => pack(unzip(new Uint8Array(readFileSync(path)), inflateRawSync))));
    }
    if (url.pathname === "/pack/defaults") {
      if (!opts.defaults || !existsSync(opts.defaults)) return send(404, "no defaults", "text/plain");
      return send(200, packOf("defaults", () => pack(unzip(new Uint8Array(readFileSync(opts.defaults)), inflateRawSync))));
    }
    if (url.pathname === "/pack/fonts") {
      if (!opts.fonts || !existsSync(opts.fonts)) return send(404, "no fonts", "text/plain");
      return send(200, packOf("fonts", () => pack(readdirSync(opts.fonts).filter((f) => f.endsWith(".ttf")).map((f) => [`fonts/${f}`, new Uint8Array(readFileSync(join(opts.fonts, f)))]))));
    }
    const local = join(here, url.pathname.replace(/^\//, ""));
    if (!url.pathname.includes("..") && existsSync(local) && extname(local) in types) return send(200, readFileSync(local), types[extname(local)]);
    send(404, "not found", "text/plain");
  } catch (e) {
    send(500, String(e), "text/plain");
  }
});
server.listen(Number(opts.port), opts.host, () => console.log(`http://${opts.host}:${opts.port}/`));
