// SPDX-License-Identifier: GPL-3.0-or-later

//! The engine for the web: a wasm module (`wasm32-unknown-unknown`) with a small C-style
//! surface a JS host drives. Nothing here draws, decodes or measures: the host gives the skin's
//! files, answers image sizes and pixels and text metrics through imports, steps the clock, and
//! takes the draw list out as text, one command per line in the trace format (`web/walkit.mjs`
//! wraps all of it).
//!
//! Two surfaces:
//! - `wk_rt_*`: a runtime on a host of imports (`host_*`), for a page that shows a skin. A
//!   frame comes out as text (`wk_rt_draw`, one command per line, no pixel data) or as the
//!   packed frame of `walkit::packed` (`wk_rt_draw_frame`): every window with its plan and
//!   only the commands that changed since the frame before, pixels included, for a renderer.
//! - `wk_record`: a scenario of the reference set run on the fixed trace host, timed on the
//!   host's clock, for the parity check and the measurements. The pictures still come from the
//!   host (`host_decode`), as the trace host decodes none itself.
//!
//! Strings cross as UTF-8 at a pointer and a length in the module's memory; the host allocates
//! with `wk_alloc` and frees with `wk_free`. A result the module hands back stays valid until
//! the next call that gives one. Every handle is an index the module hands out.
//!
//! This crate is the boundary: exporting a function to the host and calling one the host
//! provides are what Rust marks unsafe, and nothing else here is. The engine itself forbids
//! unsafe code.

#![allow(unsafe_code)]
#![warn(missing_docs)]
// on any other target (the gate's clippy and tests run on the host) the crate is empty
#![cfg(target_arch = "wasm32")]

mod host;
mod imports;

/// The allocator: talc, a few KB smaller than the dlmalloc Rust brings by default.
#[global_allocator]
static ALLOCATOR: talc::TalckWasm = unsafe { talc::TalckWasm::new_global() };

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use walkit::files::{LayeredFiles, MemoryFiles, SkinFiles, SkinsFolderFiles};
use walkit::host::SkinSetup;
use walkit::packed::{FrameWindow, FrameWriter};
use walkit::plan::DrawPlanner;
use walkit::player::FakePlayer;
use walkit::SkinRuntime;

use host::JsHost;

/// A stack of files: a skin over the default set, with the siblings a path may reach into.
struct Stack {
    skin: usize,
    defaults: Option<usize>,
    siblings: Vec<(String, usize)>,
}

/// A runtime the host drives, with the clock it steps, and what plans and writes its frames.
struct Rt {
    engine: SkinRuntime,
    now: Rc<std::cell::Cell<i64>>,
    planner: DrawPlanner,
    frames: FrameWriter,
}

#[derive(Default)]
struct State {
    files: Vec<Rc<MemoryFiles>>,
    stacks: Vec<Stack>,
    runtimes: Vec<Option<Rt>>,
    /// The last result handed back (`wk_result_ptr` / `wk_result_len`).
    result: Vec<u8>,
    /// The last run's timing as JSON (`wk_stats_ptr` / `wk_stats_len`).
    #[cfg(feature = "trace")]
    stats: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(with_panic_log(State::default()));
}

/// `state`, once the module's panics go to the page's log with their message and place: on
/// the wasm target a panic only shows as "unreachable" otherwise.
fn with_panic_log(state: State) -> State {
    std::panic::set_hook(Box::new(|info| {
        imports::log(&format!("walkit panic: {info}"));
    }));
    state
}

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|s| f(&mut s.borrow_mut()))
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

