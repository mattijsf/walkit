# walkit

The engine of a modern Winamp skin, as a library. A modern skin (a `.wal` file) is a zip of
XML, images and compiled MAKI scripts. walkit reads it, runs the scripts, lays the windows
out and hands the host a draw list for each frame. The host draws it, on whatever it has: a
canvas, a view tree, a GPU. The host also owns the images, the fonts, the touch input and
the player; the engine never touches a screen, a file or a sound.

It is written in Rust, with no unsafe code, and builds for Android, the web (wasm) and the
desktop. It is the engine behind [Andamp](https://andamp.nl)'s modern skins.

It also draws a classic skin (a `.wsz` file: the bitmaps of Winamp 2): the main window, the
equalizer and the playlist editor, their shade bars, the analyzer and oscilloscope, and their
buttons, sliders and lists, each painted as Winamp 5.666 paints it and checked pixel
for pixel against it. A classic skin loads like a modern one; the engine wraps its bitmaps in
a generated layout and the host draws the same draw list.

## Status

Early. The engine is ported from Andamp's Kotlin engine and checked by a trace of everything
each engine does in a scripted run (see `ENGINEERING.md`): every scenario of the reference set
(45 runs over 12 skins, scripts included) records the same trace in both engines, natively and
through the wasm build in node, Chrome, Safari and Firefox, and `web/player.html` plays a skin
in the browser on the same frames the Android library draws. Nothing is published yet.

The reference set is local: the skins are the `.wal` files of Andamp's `modernskin/src/test/
resources`, the default element set a zip of Winamp's `wasabi` folder, and the fonts the
default-set fonts folder of an Andamp checkout at current main (DejaVu Sans Condensed and
Liberation Sans). None of it is in this repository.

## Building

```bash
cargo build --release
tools/gate.sh            # formatting, clippy, the tests
tools/web-build.sh       # the wasm module, both profiles (needs the wasm32 target of a rustup toolchain)
```

## License

GPL-3.0-or-later. See `LICENSE`.
