//! Times the frames a host draws while a skin plays: the engine's tick and drawing, the
//! planner and the frame writer, each separately.
//!
//!   cargo run --release -p walkit-trace --example frames -- <skin> [wasabi-base.zip] [fonts dir] [seconds]

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use walkit::files::SkinFiles;
use walkit::host::SkinSetup;
use walkit::packed::{FrameWindow, FrameWriter};
use walkit::plan::DrawPlanner;
use walkit::player::{FakePlayer, Track};
use walkit::SkinRuntime;
use walkit_trace::archive::{decoder, fonts, read_wal};
use walkit_trace::host::TraceHost;

struct FilesHandle(Rc<dyn SkinFiles>);

impl SkinFiles for FilesHandle {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let skin = Path::new(&args[0]);
    let mut files: Box<dyn SkinFiles> = Box::new(read_wal(skin).expect("the skin"));
    if let Some(d) = args.get(1).filter(|a| !a.is_empty()) {
        files = Box::new(walkit::files::LayeredFiles::new(
            files,
            Box::new(read_wal(Path::new(d)).expect("the wasabi base")),
        ));
    }
    let font_files = args
        .get(2)
        .filter(|a| !a.is_empty())
        .and_then(|d| fonts(Path::new(d)));
    let seconds: i64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(20);
    let files: Rc<dyn SkinFiles> = Rc::from(files);
    let host = TraceHost::new(
        files.clone(),
        Rc::new(RefCell::new(String::new())),
        decoder(),
        font_files,
    );
    let now = Rc::new(Cell::new(1_000_000i64));
    let clock_now = now.clone();
    let clock: walkit::host::Clock = Rc::new(move || clock_now.get());
    let mut player = FakePlayer::new(clock.clone());
    player.title = "DJ Mike Llama - Llama Whippin' Intro".into();
    player.length_ms = 300_000;
    player.playlist = (0..12)
        .map(|i| Track {
            title: format!("Artist {i} - A song with a fairly long title {i}"),
            length_ms: 200_000,
        })
        .collect();
    let mut rt = SkinRuntime::new(
        Box::new(FilesHandle(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup {
            skin_name: skin.file_name().unwrap().to_string_lossy().into(),
            have_library: false,
            host_actions: false,
        },
        clock,
    );
    rt.set_viewport(1280, 900);
    rt.load();
    for c in ["equalizer", "pledit"] {
        rt.open_window(c);
    }
    rt.player().play();
    let mut planner = DrawPlanner::new();
    let mut writer = FrameWriter::new();
    let (mut tick, mut draw, mut plan, mut encode, mut frames, mut bytes) =
        (0.0, 0.0, 0.0, 0.0, 0, 0usize);
    let end = now.get() + seconds * 1000;
    while now.get() < end {
        now.set(now.get() + 16);
        let t0 = Instant::now();
        let changed = rt.tick();
        let t1 = Instant::now();
        tick += (t1 - t0).as_secs_f64();
        if !changed {
            continue;
        }
        frames += 1;
        let ids = rt.drawn_window_ids();
        let moving = rt.take_tick_changed();
        let drawn = rt.draw_windows_owned();
        let t2 = Instant::now();
        let planned = planner.plan(&ids, drawn, &moving, now.get());
        let t3 = Instant::now();
        let infos: Vec<_> = planned.iter().map(|w| rt.window_info(w.id)).collect();
        let windows: Vec<FrameWindow> = planned
            .iter()
            .zip(infos.iter())
            .map(|(w, info)| FrameWindow {
                id: w.id,
                container: &info.container,
                layout: &info.layout,
                rect: w.rect,
                cmds: &w.cmds,
                plan: &w.plan,
            })
            .collect();
        let t4 = Instant::now();
        bytes += writer.encode(&windows).len();
        let t5 = Instant::now();
        draw += (t2 - t1).as_secs_f64();
        plan += (t3 - t2).as_secs_f64();
        encode += (t5 - t4).as_secs_f64();
    }
    let per_s = |v: f64| v * 1000.0 / seconds as f64;
    println!(
        "{}: {frames} frames in {seconds} s; ms per second of play: tick {:.2}, draw {:.2}, plan {:.2}, encode {:.2}; {} bytes per frame",
        skin.file_name().unwrap().to_string_lossy(),
        per_s(tick),
        per_s(draw),
        per_s(plan),
        per_s(encode),
        bytes / frames.max(1)
    );
}
