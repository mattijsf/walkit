---
title: Rust
---

The engine is the `walkit` crate. It has no dependencies. Until it is released, add it from the
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
    let defaults = MemoryFiles::new(); // the entries of Winamp's wasabi folder, for a modern skin

    let start = Instant::now();
    let clock: Clock = Rc::new(move || start.elapsed().as_millis() as i64);

    let mut rt = SkinRuntime::new(
        Box::new(LayeredFiles::new(Box::new(skin), Box::new(defaults))),
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

## Files

[`MemoryFiles`](/walkit/api/walkit/files/struct.MemoryFiles.html) holds a skin's entries by path. The
engine reads no archive itself, so the host unzips the skin with any zip library.

[`LayeredFiles`](/walkit/api/walkit/files/struct.LayeredFiles.html) puts a skin over Winamp's default
element set: paths under `wasabi-base/` read from the base, everything else from the skin.
Winamp draws lists, scrollbars and frames a skin does not define from that set.

A classic skin needs no base. The engine recognizes it by its `main.bmp` and generates its
layout.

## The clock

Every timer, animation and script reads the [`Clock`](/walkit/api/walkit/host/type.Clock.html), in
milliseconds. A host passes the wall clock. A test steps one, so animations and timers land on
the same frame every run.
