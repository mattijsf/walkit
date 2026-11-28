// SPDX-License-Identifier: GPL-3.0-or-later

//! What a host pays per tick for a skin playing: the engine's tick and its draw lists, stepped
//! the way a desktop host steps it, on a stepped clock so every run does the same work.
//!
//! ```text
//! cargo run --release -p walkit-trace --example tickcost -- <skin> [--wasabi <zip>]
//!     [--fonts <dir>] [--seconds <n>] [--tracks <n>] [--frame <ms>] [--rounds <n>]
//!     [--song <ms>] [--unnumbered] [--all-windows]
//! ```
//!
//! The player builds its playlist anew on every read, as a host's player does. `--song` is the
//! song's length (4 minutes; a short one moves the seek bar every tick). `--unnumbered` gives
//! the player no playlist version. `--all-windows` takes every window's drawing
//! after a tick that changed something (`draw_windows_owned`) instead of asking which windows
//! changed (`changed_windows`, `draw_changed_windows`).

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use walkit::files::{LayeredFiles, SkinFiles};
use walkit::host::{Clock, SkinSetup};
use walkit::player::{FakePlayer, Track};
use walkit::runtime::WindowChanges;
use walkit::SkinRuntime;
use walkit_trace::{archive, TraceHost};

const START: i64 = 1_792_324_800_000;

struct Shared(Rc<dyn SkinFiles>);

impl SkinFiles for Shared {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

struct Options {
    skin: PathBuf,
    wasabi: Option<PathBuf>,
    fonts: Option<PathBuf>,
    seconds: i64,
    tracks: usize,
    frame: i64,
    rounds: usize,
    song_ms: i32,
    numbered: bool,
    all_windows: bool,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let mut o = Options {
        skin: PathBuf::new(),
        wasabi: None,
        fonts: None,
        seconds: 60,
        tracks: 200,
        frame: 25,
        rounds: 3,
        song_ms: 240_000,
        numbered: true,
        all_windows: false,
    };
    while let Some(a) = args.next() {
        let mut value = || args.next().expect("a value");
        match a.as_str() {
            "--wasabi" => o.wasabi = Some(value().into()),
            "--fonts" => o.fonts = Some(value().into()),
            "--seconds" => o.seconds = value().parse().expect("seconds"),
            "--tracks" => o.tracks = value().parse().expect("tracks"),
            "--frame" => o.frame = value().parse().expect("frame"),
            "--rounds" => o.rounds = value().parse().expect("rounds"),
            "--song" => o.song_ms = value().parse().expect("song"),
            "--unnumbered" => o.numbered = false,
            "--all-windows" => o.all_windows = true,
            _ => o.skin = a.into(),
        }
    }
    o
}

struct Cost {
    ticks: u64,
    tick_us: f64,
    draw_us: f64,
    windows: u64,
}

fn run(o: &Options) -> Cost {
    let mut files: Box<dyn SkinFiles> = Box::new(archive::read_wal(&o.skin).expect("the skin"));
    if let Some(w) = &o.wasabi {
        files = Box::new(LayeredFiles::new(
            files,
            Box::new(archive::read_wal(w).expect("wasabi")),
        ));
    }
    let files: Rc<dyn SkinFiles> = Rc::from(files);
    let fonts = o.fonts.as_deref().and_then(archive::fonts);
    let out = Rc::new(std::cell::RefCell::new(String::new()));
    let host = TraceHost::new(files.clone(), out, archive::decoder(), fonts);
    let now = Rc::new(Cell::new(START));
    let clock_now = now.clone();
    let clock: Clock = Rc::new(move || clock_now.get());
    let mut player = FakePlayer::new(clock.clone());
    player.length_ms = o.song_ms;
    player.numbered = o.numbered;
    player.playlist = (0..o.tracks)
        .map(|i| Track {
            title: format!("Artist {i} - A song with a fairly long title, number {i}"),
            length_ms: 180_000 + i as i32 * 1000,
        })
        .collect();
    let mut rt = SkinRuntime::new(
        Box::new(Shared(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup {
            skin_name: o
                .skin
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            host_actions: true,
            ..SkinSetup::default()
        },
        clock,
    );
    rt.set_viewport(1920, 1080);
    rt.load();
    rt.open_window("equalizer");
    rt.open_window("pledit");
    rt.tick();
    let mut presentation = rt.presentation();
    presentation.free_windows = true;
    presentation.viewport = (1920, 1080);
    rt.set_presentation(&presentation);
    rt.tick();
    let _ = rt.draw_windows_owned();
    rt.player().play();

    let mut tick_ns = 0u128;
    let mut draw_ns = 0u128;
    let mut ticks = 0u64;
    let mut windows = 0u64;
    let mut kept: BTreeMap<u32, Rc<Vec<walkit::draw::DrawCmd>>> = BTreeMap::new();
    let until = now.get() + o.seconds * 1000;
    while now.get() < until {
        now.set(now.get() + o.frame);
        let began = Instant::now();
        let ticked = rt.tick();
        tick_ns += began.elapsed().as_nanos();
        ticks += 1;
        if !ticked {
            continue;
        }
        let began = Instant::now();
        if o.all_windows {
            if rt.take_placed_only().is_none() {
                let ids = rt.drawn_window_ids();
                for (id, d) in ids.into_iter().zip(rt.draw_windows_owned()) {
                    // a host draws the windows whose list is another
                    if kept.get(&id.0).is_none_or(|k| !Rc::ptr_eq(k, &d.cmds)) {
                        windows += 1;
                        kept.insert(id.0, d.cmds);
                    }
                }
            }
        } else {
            match rt.changed_windows() {
                WindowChanges::Nothing => {}
                WindowChanges::Placed => {
                    let _ = rt.take_placed_only();
                }
                WindowChanges::Windows(_) | WindowChanges::All => {
                    for (id, d) in rt.draw_changed_windows() {
                        windows += 1;
                        kept.insert(id.0, d.cmds);
                    }
                }
            }
        }
        draw_ns += began.elapsed().as_nanos();
    }
    Cost {
        ticks,
        tick_us: tick_ns as f64 / ticks as f64 / 1000.0,
        draw_us: draw_ns as f64 / ticks as f64 / 1000.0,
        windows,
    }
}

fn main() {
    let o = options();
    for _ in 0..o.rounds {
        let c = run(&o);
        println!(
            "{}: {} ticks, {} windows drawn; per tick: tick {:.1} us, draw lists {:.1} us, both {:.1} us",
            o.skin.file_name().unwrap_or_default().to_string_lossy(),
            c.ticks,
            c.windows,
            c.tick_us,
            c.draw_us,
            c.tick_us + c.draw_us
        );
    }
}
