# walkit engineering notes

walkit is the engine of a modern Winamp skin (`.wal`): the skin's XML and images, its
compiled MAKI scripts, the layout of its windows, and a draw list per frame that any
renderer can draw. The engine draws nothing, reads no file and plays no sound. A host does
that: it gives the engine the skin's files, measures text, decodes images, feeds touch
input, answers the player's questions, and draws the draw list.

## Commands

```bash
tools/gate.sh                               # the push gate: fmt, clippy -D warnings, tests
cargo test --workspace                      # the tests
cargo run -p walkit-trace -- <scenarios.json> traces --skins <dir> --defaults <zip>   # record the reference scenarios
tools/trace-diff.sh                         # diff traces/ against the reference engine's
tools/web-build.sh                          # the web build: page and recorder modules, wasm-opt, sizes
(cd android && ./gradlew assembleRelease)   # the Android AAR, native library included (android/README.md)
node web/node.mjs <scenarios.json> traces-wasm --wasm web/dist/walkit-release-trace.wasm \
  --skins <dir> --defaults <zip> --fonts <dir>    # the same scenarios through the wasm module in node
node web/serve.mjs --wasm web/dist/walkit-release-trace.wasm --scenarios <json> --skins <dir> \
  --defaults <zip> --fonts <dir> --results results.jsonl   # the same in a browser, at http://127.0.0.1:8765/?auto
```

The wasm build needs the `wasm32-unknown-unknown` target of a rustup toolchain (`rustup target
add wasm32-unknown-unknown`); a cargo from Homebrew has none, so the scripts put `~/.cargo/bin`
first. `wasm-opt` (Homebrew `binaryen`) is optional; `brotli` and `node` give the sizes and the
web runs.

## Layout

