---
title: Examples
---

The repository has a small app for each host in
[`examples/`](https://github.com/mattijsf/walkit/tree/main/examples). Each one opens a skin you
pick, draws its windows, plays a made-up track and passes the pointer to the engine. They follow
the host guides and stay short enough to read in one sitting.

Skins are other people's work, so none ships with the examples. A classic `.wsz` skin opens on
its own. A modern `.wal` skin also needs Winamp's default element set, a zip of its `wasabi`
folder.

## Web

[`examples/web`](https://github.com/mattijsf/walkit/tree/main/examples/web) is a page on the
`walkit-web` package, as the [Web](/walkit/web/) guide describes: one canvas per window, laid out
apart as on a desktop.

```bash
tools/web-pack.sh examples/web/walkit-web
python3 -m http.server -d examples/web 8000
# then open http://127.0.0.1:8000/
```

## Android

[`examples/android`](https://github.com/mattijsf/walkit/tree/main/examples/android) is an app on
the AAR, as the [Android](/walkit/android/) guide describes: an "Open skin" button, a "Default
set" button, and a view that runs the skin, draws every command on a `Canvas` and passes touches
to the engine. It builds the library from source through an `includeBuild`, so it needs the same
toolchain.

```bash
cd examples/android
./gradlew assembleDebug
```

## macOS

[`examples/macos`](https://github.com/mattijsf/walkit/tree/main/examples/macos) is a Rust app on
the `walkit` crate, as the [Rust](/walkit/rust/) guide describes: it unzips the skin, decodes its
pictures with `image`, measures and draws text with `fontdue`, and renders the draw list in
software into a `winit` window, window shapes included.

```bash
cd examples/macos
cargo run --release -- <skin.wsz|skin.wal> [--defaults <wasabi-defaults.zip>]
```

Drop another skin on the window to open it.
