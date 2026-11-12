---
title: Introduction
---

walkit is a Winamp skin engine, as a library. It reads a skin, runs the skin's scripts, lays its
windows out and hands the host a draw list for each frame.

It reads both kinds of Winamp skin:

- **Modern skins** (`.wal`): a zip of XML, images and compiled MAKI scripts, as Winamp 3 and
  Winamp 5 load them.
- **Classic skins** (`.wsz`): the bitmaps of Winamp 2. The engine wraps them in a generated
  layout and draws the main window, the equalizer, the playlist editor, their shade bars, the
  analyzer and the oscilloscope as Winamp 5.666 or Winamp 2.91 draws them.

The host draws the list on whatever it has, and supplies the images, fonts, touch input and
player. The engine draws nothing, reads no file and plays no sound.

walkit is written in Rust and builds for Android, the web (wasm) and the desktop. It is the skin
engine of the [Andamp](https://andamp.nl) music player.

## Where to start

| You build | Read |
| --- | --- |
| A Rust host | [Rust](/walkit/rust/) and the [API reference](/walkit/api-reference/) |
| A web page | [Web](/walkit/web/) |
| An Android app | [Android](/walkit/android/) |
| A macOS app | The [macOS example](/walkit/examples/#macos): a SwiftUI app over the [Rust](/walkit/rust/) engine, behind a small C interface |
| A renderer for another platform | [The draw list](/walkit/draw-list/) |
| Changes to the engine | [Traces](/walkit/traces/) and [Contributing](/walkit/contributing/) |

## Status

Early. The engine crate is on [crates.io](https://crates.io/crates/walkit); the AAR is not
released yet, so build it from the [repository](https://github.com/mattijsf/walkit).

## License

GPL-3.0-or-later.
