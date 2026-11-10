// SPDX-License-Identifier: GPL-3.0-or-later

//! A small macOS app that shows a Winamp skin with the walkit engine.
//!
//! ```text
//! cargo run --release -- <skin.wsz|skin.wal> [--defaults <wasabi-defaults.zip>]
//! ```
//!
//! The engine lays the skin out and hands back a draw list each frame; this app unzips the
//! skin, decodes its pictures and fonts ([`host`]), draws the list into a window with a
//! software renderer ([`render`]) and passes the mouse back. Drop another skin on the window
//! to open it. `--screenshot <out.png>` draws a few frames without a window, writes the last
//! one and exits.

mod host;
mod render;

use std::cell::{Cell, RefCell};
use std::io::Read;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use walkit::files::{self, LayeredFiles, MemoryFiles, SkinFiles};
use walkit::host::{Clock, SkinSetup};
use walkit::player::FakePlayer;
use walkit::SkinRuntime;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use host::{Assets, Host};
use render::Canvas;

/// How long a frame lasts.
const FRAME: Duration = Duration::from_millis(16);

/// The screen the skin's scripts see, in layout pixels.
const VIEWPORT: (i32, i32) = (1440, 900);

/// The largest file read out of an archive, and the most read in all.
const MAX_ENTRY: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 128 * 1024 * 1024;

struct Options {
    skin: PathBuf,
    defaults: Option<PathBuf>,
    screenshot: Option<PathBuf>,
    scale: i32,
}

fn main() {
    let options = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            eprintln!(
                "usage: walkit-macos-example <skin.wsz|skin.wal> [--defaults <wasabi-defaults.zip>] \
                 [--screenshot <out.png>] [--scale <n>]"
            );
            std::process::exit(2);
        }
    };
    // a modern skin draws lists, scrollbars and frames it does not define from the default set
    let defaults = match options.defaults.as_deref().map(read_archive) {
        Some(Ok(d)) => Some(d),
        Some(Err(e)) => {
            eprintln!("defaults: {e}");
            std::process::exit(1);
        }
        None => None,
    };
    let skin = match Skin::open(&options.skin, defaults.as_ref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", options.skin.display());
            std::process::exit(1);
        }
    };
    if let Some(out) = &options.screenshot {
        if let Err(e) = screenshot(skin, out, options.scale) {
            eprintln!("{}: {e}", out.display());
            std::process::exit(1);
        }
        return;
    }
    let event_loop = EventLoop::new().expect("an event loop");
    let mut app = App {
        skin,
        defaults,
        window: None,
        surface: None,
        cursor: PhysicalPosition::new(0.0, 0.0),
        pressed: false,
        touch: 0,
        shown_size: (0, 0),
    };
    event_loop.run_app(&mut app).expect("the event loop runs");
}

fn parse_args() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut skin = None;
    let mut defaults = None;
    let mut screenshot = None;
    let mut scale = 2;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--defaults" => defaults = Some(PathBuf::from(value()?)),
            "--screenshot" => screenshot = Some(PathBuf::from(value()?)),
            "--scale" => {
                scale = value()?
                    .parse::<i32>()
                    .map_err(|e| e.to_string())?
                    .clamp(1, 8);
            }
            _ if skin.is_none() => skin = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(Options {
        skin: skin.ok_or("no skin file given")?,
        defaults,
        screenshot,
        scale,
    })
}

/// Reads a `.wsz`, `.wal` or `.zip` (all zip archives) into memory. A skin zipped inside a
/// folder has that folder stripped.
fn read_archive(path: &Path) -> Result<MemoryFiles, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut all = MemoryFiles::new();
    let mut names = Vec::new();
    let mut total = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // the sizes an archive claims can lie: count what comes out
        let mut bytes = Vec::new();
        let read = entry
            .by_ref()
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())? as u64;
        if read > MAX_ENTRY {
            continue;
        }
        total += read;
        if total > MAX_TOTAL {
            return Err("the archive is too large".into());
        }
        all.insert(entry.name(), bytes);
        names.push(entry.name().to_string());
    }
    let folder = files::skin_folder(names.iter().map(String::as_str)).unwrap_or_default();
    Ok(all.in_folder(&folder))
}

/// A loaded skin: the engine and what it shares with the renderer.
struct Skin {
    runtime: SkinRuntime,
    assets: Rc<RefCell<Assets>>,
    redraw: Rc<Cell<bool>>,
    /// The window's title: the skin's file name.
    title: String,
}

impl Skin {
    fn open(path: &Path, defaults: Option<&MemoryFiles>) -> Result<Skin, String> {
        let archive = read_archive(path)?;
        // the engine and the host each read the files; the bytes are shared, not copied
        let layered = || -> Box<dyn SkinFiles> {
            match defaults {
                Some(d) => Box::new(LayeredFiles::new(
                    Box::new(archive.clone()),
                    Box::new(d.clone()),
                )),
                None => Box::new(archive.clone()),
            }
        };
        let assets = Rc::new(RefCell::new(Assets::new(layered())));
        let redraw = Rc::new(Cell::new(true));
        let host = Host {
            assets: assets.clone(),
            redraw: redraw.clone(),
        };
        let start = Instant::now();
        let clock: Clock = Rc::new(move || start.elapsed().as_millis() as i64);
        // the folder name scripts know the skin by
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut runtime = SkinRuntime::new(
            layered(),
            Box::new(host),
            Box::new(FakePlayer::new(clock.clone())),
            SkinSetup {
                skin_name: name,
                ..SkinSetup::default()
            },
            clock,
        );
        runtime.set_viewport(VIEWPORT.0, VIEWPORT.1);
        runtime.load();
        // a classic skin's equalizer and playlist start shut: open them, so all three windows
        // show (a modern skin has no containers by these names, and nothing opens)
        runtime.open_window("equalizer");
        runtime.open_window("pledit");
        runtime.tick();
        Ok(Skin {
            runtime,
            assets,
            redraw,
            title: title(path),
        })
    }

