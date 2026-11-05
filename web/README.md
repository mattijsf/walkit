# walkit on the web

The engine as a wasm module (`crates/walkit-web`) and the JS that drives it. No build step
and no npm on the JS side; the module comes from `tools/web-build.sh` (both profiles, with
`wasm-opt` when installed) into `web/dist/`.

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
`text`, `clip`, `unclip`, `shape`, `pixels`), the same lines `walkit-trace` writes; a renderer
parses it. A packed binary form is a follow-up.

## The reference scenarios

`corpus.mjs` runs the reference set through the module as `walkit-trace` does on a desktop, on the
fixed trace host inside the module (text by a rule, pictures decoded by `pictures.mjs`), and
times every tick and draw list on the host's clock.

```bash
# in node: writes traces to compare with the desktop build's
node web/node.mjs ~/Development/winamp-vm/scenarios.json traces-wasm \
  --wasm web/dist/walkit-release.wasm \
  --skins ~/Development/andamp/modernskin/src/test/resources \
  --defaults ~/Development/winamp-vm/wasabi-defaults.zip \
  --fonts <andamp>/modernskin/src/main/resources/nl/mattix/andamp/modernskin/defaultset/fonts \
  --stats node-stats.json

# in a browser: serves the page and the archives (unzipped as packs), keeps the posted results
node web/serve.mjs --wasm web/dist/walkit-release.wasm --scenarios ~/Development/winamp-vm/scenarios.json \
  --skins ... --defaults ... --fonts ... --results results.jsonl
# then open http://127.0.0.1:8765/?auto in each browser
```

The inputs are the ones that decide parity (see `ENGINEERING.md`): the corpus skins are
Andamp's `modernskin/src/test/resources/*.wal` (Bento and Big Bento are only there), and the
fonts folder must hold Liberation Sans as well as DejaVu Sans Condensed, as the default set
of an Andamp checkout at current main does. The skins, the default set and the fonts never
enter this repository; the server reads them where they are.
