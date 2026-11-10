# A macOS app

A small desktop app that shows a Winamp skin with the `walkit` crate: it unzips the skin,
decodes its pictures with `image`, draws text with `fontdue`, renders the engine's draw list in
software into a `winit` window through `softbuffer`, and passes the mouse back to the engine.

```sh
cargo run --release                   # Winamp's base skin
cargo run --release -- --modern       # Winamp Modern
cargo run --release -- --builtin      # the built-in classic skin; with --modern, the modern one
cargo run --release -- <skin.wsz|skin.wal> [--wasabi <wasabi-base.zip>]
```

With no skin file, the app downloads Winamp's base skin or Winamp Modern from the Winamp Skin
Museum. When the download fails, it shows the original skin of the same kind from
[`../skins`](../skins), built into the app. In the window, keys 1 and 2 open Winamp's base skin
and Winamp Modern, 3 and 4 the built-in classic and modern skins.

A modern skin (`.wal`) draws lists, scrollbars and frames it does not define from Winamp's
[wasabi base](https://mattijsf.github.io/walkit/wasabi-base/): pass its zip with `--wasabi`.
Under a classic skin (`.wsz`) the app puts Winamp's base skin (the built-in classic skin when
the download fails) as its [base skin](https://mattijsf.github.io/walkit/base-skin/): a skin
that draws only its main window shows Winamp's equalizer and playlist. Drop another skin on the
window to open it.

`--screenshot <out.png>` draws a second of frames without a window, writes the last one and
exits; `--scale <n>` sets its size (2 by default).

- `src/main.rs` reads the skin, runs the engine and the window.
- `src/host.rs` is the engine's host: pictures, fonts, text measures.
- `src/render.rs` draws the draw list: pictures, fills, text, clips and window shapes.

The example has its own workspace and lock file, apart from the repository's crates.
