---
title: Rust
---

The engine is the `walkit` crate. It has no dependencies unless the host asks for the
[`truetype`](#text) feature. Until it is released, add it from the
repository:

```toml
[dependencies]
walkit = { git = "https://github.com/mattijsf/walkit" }
```

## A host

A host implements [`SkinHost`](/walkit/api/walkit/host/trait.SkinHost.html) for pictures, text and
logging, and [`Player`](/walkit/api/walkit/player/trait.Player.html) for the transport.
[`FakePlayer`](/walkit/api/walkit/player/struct.FakePlayer.html) plays nothing and is enough for a first
look at a skin.

```rust
use std::rc::Rc;
use std::time::Instant;

use walkit::draw::{DrawCmd, FontDef};
use walkit::files::{LayeredFiles, MemoryFiles};
use walkit::host::{Clock, SkinHost, SkinSetup};
use walkit::player::FakePlayer;
use walkit::SkinRuntime;

struct Host;

impl SkinHost for Host {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        todo!("decode the picture and return its size")
    }
    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        todo!("one pixel, ARGB; override `pixels` to read a block at once")
    }
    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        todo!("measure with the host's font library")
    }
    fn log(&mut self, message: &str) {
        eprintln!("{message}");
    }
    fn invalidate(&mut self) {
        // something changed: tick and draw soon
    }
}

fn main() {
    let mut skin = MemoryFiles::new();
    // insert each entry of the .wal or .wsz archive: skin.insert(path, bytes)
    let wasabi = MemoryFiles::new(); // the entries of Winamp's wasabi folder, for a modern skin

    let start = Instant::now();
    let clock: Clock = Rc::new(move || start.elapsed().as_millis() as i64);

    let mut rt = SkinRuntime::new(
        Box::new(LayeredFiles::new(Box::new(skin), Box::new(wasabi))),
        Box::new(Host),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup { skin_name: "Winamp Modern".into(), ..SkinSetup::default() },
        clock,
    );
    rt.set_viewport(1080, 1920);
    rt.load();

    // every frame:
    if rt.tick() {
        for (rect, commands) in rt.draw_windows() {
            for command in commands {
                match command {
                    DrawCmd::Image { bitmap, dst, alpha, .. } => { /* draw the bitmap */ }
                    DrawCmd::Fill { rect, color, alpha } => { /* fill the box */ }
                    _ => { /* text, clips, shapes, pixels */ }
                }
            }
        }
    }

    // input, in viewport pixels; the last argument names the touch
    rt.pointer_down(100, 200, 1);
    rt.pointer_up(100, 200);
}
```

[`examples/macos`](https://github.com/mattijsf/walkit/tree/main/examples/macos) is a complete
desktop app built this way: a SwiftUI app over a small Rust library that runs the engine behind
a C interface and draws the draw list with a software renderer.

## Text

The engine places every run of text and the host draws its letters. A host with its own font
library measures and draws with that. A host that draws into a buffer of pixels can take the
engine's own letters instead, with the `truetype` feature (it adds the `skrifa` crate):

```toml
[dependencies]
walkit = { git = "https://github.com/mattijsf/walkit", features = ["truetype"] }
```

[`walkit::truetype`](/walkit/api/walkit/truetype/index.html) draws TrueType text as Windows
draws it for Winamp, so a playlist row has the letters and the widths it has there:

- The font's own hinting fits each letter to the pixel grid.
- Letters advance by whole pixels: the width the font's `hdmx` or `LTSH` table gives for the
  size, else the hinted width. Pairs in the font's `kern` table move by whole pixels.
- Letters are smoothed only at the sizes the font's `gasp` table asks for. Arial from 9 to 17
  pixels, the classic playlist's font, is not smoothed.
- A stroke thinner than a pixel that passes between two pixel centers still lights one pixel.

```rust
use walkit::truetype::{pixels, Font};

let font = Font::parse(bytes).expect("a TrueType file"); // once per font file
let px = pixels(size);                                    // a skin's font size in whole pixels

// SkinHost::text_width and text_advance; `fake_bold` is FontFace's
let width = font.width(text, px, fake_bold);
// SkinHost::line_metrics
let (ascent, descent) = font.line(px);

// DrawCmd::Text: start on a whole pixel, then one bitmap per letter
let (x, baseline) = (cmd.x.round() as i32, cmd.baseline.round() as i32);
for (at, glyph) in font.run(&cmd.text, px) {
    // glyph.coverage is glyph.w x glyph.h bytes, 255 the text's color;
    // its corner is (x + at + glyph.left, baseline - glyph.top)
}
```

A face the engine marks `fake_bold` has no bold cut: draw the run twice, one pixel apart. At
a scale above one, ask for `pixels(size) * scale` and multiply the pen's place by the scale.
The macOS example's `render.rs` draws text this way.

With Arial itself, the rows of a classic playlist come out as Winamp 5.666 draws them, pixel
for pixel but for the foot of a V. With the free fonts that stand in for Arial and Tahoma the
letters differ, as the fonts do. Sizes a font smooths (Arial from 18 pixels), Tahoma and the
text of modern skins have not been compared with Winamp yet.

## Files

[`MemoryFiles`](/walkit/api/walkit/files/struct.MemoryFiles.html) holds a skin's entries by path. The
engine reads no archive itself, so the host unzips the skin with any zip library.

[`LayeredFiles`](/walkit/api/walkit/files/struct.LayeredFiles.html) puts a skin over Winamp's
[wasabi base](/walkit/wasabi-base/): paths under `wasabi-base/` read from the base, everything
else from the skin. Winamp draws lists, scrollbars and frames a skin does not define from it.

The engine recognizes a classic skin by its `main.bmp` and generates its layout. Under a
classic skin, the base is a classic [base skin](/walkit/base-skin/) instead: Winamp's base skin,
say, unzipped into a `MemoryFiles`. The engine takes every bitmap the skin leaves out from it.

## The clock

Every timer, animation and script reads the [`Clock`](/walkit/api/walkit/host/type.Clock.html), in
milliseconds. A host passes the wall clock. A test steps one, so animations and timers land on
the same frame every run.
