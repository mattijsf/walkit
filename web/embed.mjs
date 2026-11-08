// SPDX-License-Identifier: GPL-3.0-or-later

// The engine for a page that brings its own player and windows: load it once, open a skin
// archive, and each frame tick it, take its windows and paint each one on a canvas.
//
//   const walkit = await loadWalkit(wasmUrl, {
//     invalidate() {},                  // something changed: draw a frame soon
//     windowOpen(kind) { return null }, // "doublesize", "ontop", "pl", "eq": true, false or null
//     player(command, value) {},        // a button for the page's player (see Walkit.hostedRuntime)
//     action(action, param, from) {},   // MENU, CLOSE, PLAYLIST, ...; from: {rect, point} in the viewport
//   });
//   const skin = await walkit.openSkin(wszBytes);  // a .wal: { defaults }, after addFonts(fonts)
//   skin.rt.setFree(true, innerWidth, innerHeight);
//   // each animation frame:
//   skin.rt.setPlayerState({ status: "playing", positionMs, lengthMs, title });
//   if (skin.tick(performance.now())) for (const w of skin.windows()) skin.paint(canvasOf(w.id), w, devicePixelRatio);
//
// `player` and `action` run once the engine call that raised them has returned, so a handler
// may call back into the engine. One skin draws at a time.

import { Walkit } from "./walkit.mjs";
import { FrameReader } from "./frame.mjs";
import { Pictures, Renderer, normalize, inSkinFolder, familyOf } from "./render.mjs";
import { decodePicture } from "./pictures.mjs";
import { unzipAsync } from "./zip.mjs";

async function inflateWith(format, bytes) {
  const stream = new DecompressionStream(format);
  const writer = stream.writable.getWriter();
  writer.write(bytes);
  writer.close();
  return new Uint8Array(await new Response(stream.readable).arrayBuffer());
}
const inflate = (b) => inflateWith("deflate", b);
const inflateRaw = (b) => inflateWith("deflate-raw", b);

