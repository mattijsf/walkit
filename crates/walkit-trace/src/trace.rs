// SPDX-License-Identifier: GPL-3.0-or-later

//! A record of everything the engine does in a scenario, as text: every line the skin logs,
//! every menu, host action and window change, the script work of each tick, and the whole draw
//! list of every frame that differs from the one before. Two engines fed the same scenario
//! should write the same trace, line for line; the first line that differs is where they part.
//!
//! The record depends on nothing a platform measures: the host measures text by a fixed rule
//! and takes image sizes and pixels from the skin's files, so any engine can be run against it
//! anywhere. The clock starts at [`START`] and moves [`FRAME_MS`] at a time.
//!
//! Format, one event per line, `#` lines being comments:
//! - `trace v1`, `skin <name>`, `song <title>`..., `clock start=<ms> frame=<ms>`
//! - `step <what>` before each step runs
//! - `log <text>`, `menu <items>`, `hostaction <ACTION> <param>`, `windows <ids>` as the engine
//!   calls its host
//! - `tick t=<ms> steps=<n> timers=<n> layouts=<n> shapes=<n> invalidates=<n>` for each tick that
//!   did any script or layout work (t counts from the start)
//! - `frame t=<ms>` ... `endframe` for each frame that draws differently from the one before:
//!   `window <id> <container>/<layout> <rect>` then one line per draw command
//! - `tap <id> <x>,<y>` before a tap's press and release
//!
//! Rects are `left,top,width,height`; colors `#rrggbb`; fractions have two decimals.

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;

use walkit::draw::{DrawCmd, Gamma, Rect, Region, RegionImage, TextCmd};
use walkit::files::SkinFiles;
use walkit::host::{EngineStats, SkinSetup};
use walkit::player::{FakePlayer, Track};
use walkit::SkinRuntime;

use crate::host::{Decoder, TraceHost};
use crate::scenario::Scenario;

/// 2026-10-20 12:00:00 UTC, as the reference tests start.
pub const START: i64 = 1_792_324_800_000;
/// The display frame.
pub const FRAME_MS: i64 = 16;
const SONG_MS: i32 = 600_000;

/// One thing the run does.
enum Step {
    /// The clock runs on, a display frame at a time, with the engine ticked on each.
    Settle(i64),
    /// The player starts playing the current song.
    Play,
    /// The player stops.
    Stop,
    /// The color theme is put on.
    Theme(String),
    /// The window opens, as a skin opens one by itself at start.
    Show(String),
    /// The windows stack in a view, as a phone stacks them.
    View(i32, i32),
    /// The main window is resized, as a host fills its box with a resizable one.
    ResizeMain(i32, i32),
    /// A tap in the middle of the object, in the window opened last that has it.
    Tap(String),
}

/// The run the Winamp reference screenshots were taken with: start-up timers run out, the
/// music plays two seconds and stops, then each tap with three seconds after it.
fn steps(sc: &Scenario) -> Vec<Step> {
    let mut steps = Vec::new();
    if let Some(t) = &sc.theme {
        steps.push(Step::Theme(t.clone()));
    }
    if let Some((w, h)) = sc.main_size {
        steps.push(Step::ResizeMain(w, h));
    }
    if let Some((w, h)) = sc.view {
        steps.push(Step::View(w, h));
    }
    for s in &sc.show {
        steps.push(Step::Show(s.clone()));
    }
    steps.push(Step::Settle(3_000));
    steps.push(Step::Play);
    steps.push(Step::Settle(2_000));
    steps.push(Step::Stop);
    steps.push(Step::Settle(3_000));
    for id in &sc.taps {
        steps.push(Step::Tap(id.clone()));
        steps.push(Step::Settle(3_000));
    }
    steps
}

