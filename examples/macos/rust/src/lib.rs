// SPDX-License-Identifier: GPL-3.0-or-later

//! The C interface the macOS example app calls (`include/walkit_macos.h`).
//!
//! The app hands over a skin archive's bytes, the classic base skin and the wasabi base; this
//! library unzips them, lays the skin over the right base and the stand-in fonts, runs the
//! engine with a demo player, and draws each frame with a software renderer ([`render`]) into
//! a buffer of pixels the app shows. Pictures and fonts are decoded here too ([`host`]).
//!
//! Every call runs on the app's main thread: the engine is not shared across threads.

mod host;
mod render;

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, CStr, CString};
use std::io::{Cursor, Read};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use walkit::files::{self, LayeredFiles, MemoryFiles, SkinFiles};
use walkit::host::{Clock, SkinSetup};
use walkit::player::{FakePlayer, Player, Track};
use walkit::SkinRuntime;

use host::{Assets, Host};

/// The screen the skin's scripts see, in layout pixels.
const VIEWPORT: (i32, i32) = (1440, 900);

/// The largest file read out of an archive, and the most read in all.
const MAX_ENTRY: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 128 * 1024 * 1024;

/// A loaded skin: the engine, what it shares with the renderer, and the last frame drawn.
pub struct WalkitSkin {
    runtime: SkinRuntime,
    assets: Rc<RefCell<Assets>>,
    redraw: Rc<Cell<bool>>,
    frame: Vec<u32>,
    /// Set when the engine panicked: the skin does nothing more.
    broken: bool,
}

impl WalkitSkin {
    fn open(
        bytes: Vec<u8>,
        file_name: &str,
        base: Option<Vec<u8>>,
        wasabi: Option<Vec<u8>>,
        fonts: MemoryFiles,
    ) -> Result<WalkitSkin, String> {
        let archive = unzip(bytes)?;
        // a classic skin lies over the classic base skin, a modern one over the wasabi base
        let under = if walkit::classic::is_classic(&archive) {
            base
        } else {
            wasabi
        };
        let under = under.map(unzip).transpose()?;
        // the engine and the host each read the files, so the host decodes wasabi-base/...
        // pictures and the stand-in fonts from under the skin too; the bytes are shared
        let layered = || -> Box<dyn SkinFiles> {
            let below: Box<dyn SkinFiles> = match &under {
                Some(base) => Box::new(LayeredFiles::new(
                    Box::new(base.clone()),
                    Box::new(fonts.clone()),
                )),
                None => Box::new(fonts.clone()),
            };
            Box::new(LayeredFiles::new(Box::new(archive.clone()), below))
        };
        let assets = Rc::new(RefCell::new(Assets::new(layered())));
        let redraw = Rc::new(Cell::new(true));
        let host = Host {
            assets: assets.clone(),
            redraw: redraw.clone(),
        };
        let start = Instant::now();
        let clock: Clock = Rc::new(move || start.elapsed().as_millis() as i64);
        // the demo player: it plays nothing, but answers play, pause, stop, seek, volume, pan,
        // the equalizer, shuffle and repeat, and its clock moves the time and the visualizer
        let mut player = FakePlayer::new(clock.clone());
        player.playlist = (0..8)
            .map(|i| Track {
                title: format!("walkit example - track {}", i + 1),
                length_ms: 180_000 + i * 15_000,
            })
            .collect();
        player.play_track(0);
        // the folder name scripts know the skin by
        let name = Path::new(file_name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut runtime = SkinRuntime::new(
            layered(),
            Box::new(host),
            Box::new(player),
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
        Ok(WalkitSkin {
            runtime,
            assets,
            redraw,
            frame: Vec::new(),
            broken: false,
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

    /// Draws a frame of `w` x `h` device pixels at `scale` into [`WalkitSkin::frame`].
    fn draw(&mut self, w: i32, h: i32, scale: i32) {
        let windows = self.runtime.draw_windows();
        let mut canvas = render::Canvas::new(w, h);
        render::draw(&mut canvas, &mut self.assets.borrow_mut(), &windows, scale);
        self.redraw.set(false);
        self.frame = canvas.pixels;
    }
}

/// The files of a zip archive in memory. A skin zipped inside a folder has that folder
/// stripped.
fn unzip(bytes: Vec<u8>) -> Result<MemoryFiles, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
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

/// The `.ttf` files in `dir`, as `fonts/<name>`: where the engine looks for the free fonts
/// that stand in for Arial and Tahoma.
fn read_fonts(dir: &Path) -> MemoryFiles {
    let mut fonts = MemoryFiles::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return fonts;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ttf"))
        {
            if let (Some(name), Ok(bytes)) = (path.file_name(), std::fs::read(&path)) {
                fonts.insert(&format!("fonts/{}", name.to_string_lossy()), bytes);
            }
        }
    }
    fonts
}

/// The `len` bytes at `ptr`, or `None` for a null pointer.
unsafe fn bytes(ptr: *const u8, len: usize) -> Option<Vec<u8>> {
    (!ptr.is_null()).then(|| std::slice::from_raw_parts(ptr, len).to_vec())
}

/// The C string at `ptr`, or an empty one for a null pointer.
unsafe fn string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

/// The text of a panic's payload.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the engine stopped".into())
}

/// Runs `f` on the skin at `skin` unless it broke before; a panic breaks it and gives
/// `fallback`.
unsafe fn with_skin<T>(
    skin: *mut WalkitSkin,
    fallback: T,
    f: impl FnOnce(&mut WalkitSkin) -> T,
) -> T {
    let Some(skin) = skin.as_mut() else {
        return fallback;
    };
    if skin.broken {
        return fallback;
    }
    match catch_unwind(AssertUnwindSafe(|| f(&mut *skin))) {
        Ok(v) => v,
        Err(payload) => {
            eprintln!("walkit: {}", panic_text(payload.as_ref()));
            skin.broken = true;
            fallback
        }
    }
}

/// Opens the skin archive (`.wsz` or `.wal`) of `skin_len` bytes at `skin`, named `name` (its
/// file name). `base` is a classic skin laid under a classic skin, `wasabi` a zip of Winamp's
/// wasabi folder laid under a modern one; either may be null. `fonts_dir` is a folder of the
/// stand-in fonts, or null. On failure it gives null and, when `error` is not null, a message
/// in `*error` to free with [`walkit_string_free`].
///
/// # Safety
/// The pointers are null or point at that many bytes, or at C strings.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_open(
    skin: *const u8,
    skin_len: usize,
    name: *const c_char,
    base: *const u8,
    base_len: usize,
    wasabi: *const u8,
    wasabi_len: usize,
    fonts_dir: *const c_char,
    error: *mut *mut c_char,
) -> *mut WalkitSkin {
    let result = catch_unwind(|| {
        let skin = bytes(skin, skin_len).ok_or("no skin")?;
        let fonts = if fonts_dir.is_null() {
            MemoryFiles::new()
        } else {
            read_fonts(Path::new(&string(fonts_dir)))
        };
        WalkitSkin::open(
            skin,
            &string(name),
            bytes(base, base_len),
            bytes(wasabi, wasabi_len),
            fonts,
        )
    })
    .unwrap_or_else(|payload| Err(panic_text(payload.as_ref())));
    match result {
        Ok(skin) => Box::into_raw(Box::new(skin)),
        Err(message) => {
            if !error.is_null() {
                *error = CString::new(message.replace('\0', " "))
                    .unwrap_or_default()
                    .into_raw();
            }
            std::ptr::null_mut()
        }
    }
}

/// Frees a skin [`walkit_skin_open`] gave.
///
/// # Safety
/// `skin` is null or a skin not freed before.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_free(skin: *mut WalkitSkin) {
    if !skin.is_null() {
        let skin = Box::from_raw(skin);
        let _ = catch_unwind(AssertUnwindSafe(move || drop(skin)));
    }
}

/// Frees a message [`walkit_skin_open`] gave.
///
/// # Safety
/// `text` is null or a message not freed before.
#[no_mangle]
pub unsafe extern "C" fn walkit_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}

