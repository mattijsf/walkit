# walkit on the web

The engine as a wasm module (`crates/walkit-web`) and the JS that drives it. The JS needs no
build step and no npm. `tools/web-build.sh` writes the modules to `web/dist/`:

- `walkit-release` and `walkit-small`: the page builds, tuned for speed and for size.
- `walkit-release-trace` and `walkit-small-trace`: the same with the trace recorder
  (`wk_record`, the `trace` feature), for the trace scenarios.
- a `.opt` copy of each when `wasm-opt` is installed.

The script prints each module's size raw, gzipped and brotli-compressed.

## The surface

`walkit.mjs` loads the module with a host of callbacks and wraps the exports:

```js
import { Walkit } from "./walkit.mjs";
const w = await Walkit.load(wasmBytes, {
  decode: (file, bytes) => null,          // {w, h, argb: Uint32Array} of a skin picture
  textWidth: (text, font, size, bold) => 0,
  lineMetrics: (font, size, bold) => null, // [ascent, descent]
  imageSize: (file) => null,               // [w, h]
  pixels: (file, x, y, w, h) => null,      // Uint32Array, ARGB
  log: (line) => console.log(line),
});
const skin = w.filesFrom(entriesOfTheWal);          // path -> Uint8Array
const stack = w.stack(skin, w.filesFrom(wasabi)); // the skin over Winamp's wasabi base
const rt = w.runtime(stack, "Winamp Modern");
rt.load();
rt.setNow(performance.now());
if (rt.tick()) draw(rt.draw());                      // the draw list, one command per line
```

The draw list is text in the trace format (`image <file> <x,y,w,h> -> <rect> a=<alpha>`, `fill`,
`text`, `clip`, `unclip`, `shape`, `pixels`), the same lines `walkit-trace` writes, from the
engine's `listing` module. The player page uses the packed frame described below instead.

## Embedding

`tools/web-pack.sh <dir>` puts the `walkit-web` package together (the modules, their types in
`embed.d.ts`, and the module as `walkit.wasm`) for a host to vendor.

`embed.mjs` is the entry point for a page that brings a player and places the windows itself,
such as webamp. It loads the module, opens a skin archive and draws each window.

```js
import { loadWalkit } from "walkit-web";      // or "./embed.mjs" from this folder
const walkit = await loadWalkit({
  invalidate() {},                   // something changed; draw soon
  windowOpen(kind) { return null; }, // "doublesize", "ontop": true, false or null
  player(command, value) {},         // play, pause, stop, next, previous, eject, play_track,
                                     // seek, volume, pan, shuffle, repeat, eq, preamp, band0..9
  action(action, param, from) {},    // MENU, CLOSE, MINIMIZE, EQ_PRESETS, PLAYLIST, ...;
                                     // from.rect is where the button sits, for a menu
});                                  // walkit.wasm next to embed.mjs, or { wasm: url } after
const skin = await walkit.openSkin(wszBytes);  // or a URL; a .wal takes { wasabi }
skin.rt.setFree(true, innerWidth, innerHeight); // windows apart, docked as on Winamp's desktop

function frame(now) {
  skin.rt.setPlayerState({ status: "playing", positionMs, lengthMs, title, volume });
  skin.rt.setPlaylist(tracks);                  // [{ title, lengthMs }], when it changes
  skin.rt.setVis({ spectrum, waveform, vu });    // 75 levels, 576 samples, [left, right]
  if (skin.tick(now)) {
    for (const w of skin.windows()) skin.paint(canvasFor(w), w, devicePixelRatio);
  }
  requestAnimationFrame(frame);
}
```

Each window is `{id, container, layout, rect}`, listed bottom to top. With one canvas per window
at `rect`, the page between the windows stays clickable.

The pointer goes to `skin.rt.pointerDown`, `pointerMove`, `pointerUp` and `doubleTap`, in
viewport pixels. `player` and `action` run after the engine call that raised them returns, so
they may call back into the engine.

The runtime also:

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

## The player page

`player.html` shows a skin from the server's folder, or a `.wal` dropped on the page. The skin
sits over Winamp's wasabi base, and the other archives in the folder are reachable as its
siblings. Pictures are decoded by `pictures.mjs`, and text is drawn with the fonts the server serves (Liberation
Sans stands in for Arial) and any TrueType file in the skin.

Each frame comes out of the module as a packed frame (`wk_rt_draw_frame`, read by
`frame.mjs`): every window with its plan, and only the commands
that changed since the previous frame. `render.mjs` draws it on a canvas, each window in a layer
masked by its shape.

The pointer drives the skin; a long press or a right click opens its menu in a prompt. The
page's buttons drive the engine's fake player, which plays a demo track.

"free windows" (or `?free` in the link) lays the windows out as on a desktop: each opens at its
container's `default_x`, `default_y`, moves by a drag on its move areas, resizes by its grip,
and docks and snaps as in Winamp 5.666.

```bash
node web/serve.mjs --wasm web/dist/walkit-release.wasm --scenarios <json> --skins <dir> \
  --wasabi <zip> --fonts <dir> --snapshots snapshots
# then open http://127.0.0.1:8765/player.html?skin=Bento.wal
# add --host 0.0.0.0 to serve it on the local network; a narrow screen gets zoom 1
# (`?zoom=2` asks for more)
```

"Check frame" draws a fixed frame (a fresh runtime on a clock stepped from 0, the fake player
playing, 200 ticks of 16 ms, the stack at 1x from its corner) and posts it to the server as
`<skin>-check.png`. `?skin=Bento.wal&check` does it on load.

`pngdiff.mjs` compares two PNGs; `--out` writes the first one dimmed, with the differing
pixels in red:

```bash
node web/pngdiff.mjs snapshots/Bento-check.png <other>.png --out diff.png
```

## The trace scenarios

`corpus.mjs` runs the trace scenarios through a module built with the recorder
(`walkit-*-trace`), the same way `walkit-trace` runs them on a desktop, and times every tick and
draw list.

```bash
# in node: writes traces to compare with the desktop walkit-trace's
node web/node.mjs <scenarios.json> traces-wasm \
  --wasm web/dist/walkit-release-trace.wasm \
  --skins <skins dir> \
  --wasabi <wasabi-base.zip> \
  --fonts <fonts dir> \
  --stats node-stats.json

# in a browser: serves the page and the archives (unzipped as packs), keeps the posted results
node web/serve.mjs --wasm web/dist/walkit-release-trace.wasm --scenarios <scenarios.json> \
  --skins ... --wasabi ... --fonts ... --results results.jsonl
# then open http://127.0.0.1:8765/?auto in each browser
```

The inputs are the ones [ENGINEERING.md](../ENGINEERING.md) describes. The skins, the default
set and the fonts stay outside this repository; the server reads them where they are.