fn describe(step: &Step) -> String {
    match step {
        Step::Settle(ms) => format!("settle {ms}"),
        Step::Play => "play".into(),
        Step::Stop => "stop".into(),
        Step::Theme(n) => format!("theme {n}"),
        Step::Show(c) => format!("show {c}"),
        Step::View(w, h) => format!("view {w}x{h}"),
        Step::ResizeMain(w, h) => format!("resizemain {w}x{h}"),
        Step::Tap(id) => format!("tap {id}"),
    }
}

/// A clock for timing a run, in milliseconds of any resolution; the platform supplies it.
pub type Perf = Box<dyn Fn() -> f64>;

/// What a run took: the load, each tick, and each draw list, in milliseconds of the [`Perf`]
/// clock. `draws` holds every draw list made (one per tick); `dirty` says which of them
/// followed a tick that reported a change, as a host would draw only those.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    /// How long `load` took.
    pub load_ms: f64,
    /// Each tick's time.
    pub ticks: Vec<f64>,
    /// Each draw list's time.
    pub draws: Vec<f64>,
    /// Whether the tick before each draw reported a change.
    pub dirty: Vec<bool>,
    /// How many frames the trace holds.
    pub frames: usize,
}

/// A recorded run: the trace text, and what it took.
#[derive(Debug, Clone)]
pub struct Recording {
    /// The trace, `trace v1`.
    pub text: String,
    /// The timing.
    pub stats: Stats,
}

/// The trace of `sc` run on `files`, with pictures decoded by `decoder`, the stand-in fonts
/// read from `fonts`, and the run timed on `perf`.
pub fn record(
    sc: &Scenario,
    files: Box<dyn SkinFiles>,
    decoder: Decoder,
    fonts: Option<Rc<dyn SkinFiles>>,
    perf: Perf,
) -> Recording {
    let files: Rc<dyn SkinFiles> = Rc::from(files);
    let out = Rc::new(RefCell::new(String::new()));
    let host = TraceHost::new(files.clone(), out.clone(), decoder, fonts);
    let invalidates = host.invalidates.clone();
    let now = Rc::new(Cell::new(START));
    let clock_now = now.clone();
    let clock: walkit::host::Clock = Rc::new(move || clock_now.get());
    let mut player = FakePlayer::new(clock.clone());
    player.title = sc
        .playlist
        .first()
        .cloned()
        .unwrap_or_else(|| "Andamp".to_string());
    player.length_ms = SONG_MS;
    player.playlist = sc
        .playlist
        .iter()
        .map(|t| Track {
            title: t.clone(),
            length_ms: SONG_MS,
        })
        .collect();
    player.decoder_name = "Nullsoft Waveform Decoder v3.25".to_string();
    player.bitrate_kbps = 64;
    player.sample_rate_khz = 8;
    player.channels = 1;
    let mut rt = SkinRuntime::new(
        Box::new(FilesHandle(files)),
        Box::new(host),
        Box::new(player),
        SkinSetup {
            skin_name: sc.skin.clone(),
            have_library: sc.have_library,
            host_actions: false,
        },
        clock,
    );
    rt.set_viewport(1080, 1920);
    {
        let mut o = out.borrow_mut();
        let _ = writeln!(o, "trace v{}", walkit::TRACE_VERSION);
        let _ = writeln!(o, "skin {}", sc.skin);
        for t in &sc.playlist {
            let _ = writeln!(o, "song {t}");
        }
        let _ = writeln!(o, "clock start={START} frame={FRAME_MS}");
        let _ = writeln!(o, "step load");
    }
    let t0 = perf();
    rt.load();
    let mut stats = Stats {
        load_ms: perf() - t0,
        ..Default::default()
    };
    let mut frames = Frames {
        stats: rt.stats(),
        invalidates: 0,
        last: String::new(),
    };
    frames.tick(&mut rt, &out, &invalidates, now.get(), &perf, &mut stats);
    for step in steps(sc) {
        let _ = writeln!(out.borrow_mut(), "step {}", describe(&step));
        match step {
            Step::Settle(ms) => {
                let until = now.get() + ms;
                while now.get() < until {
                    now.set((now.get() + FRAME_MS).min(until));
                    frames.tick(&mut rt, &out, &invalidates, now.get(), &perf, &mut stats);
                }
            }
            Step::Play => rt.player().play(),
            Step::Stop => rt.player().stop(),
            Step::Theme(name) => rt.select_color_theme(&name),
            Step::Show(c) => {
                rt.open_window(&c);
            }
            Step::View(w, h) => rt.set_view_size(Some((w, h))),
            Step::ResizeMain(w, h) => {
                if let Some(m) = rt.main_window() {
                    rt.resize_window(m, w, h);
                }
            }
            Step::Tap(id) => match rt.object_info(&id) {
                None => {
                    let _ = writeln!(out.borrow_mut(), "tap {id} missing");
                }
                Some(target) => {
                    let r = target.stack_rect;
                    let x = r.left + r.width / 2;
                    let y = r.top + r.height / 2;
                    let _ = writeln!(out.borrow_mut(), "tap {id} {x},{y}");
                    rt.pointer_down(x, y, 0);
                    rt.pointer_up(x, y);
                }
            },
        }
    }
    let _ = writeln!(out.borrow_mut(), "end t={}", now.get() - START);
    let text = out.borrow().clone();
    Recording { text, stats }
}

