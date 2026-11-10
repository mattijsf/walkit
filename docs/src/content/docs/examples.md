---
title: Examples
---

The repository has a small app for each host in
[`examples/`](https://github.com/mattijsf/walkit/tree/main/examples). Each one opens a skin you
pick, draws its windows, plays a made-up track and passes the pointer to the engine. They follow
the host guides and stay short enough to read in one sitting.

Each one shows Winamp's own skins, the base skin and Winamp Modern, downloaded from the
[Winamp Skin Museum](https://skins.webamp.org/). Those are Winamp's art, so the repository
doesn't ship them. When a download fails, an example shows one of the two original skins in
[`examples/skins`](https://github.com/mattijsf/walkit/tree/main/examples/skins) instead, a
classic one and a modern one. Each example also has a button or key that opens these directly.

Each example puts the free fonts that stand in for Windows' Arial and Tahoma, from
`examples/skins/fonts`, under every skin, where the engine looks for them. A classic playlist
then sets its rows in Arial's widths, as Winamp does.

Any other `.wsz` or `.wal` opens too. Under every classic skin, each example puts Winamp's base
skin (the original classic skin when the download fails) as its [base skin](/walkit/base-skin/),
so a skin that draws only its main window shows Winamp's equalizer and playlist. A modern skin
that borrows frames, scrollbars or lists from Winamp's [wasabi base](/walkit/wasabi-base/) also
needs it, as a zip: each example takes one with its "Wasabi base" button.

## Web

[`examples/web`](https://github.com/mattijsf/walkit/tree/main/examples/web) is a page on the
`walkit-web` package, as the [Web](/walkit/web/) guide describes: one canvas per window, laid out
apart as on a desktop.

```bash
tools/web-pack.sh examples/web/walkit-web
python3 -m http.server -d examples 8000
# then open http://127.0.0.1:8000/web/
```

## Android

[`examples/android`](https://github.com/mattijsf/walkit/tree/main/examples/android) is an app on
the AAR, as the [Android](/walkit/android/) guide describes: buttons for the two Winamp skins,
an "Open skin" button, a "Wasabi base" button, and a view that runs the skin, draws every command on a `Canvas` and passes touches
to the engine. It builds the library from source through an `includeBuild`, so it needs the same
toolchain.

```bash
cd examples/android
./gradlew assembleDebug
```

## macOS

[`examples/macos`](https://github.com/mattijsf/walkit/tree/main/examples/macos) is a SwiftUI app
on the `walkit` crate, as the [Rust](/walkit/rust/) guide describes. A small Rust library in the
example wraps the engine behind a C interface: it unzips the skin, decodes its pictures with
`image`, measures and draws text with `fontdue`, and renders the draw list in software, window
shapes included. The Swift side has the same buttons as the Android app ("Winamp classic",
"Winamp Modern", "walkit classic", "walkit modern", "Open skin", "Wasabi base"), downloads the
skins, shows each frame and passes the mouse to the engine. It needs Xcode and Rust; the Xcode
project builds the library with Cargo.

```bash
cd examples/macos
xcodebuild -project WalkitExample.xcodeproj -scheme WalkitExample -derivedDataPath build build
open build/Build/Products/Debug/walkit.app
```

Or open `WalkitExample.xcodeproj` in Xcode and run it. The skin's windows are set out apart, as
on the web: drag one by its title bar anywhere in the app's window, which resizes. Drop another
skin on the window to open it.
