# walkit

The engine of a modern Winamp skin, as a library. A modern skin (a `.wal` file) is a zip of
XML, images and compiled MAKI scripts. walkit reads it, runs the scripts, lays the windows
out and hands the host a draw list for each frame. The host draws it, on whatever it has: a
canvas, a view tree, a GPU. The host also owns the images, the fonts, the touch input and
the player; the engine never touches a screen, a file or a sound.

It is written in Rust, with no unsafe code, and builds for Android, the web (wasm) and the
desktop. It is the engine behind [Andamp](https://andamp.nl)'s modern skins.

## Status

Early. The engine is ported from Andamp's Kotlin engine and checked by a trace of everything
each engine does in a scripted run (see `ENGINEERING.md`): every scenario of the reference set
(45 runs over 12 skins, scripts included) records the same trace in both engines. Nothing is
published yet.

## Building

```bash
cargo build --release
tools/gate.sh            # formatting, clippy, the tests
```

## License

GPL-3.0-or-later. See `LICENSE`.