/// The files of stack `id` composed as the engine reads them.
fn stack_files(st: &State, id: usize) -> Option<Box<dyn SkinFiles>> {
    let stack = st.stacks.get(id)?;
    let mut files: Box<dyn SkinFiles> = Box::new(FilesHandle(st.files.get(stack.skin)?.clone()));
    if !stack.siblings.is_empty() {
        let map: HashMap<String, Rc<dyn SkinFiles>> = stack
            .siblings
            .iter()
            .filter_map(|(n, i)| {
                st.files
                    .get(*i)
                    .map(|f| (n.clone(), f.clone() as Rc<dyn SkinFiles>))
            })
            .collect();
        files = Box::new(SkinsFolderFiles::new(
            files,
            Box::new(move |name| map.get(name).cloned()),
        ));
    }
    if let Some(d) = stack.defaults.and_then(|d| st.files.get(d)) {
        files = Box::new(LayeredFiles::new(files, Box::new(FilesHandle(d.clone()))));
    }
    Some(files)
}

/// The bytes at `ptr`, `len` of the module's memory, as text.
fn text(ptr: *const u8, len: usize) -> String {
    imports::read_bytes(ptr, len)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// Memory for the host: `len` bytes the host fills, to be given back with `wk_free`.
#[unsafe(no_mangle)]
pub extern "C" fn wk_alloc(len: usize) -> *mut u8 {
    imports::alloc(len)
}

/// Gives back memory from `wk_alloc`.
#[unsafe(no_mangle)]
pub extern "C" fn wk_free(ptr: *mut u8, len: usize) {
    imports::free(ptr, len);
}

/// Where the last result's text starts.
#[unsafe(no_mangle)]
pub extern "C" fn wk_result_ptr() -> *const u8 {
    with_state(|s| s.result.as_ptr())
}

/// How long the last result's text is.
#[unsafe(no_mangle)]
pub extern "C" fn wk_result_len() -> usize {
    with_state(|s| s.result.len())
}

/// Where the last run's timing JSON starts.
#[cfg(feature = "trace")]
#[unsafe(no_mangle)]
pub extern "C" fn wk_stats_ptr() -> *const u8 {
    with_state(|s| s.stats.as_ptr())
}

/// How long the last run's timing JSON is.
#[cfg(feature = "trace")]
#[unsafe(no_mangle)]
pub extern "C" fn wk_stats_len() -> usize {
    with_state(|s| s.stats.len())
}

/// A new, empty set of files; its id.
#[unsafe(no_mangle)]
pub extern "C" fn wk_files_new() -> i32 {
    with_state(|s| {
        s.files.push(Rc::new(MemoryFiles::new()));
        s.files.len() as i32 - 1
    })
}

/// Adds the file at `path` with `bytes` to the files `id`. The set must not be in a stack yet.
#[unsafe(no_mangle)]
pub extern "C" fn wk_files_add(
    id: i32,
    path: *const u8,
    path_len: usize,
    bytes: *const u8,
    len: usize,
) -> i32 {
    let path = text(path, path_len);
    let Some(bytes) = imports::read_bytes(bytes, len) else {
        return 0;
    };
    with_state(
        |s| match s.files.get_mut(id as usize).and_then(Rc::get_mut) {
            Some(f) => {
                f.insert(&path, bytes);
                1
            }
            None => 0,
        },
    )
}

/// Strips the folder the skin sits in from the files `id` (some archives hold the whole skin
/// in one folder), as [`walkit::files::skin_folder`] finds it. Nothing changes at the root.
#[unsafe(no_mangle)]
pub extern "C" fn wk_files_strip_folder(id: i32) -> i32 {
    with_state(|s| {
        let Some(slot) = s.files.get_mut(id as usize) else {
            return 0;
        };
        let names = slot.names();
        let prefix =
            walkit::files::skin_folder(names.iter().map(|n| n.as_str())).unwrap_or_default();
        if prefix.is_empty() {
            return 1;
        }
        let taken = std::mem::replace(slot, Rc::new(MemoryFiles::new()));
        let inner = Rc::try_unwrap(taken).unwrap_or_else(|rc| (*rc).clone());
        *slot = Rc::new(inner.in_folder(&prefix));
        1
    })
}

/// A new stack: the skin's files `skin` over the default set `defaults` (-1 for none); its id.
#[unsafe(no_mangle)]
pub extern "C" fn wk_stack_new(skin: i32, defaults: i32) -> i32 {
    with_state(|s| {
        s.stacks.push(Stack {
            skin: skin as usize,
            defaults: usize::try_from(defaults).ok(),
            siblings: Vec::new(),
        });
        s.stacks.len() as i32 - 1
    })
}

/// Adds the files `files` as the sibling skin `name` (its folder name lowercased without spaces) of stack `stack`.
#[unsafe(no_mangle)]
pub extern "C" fn wk_stack_sibling(
    stack: i32,
    name: *const u8,
    name_len: usize,
    files: i32,
) -> i32 {
    let name = text(name, name_len);
    with_state(|s| match s.stacks.get_mut(stack as usize) {
        Some(st) => {
            st.siblings.push((name, files as usize));
            1
        }
        None => 0,
    })
}

/// Records the scenario `spec` (see `walkit_trace::Scenario::spec`) on stack `stack` with the
/// stand-in fonts in files `fonts` (-1 for none). The trace text is the result; the timing is
/// the stats JSON. 1 when it ran.
#[cfg(feature = "trace")]
#[unsafe(no_mangle)]
pub extern "C" fn wk_record(stack: i32, fonts: i32, spec: *const u8, spec_len: usize) -> i32 {
    let sc = walkit_trace::Scenario::from_spec(&text(spec, spec_len));
    let (files, fonts) = with_state(|s| {
        (
            stack_files(s, stack as usize),
            usize::try_from(fonts)
                .ok()
                .and_then(|f| s.files.get(f).cloned()),
        )
    });
    let Some(files) = files else { return 0 };
    let fonts: Option<Rc<dyn SkinFiles>> = fonts.map(|f| f as Rc<dyn SkinFiles>);
    let perf: walkit_trace::trace::Perf = Box::new(imports::perf_now);
    let rec = walkit_trace::record(&sc, files, Box::new(host::decode), fonts, &perf);
    let stats = stats_json(&sc.name, &rec.stats);
    with_state(|s| {
        s.result = rec.text.into_bytes();
        s.stats = stats.into_bytes();
    });
    1
}

#[cfg(feature = "trace")]
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[i.min(sorted.len() - 1)]
}

