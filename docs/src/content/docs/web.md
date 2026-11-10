---
title: Web
---

The engine builds as a wasm module (`crates/walkit-web`) with plain JS modules around it. The JS
needs no build step and no npm.

```bash
rustup target add wasm32-unknown-unknown
tools/web-build.sh        # the modules into web/dist/, with their sizes
tools/web-pack.sh <dir>   # the walkit-web package for a page to vendor
```

The package holds the JS modules, their types in `embed.d.ts`, and the module as `walkit.wasm`.

[`examples/web`](https://github.com/mattijsf/walkit/tree/main/examples/web) is a complete page
built on it.

## Embedding

`embed.mjs` is the entry point for a page that brings a player and places the windows itself. It
loads the module, opens a skin archive and draws each window on a canvas.

```js
import { loadWalkit } from "walkit-web";
const walkit = await loadWalkit({
  invalidate() {},                   // something changed; draw soon
  windowOpen(kind) { return null; }, // "doublesize", "ontop": true, false or null
  player(command, value) {},         // play, pause, stop, next, previous, eject, play_track,
                                     // seek, volume, pan, shuffle, repeat, eq, preamp, band0..9
  action(action, param, from) {},    // MENU, CLOSE, MINIMIZE, EQ_PRESETS, PLAYLIST, ...;
                                     // from.rect is where the button sits, for a menu
});                                  // walkit.wasm next to embed.mjs, or { wasm: url }
const skin = await walkit.openSkin(wszBytes);  // or a URL; a .wal takes { defaults }
skin.rt.setFree(true, innerWidth, innerHeight); // windows apart, docked as on Winamp's desktop

function frame(now) {
  skin.rt.setPlayerState({ status: "playing", positionMs, lengthMs, title, volume });
  skin.rt.setPlaylist(tracks);                  // [{ title, lengthMs }], when it changes
  skin.rt.setVis({ spectrum, waveform, vu });   // 75 levels, 576 samples, [left, right]
  if (skin.tick(now)) {
    for (const w of skin.windows()) skin.paint(canvasFor(w), w, devicePixelRatio);
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
```

Each window is `{id, container, layout, rect}`, listed bottom to top. With one canvas per window
at `rect`, the page between the windows stays clickable.

The pointer goes to `skin.rt.pointerDown`, `pointerMove`, `pointerUp` and `doubleTap`, in
viewport pixels. `player` and `action` run after the engine call that raised them returns, so
they may call back into the engine.

## The runtime

Besides drawing, the runtime:

- places, opens and closes windows: `placeWindow`, `openWindow`, `closeWindow`,
  `setWindowLayout`;
- scales a window: `setWindowScale(container, 2)` for Winamp's double size, any factor from
  0.25 to 8, and docked windows stay docked;
- names a classic frame: `setWindowTitle`;
- says where the page draws its visualizer: `visRect`, a classic skin's `avs` window framed by
  `gen.bmp`, or a modern skin's AVS holder;
- says which window has the focus: `focusedWindow`;
- shows the page's playlist selection and scrolls a classic playlist: `setPlaylistSelection`,
  `scrollPlaylist`;
- keeps a classic skin's time direction and equalizer auto in step with the host's settings:
  `timeRemaining`, `eqAuto` and their setters.

`embed.d.ts` types the whole surface.

## The lower layer

`walkit.mjs` wraps the module's exports directly, for a host that decodes pictures and measures
text itself:

```js
import { Walkit } from "./walkit.mjs";
const w = await Walkit.load(wasmBytes, {
  decode: (file, bytes) => null,           // {w, h, argb: Uint32Array} of a skin picture
  textWidth: (text, font, size, bold) => 0,
  lineMetrics: (font, size, bold) => null, // [ascent, descent]
  imageSize: (file) => null,               // [w, h]
  pixels: (file, x, y, w, h) => null,      // Uint32Array, ARGB
  log: (line) => console.log(line),
});
const skin = w.filesFrom(entriesOfTheWal);          // path -> Uint8Array
const stack = w.stack(skin, w.filesFrom(defaults)); // the skin over Winamp's default set
const rt = w.runtime(stack, "Winamp Modern");
rt.load();
rt.setNow(performance.now());
if (rt.tick()) draw(rt.draw());                     // the draw list, one command per line
```

## The player page

`web/player.html` shows a skin served from a local folder, or a `.wal` dropped on the page, with
the engine's fake player. The repository's
[web/README.md](https://github.com/mattijsf/walkit/blob/main/web/README.md) shows how to serve it.
