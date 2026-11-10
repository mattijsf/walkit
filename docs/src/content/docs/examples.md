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

The web and Android examples put the free fonts that stand in for Windows' Arial and Tahoma,
from `examples/skins/fonts`, under every skin, where the engine looks for them; the macOS one
draws those faces with the system's own Arial and Tahoma. A classic playlist then sets its rows
in Arial's widths, as Winamp does.

Any other `.wsz` or `.wal` opens too. A modern skin that borrows frames, scrollbars or lists
from Winamp's [wasabi base](/walkit/wasabi-base/) also needs it, as a zip: each example takes
one ("Wasabi base", or `--wasabi` on macOS).

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

[`examples/macos`](https://github.com/mattijsf/walkit/tree/main/examples/macos) is a Rust app on
the `walkit` crate, as the [Rust](/walkit/rust/) guide describes: it unzips the skin, decodes its
pictures with `image`, measures and draws text with `fontdue`, and renders the draw list in
software into a `winit` window, window shapes included.

```bash
cd examples/macos
cargo run --release                  # Winamp's base skin; --modern for Winamp Modern
cargo run --release -- <skin.wsz|skin.wal> [--wasabi <wasabi-base.zip>]
```

Drop another skin on the window to open it.