/// One run's timing as a JSON object, the same shape the native binary writes.
#[cfg(feature = "trace")]
fn stats_json(name: &str, s: &walkit_trace::Stats) -> String {
    let sorted = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        v
    };
    let ticks = sorted(&s.ticks);
    let draws = sorted(&s.draws);
    let dirty: Vec<f64> = s
        .draws
        .iter()
        .zip(s.dirty.iter())
        .filter(|(_, d)| **d)
        .map(|(t, _)| *t)
        .collect();
    let dirty = sorted(&dirty);
    let mean = |v: &[f64]| {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f64>() / v.len() as f64
        }
    };
    format!(
        concat!(
            "{{\"name\":\"{}\",\"load_ms\":{:.4},\"ticks\":{},\"tick_mean_ms\":{:.5},\"tick_p95_ms\":{:.5},\"tick_max_ms\":{:.5},",
            "\"draw_mean_ms\":{:.5},\"draw_p95_ms\":{:.5},\"draw_max_ms\":{:.5},\"dirty_draws\":{},\"dirty_draw_p95_ms\":{:.5},",
            "\"frames\":{},\"total_ms\":{:.3}}}"
        ),
        name,
        s.load_ms,
        s.ticks.len(),
        mean(&s.ticks),
        percentile(&ticks, 0.95),
        ticks.last().copied().unwrap_or(0.0),
        mean(&s.draws),
        percentile(&draws, 0.95),
        draws.last().copied().unwrap_or(0.0),
        dirty.len(),
        percentile(&dirty, 0.95),
        s.frames,
        s.load_ms + s.ticks.iter().sum::<f64>() + s.draws.iter().sum::<f64>()
    )
}

// ---- a runtime the host drives ---------------------------------------------------------------