/// A shared files handle as a boxed `SkinFiles`.
struct FilesHandle(Rc<dyn SkinFiles>);

impl SkinFiles for FilesHandle {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0.read(path)
    }

    fn names(&self) -> Vec<String> {
        self.0.names()
    }
}

/// Ticks the engine and writes what each tick did, and each frame that changed.
struct Frames {
    stats: EngineStats,
    invalidates: i64,
    last: String,
}

impl Frames {
    fn tick(
        &mut self,
        rt: &mut SkinRuntime,
        out: &Rc<RefCell<String>>,
        invalidates: &Rc<RefCell<i64>>,
        now: i64,
        perf: &Perf,
        timing: &mut Stats,
    ) {
        let t0 = perf();
        let dirty = rt.tick();
        let t1 = perf();
        let s = rt.stats();
        let t = now - START;
        let mut frame = String::new();
        let ids = rt.window_ids();
        let t2 = perf();
        let drawn = rt.draw_windows();
        let t3 = perf();
        timing.ticks.push(t1 - t0);
        timing.draws.push(t3 - t2);
        timing.dirty.push(dirty);
        assert_eq!(
            ids.len(),
            drawn.len(),
            "{} windows, {} drawn",
            ids.len(),
            drawn.len()
        );
        for (id, (_, cmds)) in ids.iter().zip(drawn.iter()) {
            let info = rt.window_info(*id);
            let _ = writeln!(
                frame,
                "window {} {}/{} {}",
                id.0,
                info.container,
                info.layout,
                rect(&rt.window_rect(*id))
            );
            for cmd in cmds {
                frame.push_str(&line(cmd));
                frame.push('\n');
            }
        }
        let changed = frame != self.last;
        let inv = *invalidates.borrow();
        let worked = s != self.stats || inv != self.invalidates;
        if worked || changed {
            let _ = writeln!(
                out.borrow_mut(),
                "tick t={t} steps={} timers={} layouts={} shapes={} invalidates={}",
                s.script_steps - self.stats.script_steps,
                s.timer_fires - self.stats.timer_fires,
                s.layout_passes - self.stats.layout_passes,
                s.shapes_worked_out - self.stats.shapes_worked_out,
                inv - self.invalidates
            );
        }
        if changed {
            let mut o = out.borrow_mut();
            let _ = writeln!(o, "frame t={t}");
            o.push_str(&frame);
            o.push_str("endframe\n");
            self.last = frame;
            timing.frames += 1;
        }
        self.stats = s;
        self.invalidates = inv;
    }
}

