// SPDX-License-Identifier: GPL-3.0-or-later

// A player page on the wasm engine: a skin from the server's folder (or a `.wal` dropped on
// the page) over Winamp's wasabi base, drawn on a canvas from the engine's packed frames,
// driven by the pointer, on the engine's fake player. The same engine, frames and plans the
// Android library gets, for trying a skin and comparing the two platforms.

import { Walkit } from "./walkit.mjs";
import { unpack, siblingName } from "./corpus.mjs";
import { unzipAsync } from "./zip.mjs";
import { decodePicture } from "./pictures.mjs";
import { Pictures, Renderer, context2d, emPixels, normalize, familyOf, inSkinFolder } from "./render.mjs";
import { FrameReader } from "./frame.mjs";
import { SkinCursors } from "./cursor.mjs";

const $ = (id) => document.getElementById(id);
const log = $("log");
const say = (line) => {
  log.textContent += line + "\n";
  log.scrollTop = log.scrollHeight;
};

async function inflateWith(format, bytes) {
  const ds = new DecompressionStream(format);
  const writer = ds.writable.getWriter();
  writer.write(bytes);
  writer.close();
  return new Uint8Array(await new Response(ds.readable).arrayBuffer());
}
const inflate = (b) => inflateWith("deflate", b);
const inflateRaw = (b) => inflateWith("deflate-raw", b);

const fetchBytes = async (url) => {
  const r = await fetch(url);
  if (!r.ok) return null;
  return new Uint8Array(await r.arrayBuffer());
};

// pictures decoded once per file content, whichever skin holds it
const decoded = new WeakMap();
async function decodeOnce(bytes) {
  if (decoded.has(bytes)) return decoded.get(bytes);
  const pic = await decodePicture(bytes, inflate);
  decoded.set(bytes, pic);
  return pic;
}

/** The fonts the page registered: a font file (normalized path) or `id:<name>` to a CSS family. */
// where the engine names the files of what lies under the skin (walkit::files::BASE_PREFIX)
const BASE_PREFIX = "wasabi-base/";

const families = new Map();
const registered = new Set();

/** Registers `bytes` as the font family named by `path`, once. */
async function registerFont(path, bytes) {
  const key = normalize(path);
  if (registered.has(key)) return;
  registered.add(key);
  try {
    const face = new FontFace(key, bytes);
    await face.load();
    document.fonts.add(face);
    families.set(key, key);
  } catch (e) {
    say(`font ${path}: ${e}`);
  }
}

/** The Windows fonts a skin names by id, on the stand-in the fonts folder carries. */
const standIns = { arial: "fonts/liberationsans.ttf", "arial black": "fonts/liberationsans.ttf", tahoma: "fonts/liberationsans.ttf", verdana: "fonts/dejavusanscondensed.ttf", "ms sans serif": "fonts/liberationsans.ttf", "microsoft sans serif": "fonts/liberationsans.ttf", "trebuchet ms": "fonts/dejavusanscondensed.ttf" };

const measure = (typeof OffscreenCanvas !== "undefined" ? new OffscreenCanvas(1, 1) : document.createElement("canvas")).getContext("2d");

/** The font the engine names, as `font_name` in the module spells it (`id|file`), to a CSS font. */
function cssFont(name, size, bold) {
  const [id, file] = name ? name.split("|") : ["", ""];
  const font = name ? { id, file } : null;
  return `${bold ? "bold " : ""}${emPixels(size)}px ${JSON.stringify(familyOf(font, families))}`;
}

// every archive of the server's folder, by file, once fetched: a skin reads its siblings' files
const archives = new Map();
async function archive(file) {
  if (!archives.has(file)) {
    const b = await fetchBytes(`/pack/skin/${encodeURIComponent(file)}`);
    archives.set(file, b ? unpack(b) : null);
  }
  return archives.get(file);
}
/** The pictures of each archive (by file), decoded once. */
const archivePictures = new Map();
async function picturesOf(file, entries) {
  if (archivePictures.has(file)) return archivePictures.get(file);
  const pics = new Map();
  for (const [n, b] of inSkinFolder(entries)) {
    if (!/\.(png|bmp|gif)$/i.test(n)) continue;
    const pic = await decodeOnce(b);
    if (pic) pics.set(normalize(n), pic);
  }
  archivePictures.set(file, pics);
  return pics;
}