/// Runs the engine's timers and animations; true when the skin wants a new frame.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_tick(skin: *mut WalkitSkin) -> bool {
    with_skin(skin, false, |s| s.runtime.tick() || s.redraw.get())
}

/// The size the skin's windows take, in layout pixels.
///
/// # Safety
/// `skin` is null or a live skin; `w` and `h` point at ints.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_size(skin: *mut WalkitSkin, w: *mut i32, h: *mut i32) {
    let (sw, sh) = with_skin(skin, (275, 116), |s| s.size());
    *w = sw;
    *h = sh;
}

/// Draws a frame of `w` x `h` device pixels, `scale` device pixels per layout pixel, and
/// gives its pixels: `w * h` of them, rows top first, each `0x00RRGGBB`. They stay valid
/// until the next draw or until the skin is freed; null when the skin broke.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_draw(
    skin: *mut WalkitSkin,
    w: i32,
    h: i32,
    scale: i32,
) -> *const u32 {
    with_skin(skin, std::ptr::null(), |s| {
        s.draw(w.max(1), h.max(1), scale.max(1));
        s.frame.as_ptr()
    })
}

/// The mouse went down at `x`, `y` (layout pixels); `touch` numbers the press.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_pointer_down(
    skin: *mut WalkitSkin,
    x: i32,
    y: i32,
    touch: i32,
) {
    with_skin(skin, (), |s| s.runtime.pointer_down(x, y, touch));
}

/// The mouse moved to `x`, `y` while down.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_pointer_move(skin: *mut WalkitSkin, x: i32, y: i32) {
    with_skin(skin, (), |s| s.runtime.pointer_move(x, y));
}

/// The mouse went up at `x`, `y`.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_pointer_up(skin: *mut WalkitSkin, x: i32, y: i32) {
    with_skin(skin, (), |s| s.runtime.pointer_up(x, y));
}

/// A double click at `x`, `y`; `touch` numbers its press.
///
/// # Safety
/// `skin` is null or a live skin.
#[no_mangle]
pub unsafe extern "C" fn walkit_skin_double_click(
    skin: *mut WalkitSkin,
    x: i32,
    y: i32,
    touch: i32,
) {
    with_skin(skin, (), |s| s.runtime.double_tap(x, y, touch));
}