    /// The part of the stack the windows show, in layout pixels.
    fn size(&mut self) -> (i32, i32) {
        let (w, h) = self.runtime.drawn_size();
        if w > 0 && h > 0 {
            (w, h)
        } else {
            (275, 116)
        }
    }

    /// Draws a frame of `w` x `h` device pixels at `scale`.
    fn draw(&mut self, w: i32, h: i32, scale: i32) -> Canvas {
        let windows = self.runtime.draw_windows();
        let mut canvas = Canvas::new(w, h);
        render::draw(&mut canvas, &mut self.assets.borrow_mut(), &windows, scale);
        self.redraw.set(false);
        canvas
    }
}

/// Draws a second of frames with no window and writes the last one as a PNG.
fn screenshot(mut skin: Skin, out: &Path, scale: i32) -> Result<(), String> {
    for _ in 0..60 {
        skin.runtime.tick();
        std::thread::sleep(FRAME);
    }
    let (w, h) = skin.size();
    let canvas = skin.draw(w * scale, h * scale, scale);
    let rgb: Vec<u8> = canvas
        .pixels
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    image::save_buffer(
        out,
        &rgb,
        canvas.w as u32,
        canvas.h as u32,
        image::ExtendedColorType::Rgb8,
    )
    .map_err(|e| e.to_string())
}

/// The window, its surface and the mouse.
struct App {
    skin: Skin,
    defaults: Option<MemoryFiles>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    cursor: PhysicalPosition<f64>,
    pressed: bool,
    /// Numbers each press, as the engine tells touches apart.
    touch: i32,
    /// The size the window was last given, in layout pixels.
    shown_size: (i32, i32),
}

impl App {
    /// Device pixels per layout pixel: 2 on a Retina screen.
    fn scale(&self) -> i32 {
        self.window
            .as_ref()
            .map_or(1, |w| w.scale_factor().round().max(1.0) as i32)
    }

    /// The mouse in layout pixels.
    fn pointer(&self) -> (i32, i32) {
        let s = f64::from(self.scale());
        ((self.cursor.x / s) as i32, (self.cursor.y / s) as i32)
    }

    /// Sizes the window to the skin when the skin's windows grew or shrank.
    fn fit_window(&mut self) {
        let size = self.skin.size();
        if size != self.shown_size {
            self.shown_size = size;
            if let Some(window) = &self.window {
                let _ = window.request_inner_size(LogicalSize::new(size.0, size.1));
            }
        }
    }

    fn redraw(&mut self) {
        let scale = self.scale();
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        surface.resize(w, h).expect("the surface resizes");
        let canvas = self.skin.draw(w.get() as i32, h.get() as i32, scale);
        let mut buffer = surface.buffer_mut().expect("a buffer");
        buffer.copy_from_slice(&canvas.pixels);
        buffer.present().expect("the frame shows");
        self.fit_window();
    }

    fn open(&mut self, path: &Path) {
        match Skin::open(path, self.defaults.as_ref()) {
            Ok(skin) => {
                self.skin = skin;
                if let Some(window) = &self.window {
                    window.set_title(&self.skin.title);
                    window.request_redraw();
                }
                self.fit_window();
            }
            Err(e) => eprintln!("{}: {e}", path.display()),
        }
    }
}

fn title(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (w, h) = self.skin.size();
        self.shown_size = (w, h);
        let attributes = Window::default_attributes()
            .with_title(&self.skin.title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_resizable(false);
        let window = Rc::new(event_loop.create_window(attributes).expect("a window"));
        let context = softbuffer::Context::new(window.clone()).expect("a drawing context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("a surface");
        self.window = Some(window);
        self.surface = Some(surface);
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.skin.redraw.set(true);
            }
            WindowEvent::DroppedFile(path) => self.open(&path),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                if self.pressed {
                    let (x, y) = self.pointer();
                    self.skin.runtime.pointer_move(x, y);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let (x, y) = self.pointer();
                match state {
                    ElementState::Pressed => {
                        self.pressed = true;
                        self.touch += 1;
                        self.skin.runtime.pointer_down(x, y, self.touch);
                    }
                    ElementState::Released if self.pressed => {
                        self.pressed = false;
                        self.skin.runtime.pointer_up(x, y);
                    }
                    ElementState::Released => {}
                }
            }
            _ => {}
        }
    }

    /// Once per frame: the engine runs its timers and animations, and the window draws again
    /// when something changed.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let changed = self.skin.runtime.tick();
        if changed || self.skin.redraw.get() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + FRAME));
    }
}