/// A runtime on stack `stack` for the skin named `skin_name`, with a library when `have_library`,
/// its clock at `start_ms`; its id. The host answers its calls through the `host_*` imports.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_new(
    stack: i32,
    skin_name: *const u8,
    skin_name_len: usize,
    have_library: i32,
    start_ms: f64,
) -> i32 {
    let name = text(skin_name, skin_name_len);
    with_state(|s| {
        let Some(files) = stack_files(s, stack as usize) else {
            return -1;
        };
        let now = Rc::new(std::cell::Cell::new(start_ms as i64));
        let clock_now = now.clone();
        let clock: walkit::host::Clock = Rc::new(move || clock_now.get());
        let player = FakePlayer::new(clock.clone());
        let engine = SkinRuntime::new(
            files,
            Box::new(JsHost),
            Box::new(player),
            SkinSetup {
                skin_name: name,
                have_library: have_library != 0,
                host_actions: true,
            },
            clock,
        );
        s.runtimes.push(Some(Rt {
            engine,
            now,
            planner: DrawPlanner::new(),
            frames: FrameWriter::new(),
        }));
        s.runtimes.len() as i32 - 1
    })
}

fn with_rt<T>(id: i32, f: impl FnOnce(&mut Rt) -> T) -> Option<T> {
    with_state(|s| {
        s.runtimes
            .get_mut(id as usize)
            .and_then(|r| r.as_mut())
            .map(f)
    })
}

/// Loads the skin: `skin.xml` and everything it includes, the windows, the scripts.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_load(id: i32) -> i32 {
    with_rt(id, |r| r.engine.load()).map(|()| 1).unwrap_or(0)
}

/// Sets the clock the runtime's timers, animations and scripts read, in ms.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_now(id: i32, now_ms: f64) -> i32 {
    with_rt(id, |r| r.now.set(now_ms as i64))
        .map(|()| 1)
        .unwrap_or(0)
}

/// Advances timers, tickers and player-driven objects; 1 when a redraw is due.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_tick(id: i32) -> i32 {
    with_rt(id, |r| r.engine.tick() as i32).unwrap_or(0)
}

/// Every window's drawing as text: a `window <id> <container>/<layout> <rect>` line, then one
/// line per draw command in the trace format. The result; 1 when it drew.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_draw(id: i32) -> i32 {
    let Some(text) = with_rt(id, |r| {
        let ids = r.engine.window_ids();
        let drawn = r.engine.draw_windows();
        let mut out = String::new();
        for (wid, (_, cmds)) in ids.iter().zip(drawn.iter()) {
            let info = r.engine.window_info(*wid);
            let rect = r.engine.window_rect(*wid);
            out.push_str(&format!(
                "window {} {}/{} {},{},{},{}\n",
                wid.0, info.container, info.layout, rect.left, rect.top, rect.width, rect.height
            ));
            for cmd in cmds {
                out.push_str(&walkit::listing::line(cmd));
                out.push('\n');
            }
        }
        out
    }) else {
        return 0;
    };
    with_state(|s| s.result = text.into_bytes());
    1
}

/// Every window's drawing with its plan, as the packed frame of `walkit::packed` writes it,
/// planned at `now_ms`, with only the commands that changed since the frame before; the bytes
/// are the result.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_draw_frame(id: i32, now_ms: f64) -> i32 {
    let Some(bytes) = with_rt(id, |r| {
        let ids = r.engine.window_ids();
        let moving = r.engine.take_tick_changed();
        let drawn = r.engine.draw_windows_owned();
        let planned = r.planner.plan(&ids, drawn, &moving, now_ms as i64);
        let infos: Vec<_> = planned.iter().map(|w| r.engine.window_info(w.id)).collect();
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
        r.frames.encode(&windows)
    }) else {
        return 0;
    };
    with_state(|s| s.result = bytes);
    1
}

/// The main window as it shows now, or -1 when the skin has none.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_main_window(id: i32) -> i32 {
    with_rt(id, |r| {
        r.engine.main_window().map(|w| w.0 as i32).unwrap_or(-1)
    })
    .unwrap_or(-1)
}

/// Whether window `window` resizes both ways (a host fits such a window to its view).
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_window_resizable(id: i32, window: i32) -> i32 {
    with_rt(id, |r| {
        r.engine
            .window_info(walkit::host::WindowId(window as u32))
            .resizable_both_ways as i32
    })
    .unwrap_or(0)
}

