# A macOS app

A small SwiftUI app that shows a Winamp skin with the `walkit` crate. It follows the
[Examples](https://mattijsf.github.io/walkit/examples/) and [Rust](https://mattijsf.github.io/walkit/rust/)
guides.

```sh
cd examples/macos
xcodebuild -project WalkitExample.xcodeproj -scheme WalkitExample -derivedDataPath build build
open build/Build/Products/Debug/walkit.app
```

Or open `WalkitExample.xcodeproj` in Xcode and run it. The build needs Xcode and Rust (rustup's
`cargo` in `~/.cargo/bin`): a build phase runs [`rust/build.sh`](rust/build.sh), which builds the
Rust library for the architectures Xcode builds and puts it at `rust/target/macos/`.

## What it does

"Winamp classic" and "Winamp Modern" download Winamp's own skins from the Winamp Skin Museum;
when a download fails, the app shows the original skin of the same kind from
[`../skins`](../skins), built into the app. Winamp's base skin opens on start; "walkit classic"
and "walkit modern" open the original skins directly. "Open skin…" opens a `.wsz` (classic) or
`.wal` (modern) of your own, as does dropping one on the window.

Winamp's base skin (or the original classic skin when the download fails) lies under every
classic skin as its [base skin](https://mattijsf.github.io/walkit/base-skin/): a skin that draws
only its main window shows Winamp's equalizer and playlist. "Wasabi base…" takes a zip of
Winamp's `wasabi` folder, which some modern skins draw lists, scrollbars and frames from
([how to make the zip](https://mattijsf.github.io/walkit/wasabi-base/)). The stand-in fonts in
`../skins/fonts`, built into the app, lie under every skin.

The skin's windows are set out apart, as on Winamp's desktop: drag one by its title bar
anywhere in the app's window, and it docks to the others' edges. The app's window resizes, down
to the size the skin's windows take together, and zooms in whole steps once it is twice that
size or more, so the pixels stay sharp. The equalizer and the playlist open with the skin. The engine's demo player answers the skin's
buttons and sliders: play, pause, stop, seek, volume, pan, the equalizer, shuffle and repeat.

From a terminal, the app takes the skin to open first, and can write its window as a PNG:

```sh
APP=build/Build/Products/Debug/walkit.app/Contents/MacOS/walkit
$APP --modern                      # Winamp Modern
$APP --builtin                     # the original classic skin; with --modern, the modern one
$APP <skin.wsz|skin.wal> [--wasabi <wasabi-base.zip>]
$APP --modern --screenshot out.png # writes the window once the skin shows, and quits
```

## How it fits together

The engine is Rust; the app calls it through a small C interface.

- [`rust/`](rust) is a static library with its own workspace and lock file, apart from the
  repository's crates.
  - `src/lib.rs` is the C interface ([`include/walkit_macos.h`](rust/include/walkit_macos.h)):
    it unzips the skin, lays it over the base skin or the wasabi base and the fonts, runs the
    engine with its demo player and its windows set out apart, and draws frames.
  - `src/host.rs` is the engine's host: pictures decoded with `image`, text measured with the
    engine's `truetype` feature.
  - `src/render.rs` draws the draw list into a buffer of pixels: pictures, fills, clips,
    window shapes and text, its letters drawn by the `truetype` feature as Windows draws them.
- [`WalkitExample/`](WalkitExample) is the SwiftUI app.
  - `ContentView.swift`: the buttons, the file pickers and the command line.
  - `SkinLibrary.swift`: the downloads, the fallbacks, and what lies under each skin.
  - `SkinView.swift`: the space the skin's windows move in, a tick on every display frame,
    each new frame shown as a picture at the window's zoom, and the mouse passed to the engine.
  - `Skin.swift`: the Swift face of the C interface.

`WalkitExample.xcodeproj` is made from [`project.yml`](project.yml) with
[XcodeGen](https://github.com/yonaskolb/XcodeGen) (`xcodegen generate`); both are committed, so
building needs only Xcode and Rust.
