# walkit

A Winamp skin engine, as a library. It reads a modern skin (a `.wal` file: a zip of XML, images
and compiled MAKI scripts) or a classic skin (a `.wsz` file: the bitmaps of Winamp 2), runs the
skin's scripts, lays its windows out and hands the host a draw list for each frame.

The host draws that list on whatever it has, and supplies the images, fonts, touch input and
player. The engine draws nothing, reads no file and plays no sound.

walkit is written in Rust and builds for Android, the web (wasm) and the desktop. It is the skin
engine of the [Andamp](https://andamp.nl) music player.

A classic skin loads the same way as a modern one: the engine wraps its bitmaps in a generated
layout. It draws the main window, the equalizer, the playlist editor, their shade bars, the
analyzer and the oscilloscope as Winamp 5.666 or Winamp 2.91 draws them.

## Status

Early. The engine crate is on [crates.io](https://crates.io/crates/walkit); no AAR is released yet.

A scripted run writes a trace of everything the engine does. Traces of a change are compared
line by line with those of the last release (see [ENGINEERING.md](ENGINEERING.md)).

## Building

```bash
cargo build --release
tools/gate.sh            # formatting, clippy, the tests
tools/web-build.sh       # the wasm modules (needs the wasm32-unknown-unknown target from rustup)
```

[android/README.md](android/README.md) covers the Android library and
[web/README.md](web/README.md) the web build.

## Examples

[`examples/`](examples/) has a small app for each host: a web page on the `walkit-web` package,
an Android app on the AAR, and a SwiftUI app for macOS on the crate.

## Documentation

The guide and the API reference are at <https://mattijsf.github.io/walkit/>. Their sources are
in `docs/` (a Starlight site) and the engine's doc comments.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