- `crates/walkit`: the engine, a library (an `rlib`; the web and Android crates wrap it into
  their libraries). No unsafe code (`#![forbid(unsafe_code)]`); every public item documented; clippy's pedantic set
  on for the workspace (`[workspace.lints]` in `Cargo.toml`, less the cast lints an engine in
  layout pixels and bytes trips on), with warnings as errors in the gate. Its
  modules follow the reference engine's files: `files` (the skin's files and how a host stacks
  them), `xml`, `draw` (the draw list), `host` (what the engine asks of its host), `player`,
  `gui` (what each kind of object keeps, in one arena the runtime reaches by index), `text`,
  `runtime` (loading, layout, drawing, the tick, touches and actions, the objects scripts make,
  and the interpreter that runs the scripts' handlers) and `maki` (values, the `.maki` parser,
  the standard library's table, and what a loaded script keeps). The runtime is one `SkinRuntime`
  whose methods are spread over themed files: `resources` (bitmaps, fonts, colors), `windows`
  (containers, windows and the presentation), `events` (change tracking and dispatch), `shapes`
  (text placement, window shapes, fitting pictures), `tree` (the object tree, the layout pass,
  the draw lists), and per kind of object `objects` (attributes), `sizes`, `animated`,
  `buttons`, `sliders`, `draw_kinds` and `touches`, beside `build`, `layout`, `draw`, `pointer`,
  `tick`, `lists`, `vis`, `options`, `system`, `calls`, `maki` and `vm`. `runtime/free.rs` sets the
  windows out apart when the host asks for it (`Presentation::free_windows`): each where it
  is (a new one at its container's `default_x`, `default_y`), moved by a drag and sized by its
  grip, docking and snapping as Winamp 5.666 does, raised by a press (the main window with the
  windows docked to it), measured in the reference VM;
  the stack is the default and the
  only arrangement the reference traces run.
- `crates/walkit/src/classic`: the classic skin renderer. `mod.rs` detects a `.wsz` by its
  bitmaps and generates the `skin.xml` that puts one `classicwindow` object in each window's
  normal and shade layout; `main.rs`, `eq.rs` and `pl.rs` paint the three windows and their
  shade bars from the sheets (`main.bmp`, `titlebar.bmp`, `cbuttons.bmp`, `eqmain.bmp`,
  `pledit.bmp` and the rest) pixel for pixel as Winamp 5.666 draws them; `vis.rs` the analyzer
  and oscilloscope over the `viscolor.txt` palette; `font.rs`, `region.txt`, `pledit.txt` and
  `viscolor.txt` parsers beside them; `touch.rs` the presses, drags and double taps on the
  controls, each control's box being the rect the painter blits it to. Every rule was
  measured against the real player in a VM; the Andamp notes hold the list.
- `crates/walkit-trace`: the recorder that runs a scenario through the engine on a stepped
  clock and writes the trace, as a platform-free library (the fixed host, the scenario, the
  timing of each tick and draw list), and a binary that adds the files, the archives, image
  decoding and the JSON of a native run. Not published.
- `crates/walkit-web`: the engine for the web, a `cdylib` for `wasm32-unknown-unknown` with a
  C-style surface (`wk_*` exports, `host_*` imports): files in, a runtime the page drives
  (`wk_rt_*`: load, clock, tick, draw list out as text, touches, windows, themes), and
  `wk_record` for the reference scenarios on the fixed host. The recorder is behind the
  `trace` feature (on by default; the page build turns it off and carries no recorder, no
  stats and no picture decoding). The allocator is talc, a few KB smaller than Rust's
  default. It is the one crate with unsafe code, for the exports, the allocator and the
  calls into the host; on any other target it is empty.
- `crates/walkit-android`: the engine for Android, `libwalkit.so`: the same operations as the web
  surface, as native methods of `nl.mattix.walkit.Native`, with the host's answers coming back
  through bridge objects the Kotlin side registers. A frame crosses as the packed buffer of the
  engine's `packed` module. The other crate with unsafe code (the handle the Kotlin side holds).
- `android/`: the Gradle project of the AAR `nl.mattix.walkit:walkit`: the Kotlin API (package
  `nl.mattix.walkit`, a mirror of Andamp's `:modernskin` `skin` package), the JVM unit tests of
  the packed decoding, and the instrumented parity test; `android/README.md` has the toolchain.
- `web/`: the JS side. `walkit.mjs` loads the module and wraps the surface; `corpus.mjs` runs
  the reference scenarios as `walkit-trace` does; `pictures.mjs` decodes PNG, BMP and GIF the
  way the native binary does; `zip.mjs` reads `.wal` archives; `node.mjs` is the node runner,
  `serve.mjs` + `index.html` + `browser.mjs` the browser one; `player.html` + `player.mjs`
  show a skin on the engine in the browser, drawn by `render.mjs` from the packed frames
  `frame.mjs` reads, and `pngdiff.mjs` compares a frame with the phone's. No build step, no npm.
- `tools/`: the gate, the trace diff and the web build.

The Android library takes the Rust targets `aarch64-linux-android` and `x86_64-linux-android`,
`cargo-ndk`, a JDK 17 and the Android SDK with NDK 28; `android/README.md` lists the steps.

## The trace

The engine is ported from the Kotlin engine in Andamp's `:modernskin` module, which stays
the reference. Both engines write the same text for the same scripted run: `trace v1`, the
skin and the playlist, the clock, each step (load, settle, play, stop, tap, theme, view),
the skin's log lines, menus and host actions, each tick's work, and the full draw list of
every frame that changed. The reference engine's traces are written by its
`EngineTraceTest` into `modernskin/build/traces` of the Andamp checkout; walkit-trace writes
the same files into `traces/`; `tools/trace-diff.sh` prints the first line where each pair
parts. A scenario passes when its traces are identical.

The format is fixed by the reference engine's `EngineTrace` fixture (the Kotlin side); a
change there is a version bump (`trace v2`) on both sides.

The scenarios and the skins they run are a local set (`scenarios.json`, a folder of `.wal`
files given with `--skins`, plus Winamp's default element set as a zip given with
`--defaults`; `--fonts` names a folder with the free fonts that stand in for Windows' system
fonts). A scenario names its skin's file, its folder name, the songs, and what the run does;
the other `.wal` files in the folder are the skin's siblings (the skin itself included: Big
Bento reads its own scripts through `@SKINSPATH@`). They are not in this repository and are
never committed: the skins are other people's work. Traces are not committed either (tens of
megabytes per run).

Two inputs decide parity with the reference engine, and both live outside this repository:
the corpus skins are the `.wal` files of Andamp's `modernskin/src/test/resources` (Bento and
Big Bento are only there), and `--fonts` must be the default-set fonts folder of an Andamp
checkout at current main (`modernskin/src/main/resources/nl/mattix/andamp/modernskin/
defaultset/fonts`, with DejaVu Sans Condensed and Liberation Sans, regular and bold); without
Liberation Sans the baselines of Arial texts differ by fractions of a pixel. The web runs take
the same inputs through `--skins`, `--defaults` and `--fonts`, and the trace recorded in node
or a browser is byte-identical to the native binary's for every scenario.

## Rules

- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
- No unsafe code in the engine; the web and Android crates are the boundary and allow it for the
  exports, the imports and the handles. Scripts are untrusted input: every loop, stack and allocation the VM does
  has a limit, and a skin that breaks a limit stops cleanly (a handler that runs too long or
  too deep is cut off and logged; a script that fails twenty times is switched off).
- The engine stays platform-free: no file, screen, font, clock or thread inside the crate.
  Those come in through the host trait and the clock the host passes.
- `tools/gate.sh` before every push; the lefthook pre-push hook runs it. CI
  (`.github/workflows/ci.yml`) is manual-only, to keep Actions minutes near zero.
- Commit subjects are Conventional Commits, written for someone who reads release notes.
