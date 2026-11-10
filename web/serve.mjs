// SPDX-License-Identifier: GPL-3.0-or-later

// A local server for the browser page: serves the page, the wasm module, the scenarios, and
// each archive unzipped as a pack (see corpus.mjs), and keeps the results a page posts.
// The skins and the wasabi base stay where they are on this machine; nothing is copied.
//
//   node web/serve.mjs --wasm <file> --scenarios <json> --skins <dir> [--wasabi <zip>]
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
import { inflateRawSync, gzipSync } from "node:zlib";
import { createHash } from "node:crypto";
import { unzip } from "./zip.mjs";
import { pack } from "./corpus.mjs";

const args = process.argv.slice(2);
const opts = { port: "8765", host: "127.0.0.1" };
for (let i = 0; i < args.length; i += 2) opts[args[i].slice(2)] = args[i + 1];
if (!opts.wasm || !opts.scenarios || !opts.skins) {
  console.error("usage: node web/serve.mjs --wasm <file> --scenarios <json> --skins <dir> [--wasabi <zip>] [--fonts <dir>] [--results <file>] [--snapshots <dir>] [--port 8765] [--host 127.0.0.1]");
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const types = { ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json" };
const packs = new Map();
const packOf = (key, make) => {
  if (!packs.has(key)) packs.set(key, make());
  return packs.get(key);
};

// a body's tag and gzip, kept with the body: the packs are made once, so are these
const tags = new WeakMap();
const zipped = new WeakMap();
const tagOf = (body) => {
  if (!tags.has(body)) tags.set(body, `"${createHash("sha1").update(body).digest("base64url")}"`);
  return tags.get(body);
};
const gzipOf = (body) => {
  if (!zipped.has(body)) zipped.set(body, gzipSync(body, { level: 6 }));
  return zipped.get(body);
};

const server = createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  // a large body goes out gzipped when the browser takes it, with a tag the browser checks
  // next time: an unchanged skin, font set or module then costs no download
  const send = (status, body, type = "application/octet-stream") => {
    const head = { "content-type": type, "cross-origin-opener-policy": "same-origin", "cross-origin-embedder-policy": "require-corp", "cache-control": "no-store" };
    if (status === 200 && typeof body !== "string" && body.length > 1024) {
      const tag = tagOf(body);
      head["cache-control"] = "no-cache";
      head.etag = tag;
      head.vary = "accept-encoding";
      if (req.headers["if-none-match"] === tag) {
        res.writeHead(304, head);
        return res.end();
      }
      if (/\bgzip\b/.test(req.headers["accept-encoding"] || "")) {
        head["content-encoding"] = "gzip";
        body = gzipOf(body);
      }
    }
    res.writeHead(status, head);
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
    if (url.pathname === "/wal-files.json") return send(200, JSON.stringify(readdirSync(opts.skins).filter((f) => f.endsWith(".wal") || f.endsWith(".wsz"))), "application/json");
    if (url.pathname.startsWith("/pack/skin/")) {
      const file = decodeURIComponent(url.pathname.slice("/pack/skin/".length));
      const path = join(opts.skins, file);
      if (!existsSync(path) || file.includes("..")) return send(404, "no such skin", "text/plain");
      return send(200, packOf(`skin:${file}`, () => pack(unzip(new Uint8Array(readFileSync(path)), inflateRawSync))));
    }
    if (url.pathname === "/pack/wasabi") {
      if (!opts.wasabi || !existsSync(opts.wasabi)) return send(404, "no wasabi base", "text/plain");
      return send(200, packOf("wasabi", () => pack(unzip(new Uint8Array(readFileSync(opts.wasabi)), inflateRawSync))));
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
