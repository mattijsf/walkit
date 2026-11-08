# walkit on the web

The engine as a wasm module (`crates/walkit-web`) and the JS that drives it. No build step
and no npm on the JS side; the module comes from `tools/web-build.sh` into `web/dist/`:
`walkit-release` and `walkit-small` are the page builds (profile `release` for speed, `small`
for size, no recorder), `walkit-release-trace` and `walkit-small-trace` carry the recorder
(`wk_record`, the `trace` feature) for the reference scenarios, and each has a `.opt` twin
when `wasm-opt` is installed. The script prints every module's size raw, gzipped and
brotli-compressed.

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
const stack = w.stack(skin, w.filesFrom(defaults)); // the skin over Winamp's default set
const rt = w.runtime(stack, "Winamp Modern");
rt.load();
rt.setNow(performance.now());
if (rt.tick()) draw(rt.draw());                      // the draw list, one command per line
```

The draw list is text in the trace format (`image <file> <x,y,w,h> -> <rect> a=<alpha>`, `fill`,
`text`, `clip`, `unclip`, `shape`, `pixels`), the same lines `walkit-trace` writes, from the
engine's `listing` module; a renderer parses it. A packed binary form is a follow-up.

## Embedding

`tools/web-pack.sh <dir>` puts the `walkit-web` package together (the modules, their types in
`embed.d.ts`, and the module as `walkit.wasm`) for a host to vendor. `embed.mjs` is the way in for a page that has its own player and puts the windows where it
wants them (webamp, say): it loads the module, opens a skin archive, and hands out each
window's drawing.

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
const skin = await walkit.openSkin(wszBytes);  // or a URL; a .wal takes { defaults }
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

Each window is `{id, container, layout, rect}`, bottom to top; a canvas per window at
`rect` keeps the page between windows clickable. The pointer goes to `skin.rt.pointerDown`,
`pointerMove`, `pointerUp` and `doubleTap` in viewport pixels. `player` and `action` run once
the engine call that raised them has returned, so they may call back into the engine. The
runtime also places, opens and closes windows (`placeWindow`, `openWindow`, `closeWindow`,
`setWindowLayout`), scales them (`setWindowScale(container, 2)` for Winamp's double size, any
factor from 0.25 to 8, the windows docked to it staying docked), names a classic frame
(`setWindowTitle`), says where the page draws its visualizer (`visRect`: a classic skin's
`avs` window, framed by `gen.bmp`, or a modern skin's AVS holder), says which window has the
focus (`focusedWindow`), scrolls a classic playlist
(`scrollPlaylist`), and keeps a classic skin's time direction and equalizer auto
(`timeRemaining`, `eqAuto` and their setters) for a host that has the same settings.

## The player page

`player.html` shows a skin from the server's folder, or a `.wal` dropped on the page, on the
engine in the browser: the skin over Winamp's default set, its sibling archives reachable as
on a phone, the pictures decoded by `pictures.mjs`, text measured and drawn with the fonts the
server serves (Liberation Sans stands in for Arial) and any TrueType file the skin carries.
Each frame comes out of the module as the packed frame the Android library gets
(`wk_rt_draw_frame`, read by `frame.mjs`): every window with its plan and only the commands
that changed since the frame before; `render.mjs` draws it on a canvas, each window in a layer
masked by its shape. The pointer drives the skin (a long press or a right click opens its
menu in a prompt); the buttons drive the engine's fake player, which plays a demo track. A
view size fits the stack as the phone does, and a main window that resizes both ways fills it.
"free windows" (or `?free` in the link) sets the windows out apart on the view as a desktop:
each opens at its container's `default_x`, `default_y`, moves by a drag on its move areas,
sizes by its grip, and docks and snaps as Winamp 5.666 docks them (as measured in
the reference VM).

```bash
node web/serve.mjs --wasm web/dist/walkit-release.wasm --scenarios <json> --skins <dir> \
  --defaults <zip> --fonts <dir> --snapshots snapshots
# then open http://127.0.0.1:8765/player.html?skin=Bento.wal
# add --host 0.0.0.0 to open it to the local network: a phone on the same wifi shows the
# skin at its own pixels (`?zoom=2` asks for more)
```

"Check frame" draws the frame Andamp's check mode draws on a phone (a fresh runtime on a clock
stepped from 0, the fake player playing, 200 ticks of 16 ms, the stack at 1x from its corner)
and posts it to the server as `<skin>-check.png`; `?skin=Bento.wal&check` does it on load.
`pngdiff.mjs` compares it with the phone's:

```bash
node web/pngdiff.mjs snapshots/Bento-check.png <phone>/wal-check/Bento.png --out diff.png
```

What differs between the two is what the hosts draw differently, not the engine: text (each
platform's font rendering) and the fake players' demo data (the spectrum, the time). The draw
lists themselves are the same on both, as the parity run shows.

## The reference scenarios

`corpus.mjs` runs the reference set through a module with the recorder (`walkit-*-trace`) as
`walkit-trace` does natively, on the fixed trace host inside the module (text by a rule,
pictures decoded by `pictures.mjs`), and times every tick and draw list on the host's clock.

```bash
# in node: writes traces to compare with the native binary's
node web/node.mjs ~/Development/winamp-vm/scenarios.json traces-wasm \
  --wasm web/dist/walkit-release-trace.wasm \
  --skins ~/Development/andamp/modernskin/src/test/resources \
  --defaults ~/Development/winamp-vm/wasabi-defaults.zip \
  --fonts <andamp>/modernskin/src/main/resources/nl/mattix/andamp/modernskin/defaultset/fonts \
  --stats node-stats.json

# in a browser: serves the page and the archives (unzipped as packs), keeps the posted results
node web/serve.mjs --wasm web/dist/walkit-release-trace.wasm --scenarios ~/Development/winamp-vm/scenarios.json \
  --skins ... --defaults ... --fonts ... --results results.jsonl
# then open http://127.0.0.1:8765/?auto in each browser
```

The inputs are the ones that decide parity (see `ENGINEERING.md`): the corpus skins are
Andamp's `modernskin/src/test/resources/*.wal` (Bento and Big Bento are only there), and the
fonts folder must hold Liberation Sans as well as DejaVu Sans Condensed, as the default set
of an Andamp checkout at current main does. The skins, the default set and the fonts never
enter this repository; the server reads them where they are.
