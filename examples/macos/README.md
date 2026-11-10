# A macOS app

A small desktop app that shows a Winamp skin with the `walkit` crate: it unzips the skin,
decodes its pictures with `image`, draws text with `fontdue`, renders the engine's draw list in
software into a `winit` window through `softbuffer`, and passes the mouse back to the engine.

```sh
cargo run --release -- <skin.wsz|skin.wal> [--defaults <wasabi-defaults.zip>]
```

A modern skin (`.wal`) draws lists, scrollbars and frames it does not define from Winamp's
default element set: pass it as a zip of the `Plugins/freeform/xml/wasabi` folder with
`--defaults`. A classic skin (`.wsz`) needs none. Drop another skin on the window to open it.

`--screenshot <out.png>` draws a second of frames without a window, writes the last one and
exits; `--scale <n>` sets its size (2 by default).

- `src/main.rs` reads the skin, runs the engine and the window.
- `src/host.rs` is the engine's host: pictures, fonts, text measures.
- `src/render.rs` draws the draw list: pictures, fills, text, clips and window shapes.

The example has its own workspace and lock file, apart from the repository's crates.