/// Resizes window `window` to `w` x `h`, within what the skin allows it.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_resize_window(id: i32, window: i32, w: i32, h: i32) -> i32 {
    with_rt(id, |r| {
        r.engine
            .resize_window(walkit::host::WindowId(window as u32), w, h);
    })
    .map(|()| 1)
    .unwrap_or(0)
}

/// A right click, or a long press, at the last pointer position; `seq` names the touch.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_right_click(id: i32, seq: i32) -> i32 {
    with_rt(id, |r| r.engine.right_click(seq))
        .map(|()| 1)
        .unwrap_or(0)
}

/// A double tap at `x`, `y` of the stack.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_double_tap(id: i32, x: i32, y: i32, seq: i32) -> i32 {
    with_rt(id, |r| r.engine.double_tap(x, y, seq))
        .map(|()| 1)
        .unwrap_or(0)
}

/// The finger goes down at a point of the stack.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_pointer_down(id: i32, x: i32, y: i32, seq: i32) -> i32 {
    with_rt(id, |r| r.engine.pointer_down(x, y, seq))
        .map(|()| 1)
        .unwrap_or(0)
}

/// The finger moves to a point of the stack.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_pointer_move(id: i32, x: i32, y: i32) -> i32 {
    with_rt(id, |r| r.engine.pointer_move(x, y))
        .map(|()| 1)
        .unwrap_or(0)
}

/// The finger lifts at a point of the stack.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_pointer_up(id: i32, x: i32, y: i32) -> i32 {
    with_rt(id, |r| r.engine.pointer_up(x, y))
        .map(|()| 1)
        .unwrap_or(0)
}

/// The classic skin cursor slot at `x`, `y` of the stack, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_cursor_at(id: i32, x: i32, y: i32) -> i32 {
    with_rt(id, |r| {
        r.engine
            .classic_cursor_at(x, y)
            .map_or(-1, |s| i32::try_from(s).unwrap_or(-1))
    })
    .unwrap_or(-1)
}

/// What cursor `slot` stands for, 0..5 in `CursorKind`'s order: what the page shows when the
/// skin has no picture for it.
#[unsafe(no_mangle)]
pub extern "C" fn wk_cursor_kind(slot: i32) -> i32 {
    use walkit::classic::cursor::{CursorKind, CURSOR_FILES};
    usize::try_from(slot)
        .ok()
        .and_then(|s| CURSOR_FILES.get(s))
        .map_or(0, |(_, k)| match k {
            CursorKind::Arrow => 0,
            CursorKind::Move => 1,
            CursorKind::HorizontalSlide => 2,
            CursorKind::VerticalSlide => 3,
            CursorKind::Close => 4,
            CursorKind::Resize => 5,
        })
}

/// The skin's picture for cursor `slot` into the result, as little-endian u32s: the number of
/// steps, each step's frame and milliseconds, the number of frames, then each frame's width,
/// height, tip x, tip y and ARGB rows. 1 when the skin has the picture.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_cursor_image(id: i32, slot: i32) -> i32 {
    let Ok(slot) = usize::try_from(slot) else {
        return 0;
    };
    let Some(Some(c)) = with_rt(id, |r| r.engine.classic_cursor(slot)) else {
        return 0;
    };
    let mut out: Vec<u32> = vec![u32::try_from(c.steps.len()).unwrap_or(0)];
    for (f, ms) in &c.steps {
        out.push(u32::try_from(*f).unwrap_or(0));
        out.push(*ms);
    }
    out.push(u32::try_from(c.frames.len()).unwrap_or(0));
    for f in &c.frames {
        out.extend([f.width, f.height, f.hot_x, f.hot_y]);
        out.extend(&f.argb);
    }
    let bytes: Vec<u8> = out.iter().flat_map(|v| v.to_le_bytes()).collect();
    with_state(|s| s.result = bytes);
    1
}