/** A picture of a sibling skin, by the path the skin's XML writes: `../<folder>/<path>`. */
function siblingPicture(key) {
  const m = /^\.\.\/([^/]+)\/(.+)$/.exec(key);
  if (!m) return null;
  const wanted = siblingName(m[1]);
  for (const f of state.walFiles) {
    if (siblingName(f) !== wanted) continue;
    const pics = archivePictures.get(f);
    return (pics && pics.get(m[2])) || null;
  }
  return null;
}

const fileIds = new Map();
function fileIdOf(file, entries) {
  if (!fileIds.has(file)) fileIds.set(file, state.w.filesFrom(entries));
  return fileIds.get(file);
}

const state = {
  w: null,
  walFiles: [],
  rt: null,
  pictures: null,
  renderer: null,
  reader: null,
  files: null, // normalized path -> bytes, the skin over the wasabi base
  wasabi: null,
  fonts: null,
  wasabiId: -1,
  dirty: true,
  clock: null,
  frame: null,
  seq: 0,
  down: false,
  longPress: null,
  stats: { ticks: 0, tickMs: 0, frames: 0, frameMs: 0, bytes: 0, drawMs: 0, shown: 0, since: performance.now() },
};
// the page state, reachable from the console for checks
globalThis.walkitPlayer = state;

const canvas = $("view");
const ctx = context2d(canvas);
const zoom = () => Number($("zoom").value);
const scale = () => zoom() * (window.devicePixelRatio || 1);

async function loadModule() {
  const wasm = await fetchBytes("/walkit.wasm");
  state.w = await Walkit.load(wasm, {
    imageSize: (file) => (state.pictures ? state.pictures.size(file) : null),
    pixels: (file, x, y, w, h) => (state.pictures ? state.pictures.pixels(file, x, y, w, h) : null),
    textWidth: (text, font, size, bold) => {
      measure.font = cssFont(font, size, bold);
      return Math.round(measure.measureText(text).width);
    },
    lineMetrics: (font, size, bold) => {
      measure.font = cssFont(font, size, bold);
      const m = measure.measureText("Hg");
      return m.fontBoundingBoxAscent ? [m.fontBoundingBoxAscent, m.fontBoundingBoxDescent] : null;
    },
    fontFile: (file) => (state.files && state.files.get(normalize(file))) || (state.fonts && state.fonts.get(normalize(file))) || null,
    log: (line) => say(line),
    invalidate: () => {
      state.dirty = true;
    },
    windowOpen: () => null,
    chooseFromMenu: (text) => chooseFromMenu(text),
    windowsChanged: () => {},
    hostAction: (action, param) => say(`host action ${action} ${param}`),
    now: () => performance.now(),
  });
  state.wasabi = await fetchBytes("/pack/wasabi").then((b) => (b ? unpack(b) : new Map()));
  state.wasabiId = state.wasabi.size ? state.w.filesFrom(state.wasabi) : -1;
  say(`engine loaded; wasabi base ${state.wasabi.size} files`);
}

/**
 * The stand-in fonts for the system fonts skins name, fetched once when a skin first wants
 * them: a modern skin lays its text out with them, a classic skin only draws its playlist in one.
 */
function loadFonts() {
  if (!state.fontsReady) {
    state.fontsReady = (async () => {
      state.fonts = await fetchBytes("/pack/fonts").then((b) => (b ? unpack(b) : new Map()));
      for (const [name, bytes] of state.fonts) await registerFont(name, bytes);
      for (const [id, file] of Object.entries(standIns)) if (families.has(file)) families.set(`id:${id}`, file);
      if (families.has("fonts/liberationsans.ttf")) families.set("", "fonts/liberationsans.ttf");
      say(`${state.fonts.size} fonts`);
    })();
  }
  return state.fontsReady;
}

/** Whether `entries` hold a classic skin: a main.bmp and no skin.xml. */
function isClassic(entries) {
  const names = new Set([...inSkinFolder(entries).keys()].map(normalize));
  return names.has("main.bmp") && !names.has("skin.xml");
}

/**
 * The sister skins `entries` read files of, by `siblingName`: the folders their XML and scripts
 * name as `../<folder>/` (Bento reads Big Bento's). A classic skin reads none.
 */
