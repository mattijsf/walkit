# Examples

A small app for each host. Each one opens a skin, draws its windows, plays a made-up track and
passes the pointer to the engine; each builds on its own, outside the repository's workspace.
The [Examples](https://mattijsf.github.io/walkit/examples/) guide page describes them.

- [`web/`](web): a page on the `walkit-web` package, one canvas per window.
- [`android/`](android): an Android app on the AAR.
- [`macos/`](macos): a SwiftUI app for macOS, over a small Rust library that wraps the
  `walkit` crate behind a C interface. Build it with `xcodebuild` or open
  `WalkitExample.xcodeproj` in Xcode.
- [`skins/`](skins): the two original skins every example shows when it cannot download
  Winamp's, and the free fonts that stand in for Arial and Tahoma.

Each example has the same buttons: "Winamp classic" and "Winamp Modern" (downloaded from the
Winamp Skin Museum, with the original skin of the same kind when the download fails), "walkit
classic" and "walkit modern" (the original skins), a picker for a skin of your own, and "Wasabi
base" for a zip of Winamp's `wasabi` folder. Winamp's base skin lies under every classic skin,
the stand-in fonts under every skin, and the equalizer and playlist open with the skin.