/// Opens the window of the container named; 1 when the skin has it.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_open_window(id: i32, name: *const u8, name_len: usize) -> i32 {
    let name = text(name, name_len);
    with_rt(id, |r| r.engine.open_window(&name) as i32).unwrap_or(0)
}

/// Stages a song on the page's fake player: `title`, `length_ms` long.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_track(
    id: i32,
    title: *const u8,
    title_len: usize,
    length_ms: i32,
) -> i32 {
    let title = text(title, title_len);
    with_rt(id, |r| {
        r.engine.player().set_track(&title, length_ms);
        1
    })
    .unwrap_or(0)
}

/// Shows window `window`'s container in the layout named.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_window_layout(
    id: i32,
    window: i32,
    name: *const u8,
    name_len: usize,
) -> i32 {
    let name = text(name, name_len);
    with_rt(id, |r| {
        r.engine
            .set_window_layout(walkit::host::WindowId(window as u32), &name);
        1
    })
    .unwrap_or(0)
}

/// Makes the page's fake player hand out silence, or not.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_silent(id: i32, silent: i32) -> i32 {
    with_rt(id, |r| {
        r.engine.player().set_silent(silent != 0);
        1
    })
    .unwrap_or(0)
}

/// Stages the song's format on the page's fake player: `kbps`, `khz`, `channels`.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_format(id: i32, kbps: i32, khz: i32, channels: i32) -> i32 {
    with_rt(id, |r| {
        r.engine.player().set_format(kbps, khz, channels);
        1
    })
    .unwrap_or(0)
}

/// Sets the page's fake player's volume (0..255) and balance (-127..127).
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_levels(id: i32, volume: i32, pan: i32) -> i32 {
    with_rt(id, |r| {
        r.engine.player().set_volume(volume);
        r.engine.player().set_pan(pan);
        1
    })
    .unwrap_or(0)
}

/// Wears the color theme named, matched ignoring case.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_select_theme(id: i32, name: *const u8, name_len: usize) -> i32 {
    let name = text(name, name_len);
    with_rt(id, |r| r.engine.select_color_theme(&name))
        .map(|()| 1)
        .unwrap_or(0)
}

/// The windows stack in a view `w` x `h`, as a phone stacks them; 0 x 0 leaves each window at its size.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_view(id: i32, w: i32, h: i32) -> i32 {
    with_rt(id, |r| {
        r.engine
            .set_view_size(if w > 0 && h > 0 { Some((w, h)) } else { None });
    })
    .map(|()| 1)
    .unwrap_or(0)
}

/// The windows set out apart in a viewport `w` x `h` (`on` 1), moved by a drag and docked as on
/// Winamp's desktop; 0 stacks them again.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_set_free(id: i32, on: i32, w: i32, h: i32) -> i32 {
    with_rt(id, |r| {
        let mut p = r.engine.presentation();
        p.free_windows = on != 0;
        if on != 0 && w > 0 && h > 0 {
            p.viewport = (w, h);
        }
        r.engine.set_presentation(&p);
    })
    .map(|()| 1)
    .unwrap_or(0)
}

/// The player: 1 play, 2 pause, 3 stop, 4 next, 5 previous.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_player(id: i32, action: i32) -> i32 {
    with_rt(id, |r| match action {
        1 => r.engine.player().play(),
        2 => r.engine.player().pause(),
        3 => r.engine.player().stop(),
        4 => r.engine.player().next(),
        5 => r.engine.player().previous(),
        _ => {}
    })
    .map(|()| 1)
    .unwrap_or(0)
}

/// When, in clock ms, something on the skin next changes by itself; -1 when nothing will.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_next_due_at(id: i32) -> f64 {
    with_rt(id, |r| {
        r.engine.next_due_at().map(|d| d as f64).unwrap_or(-1.0)
    })
    .unwrap_or(-1.0)
}

/// Drops the runtime.
#[unsafe(no_mangle)]
pub extern "C" fn wk_rt_free(id: i32) {
    with_state(|s| {
        if let Some(slot) = s.runtimes.get_mut(id as usize) {
            *slot = None;
        }
    });
}