function siblingsNamed(entries) {
  const out = new Set();
  const text = new TextDecoder("latin1");
  for (const [n, b] of entries) {
    if (!/\.(xml|maki|m)$/i.test(n)) continue;
    for (const m of text.decode(b).matchAll(/\.\.[\\/]+([^\\/"'<>\r\n]+)[\\/]/g)) out.add(siblingName(m[1]));
  }
  return out;
}

/**
 * A menu the skin pops, as the module spells it (`[id:text*! - text>[...]]`): the entries
 * numbered in a prompt; the id chosen, or null.
 */
function chooseFromMenu(text) {
  const items = [];
  const re = /(\d+):((?:[^\s\[\]>]|\\ )+)(\*?)(!?)/g;
  let m;
  while ((m = re.exec(text))) items.push({ id: Number(m[1]), text: m[2].replace(/\\ /g, " "), checked: !!m[3], disabled: !!m[4] });
  if (!items.length) return null;
  const lines = items.map((it, i) => `${i + 1}. ${it.checked ? "* " : ""}${it.text}${it.disabled ? " (off)" : ""}`);
  const answer = window.prompt(`Pick an entry:\n${lines.join("\n")}`, "");
  const n = Number(answer);
  if (!answer || !Number.isInteger(n) || n < 1 || n > items.length) return null;
  return items[n - 1].id;
}

async function openSkin(name, entries, { clock = null } = {}) {
  state.clock = clock;
  if (state.rt) {
    state.rt.free();
    state.rt = null;
  }
  const files = new Map();
  for (const [n, b] of inSkinFolder(entries)) files.set(normalize(n), b);
  state.files = files;
  state.pictures = new Pictures(siblingPicture);
  const t0 = performance.now();
  for (const [k, pic] of await picturesOf(name, entries)) state.pictures.pics.set(k, pic);
  // the engine names the files under the skin by the prefix (walkit::files::BASE_PREFIX)
  for (const [k, pic] of await picturesOf("<wasabi>", state.wasabi)) state.pictures.pics.set(BASE_PREFIX + k, pic);
  for (const [n, b] of inSkinFolder(entries)) if (/\.ttf$/i.test(n)) await registerFont(n, b);
  const decodeMs = performance.now() - t0;
  state.renderer = new Renderer(state.pictures, families);
  state.reader = new FrameReader();
  const skinId = fileIdOf(name, entries);
  // the sister skins the skin names, and theirs in turn (Bento reads Big Bento's scripts);
  // none for a classic skin
  const siblings = {};
  siblings[siblingName(name)] = skinId;
  const wanted = siblingsNamed(entries);
  for (let more = true; more; ) {
    more = false;
    for (const f of state.walFiles) {
      const key = siblingName(f);
      if (f === name || key in siblings || !wanted.has(key)) continue;
      const e = await archive(f);
      if (!e) continue;
      siblings[key] = fileIdOf(f, e);
      await picturesOf(f, e);
      for (const k of siblingsNamed(e)) if (!wanted.has(k)) (wanted.add(k), (more = true));
    }
  }
  // a modern skin lays text out with the stand-in fonts as it loads; a classic skin starts
  // without them and draws its playlist again once they are here
  if (isClassic(entries)) {
    loadFonts().then(() => {
      state.dirty = true;
    });
  } else {
    await loadFonts();
  }
  const stack = state.w.stack(skinId, state.wasabiId, siblings);
  const skinName = name.replace(/\.(wal|wsz)$/i, "");
  const t1 = performance.now();
  state.rt = state.w.runtime(stack, skinName, true, clock === null ? performance.now() : clock);
  state.cursors = new SkinCursors(state.rt);
  state.rt.load();
  const loadMs = performance.now() - t1;
  if (clock === null) applyView();
  state.dirty = true;
  state.stats = { ticks: 0, tickMs: 0, frames: 0, frameMs: 0, bytes: 0, drawMs: 0, shown: 0, since: performance.now() };
  say(`${name}: ${entries.size} files, pictures decoded in ${decodeMs.toFixed(0)} ms, loaded in ${loadMs.toFixed(1)} ms`);
}

/**
 * Fits the stack to the view the page asks for; a main window that resizes both ways fills it, as
 * Android fits Bento. With "free windows" on, the view is a desktop the windows sit apart on,
 * moved by a drag and docked as Winamp docks them.
 */
function applyView() {
  const rt = state.rt;
  if (!rt) return;
  const w = Number($("viewW").value) || 0;
  const h = Number($("viewH").value) || 0;
  state.free = $("free").checked;
  const main = rt.mainWindow();
  if (!state.free && main !== null && w > 0 && h > 0 && rt.windowResizable(main)) rt.resizeWindow(main, w, h);
  rt.setView(w, h);
  rt.setFree(state.free, w, h);
  state.dirty = true;
}

/**
 * The frame Andamp's check mode draws, drawn here the same way: a fresh runtime on a clock
 * stepped from 0, the fake player playing, 200 ticks of 16 ms, the stack at 1x from its
 * corner; the PNG goes to the server as `<skin>-check.png`.
 */
async function checkFrame() {
  const name = $("skins").value;
  const entries = await archive(name);
  if (!entries || !state.w) return;
  await openSkin(name, entries, { clock: 0 });
  const rt = state.rt;
  let now = 0;
  rt.setNow(now);
  rt.player(1);
  const t0 = performance.now();
  for (let i = 0; i < 200; i++) {
    now += 16;
    rt.setNow(now);
    rt.tick();
    state.reader.decode(rt.drawFrame(now));
  }
  const frame = state.reader.decode(rt.drawFrame(now));
  const ms = performance.now() - t0;
  state.frame = frame;
  state.dirty = true;
  const [w, h] = stackSize(frame);
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  new Renderer(state.pictures, families).draw(c.getContext("2d"), frame, 1);
  const base = name.replace(/\.(wal|wsz)$/i, "");
  await new Promise((done) =>
    c.toBlob(async (blob) => {
      const r = await fetch(`/snapshot/${encodeURIComponent(`${base}-check`)}`, { method: "POST", body: blob });
      say(`${name}: check frame ${w}x${h} after 200 ticks in ${ms.toFixed(0)} ms${r.ok ? `, kept as ${await r.text()}` : ""}`);
      done();
    }, "image/png"),
  );
  // the page's clock from here on
  state.clock = null;
}

async function loadFromServer() {
  const file = $("skins").value;
  if (!file) return;
  const entries = await archive(file);
  if (!entries) {
    say(`${file}: not served`);
    return;
  }
  await openSkin(file, entries);
}

function stackSize(frame) {
  let w = 0;
  let h = 0;
  for (const win of frame.windows) {
    w = Math.max(w, win.rect.left + win.rect.width);
    h = Math.max(h, win.rect.top + win.rect.height);
  }
  return [w, h];
}

function show(frame) {
  const s = scale();
  // apart, the windows sit on the view as a desktop; stacked, the canvas is the stack
  const [w, h] = state.free ? [Number($("viewW").value) || 0, Number($("viewH").value) || 0] : stackSize(frame);
  const pw = Math.max(1, Math.ceil(w * s));
  const ph = Math.max(1, Math.ceil(h * s));
  if (canvas.width !== pw || canvas.height !== ph) {
    canvas.width = pw;
    canvas.height = ph;
    canvas.style.width = `${Math.ceil(w * zoom())}px`;
    canvas.style.height = `${Math.ceil(h * zoom())}px`;
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, pw, ph);
  state.renderer.draw(ctx, frame, s);
}

function loop() {
  requestAnimationFrame(loop);
  const rt = state.rt;
  if (!rt || state.clock !== null) return;
  const st = state.stats;
  const now = performance.now();
  rt.setNow(now);
  const t0 = performance.now();
  const changed = rt.tick();
  st.tickMs += performance.now() - t0;
  st.ticks++;
  if (!changed && !state.dirty) {
    report(now);
    return;
  }
  state.dirty = false;
  const t1 = performance.now();
  const bytes = rt.drawFrame(now);
  const frame = state.reader.decode(bytes);
  st.frameMs += performance.now() - t1;
  st.bytes += bytes.length;
  st.frames++;
  state.frame = frame;
  const t2 = performance.now();
  show(frame);
  st.drawMs += performance.now() - t2;
  st.shown++;
  report(now);
}

let lastReport = 0;
function report(now) {
  if (now - lastReport < 500) return;
  lastReport = now;
  const st = state.stats;
  const secs = (now - st.since) / 1000;
  const per = (sum, n) => (n ? (sum / n).toFixed(2) : "-");
  $("stats").textContent =
    `ticks ${per(st.tickMs, st.ticks)} ms; frames ${st.frames} (${(st.frames / Math.max(secs, 0.001)).toFixed(1)}/s): ` +
    `engine and read ${per(st.frameMs, st.frames)} ms, ${st.frames ? Math.round(st.bytes / st.frames) : 0} bytes, draw ${per(st.drawMs, st.shown)} ms`;
}

// ---- pointer --------------------------------------------------------------------------------

function skinPoint(e) {
  const r = canvas.getBoundingClientRect();
  return [Math.floor((e.clientX - r.left) / zoom()), Math.floor((e.clientY - r.top) / zoom())];
}

canvas.addEventListener("pointerdown", (e) => {
  if (!state.rt || e.button !== 0) return;
  canvas.setPointerCapture(e.pointerId);
  const [x, y] = skinPoint(e);
  state.rt.setTouch(e.pointerType !== "mouse");
  state.down = true;
  state.seq++;
  state.rt.pointerDown(x, y, state.seq);
  state.dirty = true;
  clearTimeout(state.longPress);
  state.longPress = setTimeout(() => {
    if (state.down && state.rt) {
      state.rt.rightClick(state.seq);
      state.dirty = true;
    }
  }, 600);
});
canvas.addEventListener("pointerleave", () => state.cursors?.hide(canvas));

canvas.addEventListener("pointermove", (e) => {
  if (state.rt && !state.down) state.cursors.show(canvas, ...skinPoint(e), zoom());
  if (!state.rt || !state.down) return;
  const [x, y] = skinPoint(e);
  state.rt.pointerMove(x, y);
  state.dirty = true;
});
const up = (e) => {
  if (!state.rt || !state.down) return;
  state.down = false;
  clearTimeout(state.longPress);
  const [x, y] = skinPoint(e);
  state.rt.pointerUp(x, y);
  state.dirty = true;
};
canvas.addEventListener("pointerup", up);
canvas.addEventListener("pointercancel", up);
canvas.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  if (!state.rt) return;
  const [x, y] = skinPoint(e);
  state.seq++;
  state.rt.pointerDown(x, y, state.seq);
  state.rt.pointerUp(x, y);
  state.rt.rightClick(state.seq);
  state.dirty = true;
});
canvas.addEventListener("dblclick", (e) => {
  if (!state.rt) return;
  const [x, y] = skinPoint(e);
  state.rt.doubleTap(x, y, state.seq);
  state.dirty = true;
});

// ---- the bar --------------------------------------------------------------------------------

$("load").addEventListener("click", () => loadFromServer().catch((e) => say(String(e))));
$("check").addEventListener("click", () => checkFrame().catch((e) => say(String(e))));
for (const [id, action] of [["play", 1], ["pause", 2], ["stop", 3], ["next", 4], ["prev", 5]]) {
  $(id).addEventListener("click", () => {
    if (!state.rt) return;
    state.rt.player(action);
    state.dirty = true;
  });
}
$("zoom").addEventListener("change", () => {
  state.dirty = true;
});
for (const id of ["viewW", "viewH", "free"]) $(id).addEventListener("change", applyView);
window.walkit = state;
window.walkitCheck = checkFrame;
$("snapshot").addEventListener("click", () => {
  if (!state.frame) return;
  // the stack at 1x from its corner, as Andamp's check mode draws it; kept by the server when
  // it has a folder for snapshots, else downloaded
  const [w, h] = stackSize(state.frame);
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  new Renderer(state.pictures, families).draw(c.getContext("2d"), state.frame, 1);
  const name = ($("skins").value || "skin").replace(/\.(wal|wsz)$/i, "");
  c.toBlob(async (blob) => {
    const r = await fetch(`/snapshot/${encodeURIComponent(name)}`, { method: "POST", body: blob });
    if (r.ok) {
      say(`snapshot kept as ${await r.text()}`);
      return;
    }
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = `${name}.png`;
    a.click();
  }, "image/png");
});
document.body.addEventListener("dragover", (e) => e.preventDefault());
document.body.addEventListener("drop", async (e) => {
  e.preventDefault();
  const f = e.dataTransfer.files[0];
  if (!f) return;
  try {
    const bytes = new Uint8Array(await f.arrayBuffer());
    const entries = await unzipAsync(bytes, inflateRaw);
    await openSkin(f.name, entries);
  } catch (err) {
    say(`${f.name}: ${err}`);
  }
});

(async () => {
  try {
    const files = await (await fetch("/wal-files.json")).json();
    state.walFiles = files;
    const select = $("skins");
    for (const f of files) {
      const o = document.createElement("option");
      o.value = f;
      o.textContent = f;
      select.appendChild(o);
    }
    await loadModule();
    const params = new URL(location.href).searchParams;
    // A phone shows the skin at zoom 1, unless the link says otherwise.
    const zoomWanted = params.get("zoom");
    if (zoomWanted) $("zoom").value = zoomWanted;
    if (params.has("free")) $("free").checked = true;
    else if (window.innerWidth < 2 * Number($("viewW").value) + 64) $("zoom").value = "1";
    const wanted = params.get("skin");
    if (wanted && files.includes(wanted)) {
      select.value = wanted;
      if (params.has("check")) await checkFrame();
      else await loadFromServer();
      // open=library,video opens those windows as well (a classic skin's library and video)
      for (const name of (params.get("open") || "").split(",").filter(Boolean)) {
        if (state.rt && state.rt.openWindow(name)) state.dirty = true;
      }
    }
    requestAnimationFrame(loop);
  } catch (e) {
    say(String(e));
  }
})();
