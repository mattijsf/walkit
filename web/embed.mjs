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
//   const skin = await walkit.openSkin(wszBytes);
//   skin.rt.setFree(true, innerWidth, innerHeight);
//   // each animation frame:
//   skin.rt.setPlayerState({ status: "playing", positionMs, lengthMs, title });
//   if (skin.tick(performance.now())) for (const w of skin.windows()) skin.paint(canvasOf(w.id), w, devicePixelRatio);
//
// `player` and `action` run once the engine call that raised them has returned, so a handler
// may call back into the engine. One skin draws at a time.

import { Walkit } from "./walkit.mjs";
import { FrameReader } from "./frame.mjs";
import { Pictures, Renderer, normalize, inSkinFolder } from "./render.mjs";
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
    this.measure = document.createElement("canvas").getContext("2d");
  }

  async load(wasm) {
    const later = (f) => queueMicrotask(f);
    const h = () => this.handlers;
    this.w = await Walkit.load(wasm, {
      imageSize: (f) => this.pictures?.size(f) ?? null,
      pixels: (f, x, y, w, ht) => this.pictures?.pixels(f, x, y, w, ht) ?? null,
      textWidth: (text, _font, size, bold) => {
        const px = this.classic ? pointsToPixels(size) : size;
        this.measure.font = `${bold ? "bold " : ""}${px}px Arial, sans-serif`;
        return Math.round(this.measure.measureText(text).width);
      },
      lineMetrics: () => null,
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
   * .wal over `defaults` (Winamp's default element set, path -> bytes). On the page's player
   * unless `demoPlayer`.
   */
  async openSkin(source, { name = "skin", defaults = null, demoPlayer = false } = {}) {
    const entries = inSkinFolder(await unzipAsync(await bytesOf(source), inflateRaw));
    const names = new Set([...entries.keys()].map(normalize));
    const pictures = new Pictures();
    for (const layer of [entries, defaults || new Map()]) await pictures.add(layer, inflate);
    this.pictures = pictures;
    this.classic = isClassic(names);
    const defaultsId = defaults ? this.w.filesFrom(defaults) : -1;
    const stack = this.w.stack(this.w.filesFrom(entries), defaultsId);
    const make = demoPlayer ? "runtime" : "hostedRuntime";
    const rt = this.w[make](stack, name, false, performance.now());
    rt.load();
    const families = new Map([["", "Arial"]]);
    const renderer = this.classic ? new PointRenderer(pictures, families) : new Renderer(pictures, families);
    return new Skin(rt, renderer);
  }
}

/** An open skin: its runtime (`rt`, see `Runtime` in walkit.mjs) and its windows to paint. */
export class Skin {
  constructor(rt, renderer) {
    this.rt = rt;
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