async function bytesOf(source) {
  if (source instanceof Uint8Array) return source;
  if (source instanceof ArrayBuffer) return new Uint8Array(source);
  const r = await fetch(source);
  if (!r.ok) throw new Error(`walkit: ${source}: ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
}

const isClassic = (names) => names.has("main.bmp") && !names.has("skin.xml");

// the free fonts that stand in for the Windows fonts skins name
const STAND_INS = {
  arial: "fonts/liberationsans.ttf",
  "arial black": "fonts/liberationsans.ttf",
  tahoma: "fonts/liberationsans.ttf",
  verdana: "fonts/dejavusanscondensed.ttf",
  "ms sans serif": "fonts/liberationsans.ttf",
  "microsoft sans serif": "fonts/liberationsans.ttf",
  "trebuchet ms": "fonts/dejavusanscondensed.ttf",
};

// a classic skin gives its text sizes in points
const pointsToPixels = (size) => (size * 3) / 4;

class PointRenderer extends Renderer {
  fontOf(cmd) {
    return super.fontOf({ ...cmd, size: pointsToPixels(cmd.size) });
  }
}

/** Loads the engine from a URL or the module's bytes, with the page's `handlers`. */
export async function loadWalkit(wasm, handlers = {}) {
  const embed = new Embed(handlers);
  await embed.load(await bytesOf(wasm));
  return embed;
}

class Embed {
  constructor(handlers) {
    this.handlers = handlers;
    this.pictures = null;
    this.classic = false;
    this.fonts = new Map();
    this.skinFiles = new Map();
    this.families = new Map([["", "Arial"]]);
    this.registered = new Set();
    this.measure = document.createElement("canvas").getContext("2d");
  }

  /** Registers `bytes` as the font family named by its normalized path, once. */
  async registerFont(path, bytes) {
    const key = normalize(path);
    if (this.registered.has(key)) return;
    this.registered.add(key);
    try {
      const face = new FontFace(key, bytes);
      await face.load();
      document.fonts.add(face);
      this.families.set(key, key);
    } catch {
      this.registered.delete(key);
    }
  }

  /**
   * The stand-in fonts (path -> bytes, `fonts/LiberationSans.ttf` and the like) a modern skin
   * lays its text out with where it names a Windows font.
   */
  async addFonts(fonts) {
    for (const [path, bytes] of fonts) {
      this.fonts.set(normalize(path), bytes);
      await this.registerFont(path, bytes);
    }
    for (const [id, file] of Object.entries(STAND_INS)) {
      if (this.families.has(file)) this.families.set(`id:${id}`, file);
    }
    if (this.families.has(STAND_INS.arial)) this.families.set("", STAND_INS.arial);
  }

  cssFont(name, size, bold) {
    const [id, file] = name ? name.split("|") : ["", ""];
    const family = familyOf(name ? { id, file } : null, this.families);
    const px = this.classic ? pointsToPixels(size) : size;
    return `${bold ? "bold " : ""}${px}px ${JSON.stringify(family)}`;
  }

  async load(wasm) {
    const later = (f) => queueMicrotask(f);
    const h = () => this.handlers;
    this.w = await Walkit.load(wasm, {
      imageSize: (f) => this.pictures?.size(f) ?? null,
      pixels: (f, x, y, w, ht) => this.pictures?.pixels(f, x, y, w, ht) ?? null,
      textWidth: (text, font, size, bold) => {
        this.measure.font = this.cssFont(font, size, bold);
        return Math.round(this.measure.measureText(text).width);
      },
      lineMetrics: (font, size, bold) => {
        this.measure.font = this.cssFont(font, size, bold);
        const m = this.measure.measureText("Hg");
        return m.fontBoundingBoxAscent ? [m.fontBoundingBoxAscent, m.fontBoundingBoxDescent] : null;
      },
      fontFile: (file) => this.skinFiles.get(normalize(file)) ?? this.fonts.get(normalize(file)) ?? null,
      log: (line) => h().log?.(line),
      invalidate: () => h().invalidate?.(),
      windowOpen: (kind) => h().windowOpen?.(kind) ?? null,
      chooseFromMenu: () => null,
      windowsChanged: (ids) => later(() => h().windowsChanged?.(ids)),
      hostAction: (action, param, from) => later(() => h().action?.(action, param, from)),
      player: (command, value) => later(() => h().player?.(command, value)),
    });
  }

  /**
   * Opens a skin archive (a URL, an ArrayBuffer or bytes): a classic .wsz on its own, a modern
   * .wal over `defaults` (Winamp's default element set: an archive like the skin, or path ->
   * bytes) with the fonts given to `addFonts`. On the page's player unless `demoPlayer`.
   */
  async openSkin(source, { name = "skin", defaults = null, demoPlayer = false } = {}) {
    const entries = inSkinFolder(await unzipAsync(await bytesOf(source), inflateRaw));
    if (defaults && !(defaults instanceof Map)) {
      defaults = await unzipAsync(await bytesOf(defaults), inflateRaw);
    }
    const names = new Set([...entries.keys()].map(normalize));
    const pictures = new Pictures();
    for (const layer of [entries, defaults || new Map()]) await pictures.add(layer, inflate);
    this.pictures = pictures;
    this.classic = isClassic(names);
    this.skinFiles = new Map([...entries].map(([path, bytes]) => [normalize(path), bytes]));
    for (const [path, bytes] of entries) {
      if (/\.ttf$/i.test(path)) await this.registerFont(path, bytes);
    }
    const defaultsId = defaults ? this.w.filesFrom(defaults) : -1;
    const stack = this.w.stack(this.w.filesFrom(entries), defaultsId);
    const make = demoPlayer ? "runtime" : "hostedRuntime";
    const rt = this.w[make](stack, name, false, performance.now());
    rt.load();
    const families = this.families;
    const renderer = this.classic ? new PointRenderer(pictures, families) : new Renderer(pictures, families);
    return new Skin(rt, renderer, this.classic);
  }
}

/**
 * An open skin: its runtime (`rt`, see `Runtime` in walkit.mjs), whether it is a classic
 * skin, and its windows to paint.
 */
export class Skin {
  constructor(rt, renderer, classic) {
    this.rt = rt;
    this.classic = classic;
    this.renderer = renderer;
    this.reader = new FrameReader();
    this.shown = [];
    this.dirty = true;
  }

  /** Steps the clock to `now` (ms); whether the windows need painting again. */
  tick(now) {
    this.rt.setNow(now);
    const changed = this.rt.tick() || this.dirty;
    this.dirty = false;
    if (changed) this.shown = this.reader.decode(this.rt.drawFrame(now)).windows;
    return changed;
  }

  /** The windows on screen, bottom to top: `{id, container, layout, rect}`, rect in the viewport. */
  windows() {
    return this.shown;
  }

  /** Asks for the windows again on the next `tick`. */
  invalidate() {
    this.dirty = true;
  }

  /** Paints `window` on `canvas` at `scale` device pixels per skin pixel, its corner at the canvas's. */
  paint(canvas, window, scale = 1) {
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    const rect = { ...window.rect, left: 0, top: 0 };
    this.renderer.draw(ctx, { windows: [{ ...window, rect }] }, scale);
  }

  free() {
    this.rt.free();
  }
}