/// The CRC-32 (IEEE) of the pixels as bytes, each ARGB big-endian, as `java.util.zip.CRC32` sums them.
pub fn crc32_of(argb: &[u32]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for p in argb {
        for b in p.to_be_bytes() {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

/// A draw command as one line of the trace.
pub fn line(cmd: &DrawCmd) -> String {
    match cmd {
        DrawCmd::Image {
            bitmap,
            dst,
            alpha,
            gamma,
            smooth,
        } => image(&RegionImage {
            bitmap: bitmap.clone(),
            dst: *dst,
            alpha: *alpha,
            gamma: *gamma,
            smooth: *smooth,
        }),
        DrawCmd::Fill {
            rect: r,
            color: c,
            alpha,
        } => format!("fill {} {} a={alpha}", rect(r), color(c.0)),
        DrawCmd::WindowStart(r) => format!("shape {}", region(r)),
        DrawCmd::WindowEnd => "endshape".to_string(),
        DrawCmd::Text(t) => text(t),
        DrawCmd::BitmapText(b) => format!("bitmaptext {} {}", quote(&b.text), rect(&b.rect)),
        DrawCmd::Clip(r) => format!("clip {}", rect(r)),
        DrawCmd::Unclip => "unclip".to_string(),
        DrawCmd::Pixels {
            rect: r,
            w,
            h,
            argb,
            alpha,
            stable,
        } => {
            format!(
                "pixels {} {w}x{h} a={alpha} crc={:08x}{}",
                rect(r),
                crc32_of(argb),
                if *stable { " stable" } else { "" }
            )
        }
    }
}

fn image(cmd: &RegionImage) -> String {
    let b = &cmd.bitmap;
    let mut s = format!(
        "image {} {},{},{},{} -> {} a={}",
        b.file,
        b.x,
        b.y,
        b.w.map(|w| w.to_string()).unwrap_or_else(|| "-".into()),
        b.h.map(|h| h.to_string()).unwrap_or_else(|| "-".into()),
        rect(&cmd.dst),
        cmd.alpha
    );
    if let Some(g) = cmd.gamma.filter(|g| !g.is_identity()) {
        let _ = write!(s, " g={}", gamma(&g));
    }
    if cmd.smooth {
        s.push_str(" smooth");
    }
    s
}

fn text(cmd: &TextCmd) -> String {
    let mut s = format!(
        "text {} font={} size={} {} a={} {} x={} base={}",
        quote(&cmd.text),
        cmd.font
            .as_ref()
            .map(|f| f.file.to_string())
            .unwrap_or_else(|| "-".into()),
        cmd.size,
        color(cmd.color.0),
        cmd.alpha,
        rect(&cmd.rect),
        num(cmd.x),
        num(cmd.baseline)
    );
    if cmd.bold {
        s.push_str(" bold");
    }
    if let Some(sh) = cmd.shadow {
        let _ = write!(
            s,
            " shadow={} {},{}",
            color(sh.0),
            cmd.shadow_dx,
            cmd.shadow_dy
        );
    }
    s
}

fn region(r: &Region) -> String {
    match r {
        Region::Pic(img) => format!("pic({})", image(img).trim_start_matches("image ")),
        Region::Box(b) => format!("box({})", rect(b)),
        Region::Op {
            base,
            add,
            keep,
            cut,
        } => {
            let mut parts = Vec::new();
            if let Some(b) = base {
                parts.push(format!("base={}", region(b)));
            }
            let list = |rs: &[Region]| rs.iter().map(region).collect::<Vec<_>>().join(" ");
            if !add.is_empty() {
                parts.push(format!("add=[{}]", list(add)));
            }
            if !keep.is_empty() {
                parts.push(format!("keep=[{}]", list(keep)));
            }
            if !cut.is_empty() {
                parts.push(format!("cut=[{}]", list(cut)));
            }
            format!("op({})", parts.join(" "))
        }
    }
}

fn gamma(g: &Gamma) -> String {
    format!("{},{},{},{},{}", g.r, g.g, g.b, g.boost, g.gray)
}

fn rect(r: &Rect) -> String {
    format!("{},{},{},{}", r.left, r.top, r.width, r.height)
}

fn color(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0xFFFFFF)
}

/// A float with two decimals, rounded half up as Java's `%.2f` rounds.
fn num(f: f32) -> String {
    let v = f as f64;
    let scaled = v * 100.0;
    let rounded = if scaled.fract().abs() == 0.5 {
        scaled.trunc() + scaled.signum()
    } else {
        scaled.round()
    };
    let r = rounded / 100.0;
    format!("{r:.2}")
}

fn quote(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkit::files::MemoryFiles;

    /// A main window with the song's title and a play button, and nothing a file would draw.
    fn small() -> Box<dyn SkinFiles> {
        let mut m = MemoryFiles::new();
        m.insert(
            "skin.xml",
            br#"<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>"#.to_vec(),
        );
        m.insert(
            "player.xml",
            concat!(
                r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">"#,
                r#"<text id="title" display="songtitle" x="10" y="10" w="180" h="20" color="255,255,255"/>"#,
                r#"<text id="time" display="time" x="10" y="40" w="80" h="20" color="0,255,0"/>"#,
                r#"<button id="play" action="PLAY" x="100" y="40" w="40" h="20"/>"#,
                r#"</layout></container>"#
            )
            .as_bytes()
            .to_vec(),
        );
        Box::new(m)
    }

    fn rec(sc: &Scenario) -> String {
        record(sc, small(), Box::new(|_, _| None), None, Box::new(|| 0.0)).text
    }

    fn scenario(title: &str, taps: &[&str]) -> Scenario {
        Scenario {
            name: "small".into(),
            wal: "small.wal".into(),
            skin: "Small".into(),
            playlist: vec![title.to_string()],
            theme: None,
            view: None,
            show: Vec::new(),
            main_size: None,
            taps: taps.iter().map(|t| t.to_string()).collect(),
            have_library: true,
            no_defaults: true,
            no_siblings: true,
        }
    }

    #[test]
    fn the_same_run_gives_the_same_trace_and_it_says_what_it_did() {
        let trace = rec(&scenario("Andamp", &[]));
        assert_eq!(trace, rec(&scenario("Andamp", &[])));
        let lines: Vec<&str> = trace.trim_end().lines().collect();
        assert_eq!(lines[0], "trace v1");
        assert_eq!(lines[1], "skin Small");
        assert_eq!(lines[2], "song Andamp");
        assert!(lines.contains(&"step load"));
        assert!(lines.contains(&"frame t=0"));
        assert!(lines
            .iter()
            .any(|l| l.starts_with("window ") && l.contains(" main/normal ")));
        assert!(lines.iter().any(|l| l.starts_with("text \"Andamp\"")));
        assert!(lines.contains(&"step play"));
        assert!(
            lines.iter().filter(|l| l.starts_with("frame t=")).count() > 2,
            "the time display changes while the song plays"
        );
        assert!(lines.last().unwrap().starts_with("end t="));
    }

    #[test]
    fn what_the_skin_shows_is_in_the_trace_and_a_tap_too() {
        let plain = rec(&scenario("Andamp", &[]));
        let other = rec(&scenario("Second Song", &[]));
        assert_ne!(plain, other);
        assert!(other.lines().any(|l| l.starts_with("text \"Second Song\"")));
        let tapped = rec(&scenario("Andamp", &["play"]));
        assert!(tapped.lines().any(|l| l == "tap play 120,50"));
        assert!(tapped.lines().any(|l| l == "log action PLAY  from play"));
        let missing = rec(&scenario("Andamp", &["nothing"]));
        assert!(missing.lines().any(|l| l == "tap nothing missing"));
    }
}
