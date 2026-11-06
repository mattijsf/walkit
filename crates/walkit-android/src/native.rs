// SPDX-License-Identifier: GPL-3.0-or-later

//! The native methods of `nl.mattix.walkit.Native`, one per engine operation. A runtime is a
//! handle (`long`) made by `rtNew` and dropped by `rtFree`; every other method takes it first.
//! Sizes and rects come back as int arrays, a window's or an object's description as a small
//! packed buffer (ints, then strings as a length and UTF-8 bytes), a frame as the buffer of
//! `walkit::packed`. "None" answers are -1 for ids, `Int.MIN_VALUE` for seqs, `Long.MIN_VALUE`
//! for a time, an empty array for a size.

use std::cell::{Cell, RefCell};
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::rc::Rc;
use std::time::Instant;

use jni::errors::{Error, Result, ThrowRuntimeExAndDefault};
use jni::objects::{JByteArray, JClass, JIntArray, JLongArray, JObject, JObjectArray, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::{jni_mangle, Env, EnvUnowned};
use walkit::host::{
    Edge, OptionEntry, OptionKey, Presentation, SavedSettings, SkinSetup, WindowId,
};
use walkit::maki::{Handle, Value};
use walkit::packed::{FrameWindow, FrameWriter};
use walkit::plan::DrawPlanner;
use walkit::SkinRuntime;

use crate::bridge::{bytes_of, jints, jstring, throw, Bridge, Failure, Thrown};
use crate::files::JniFiles;
use crate::host::{HostCache, JniHost};
use crate::player::{JniPlayer, PlayerCache};

/// A runtime the Kotlin side drives, with the clock it sets.
struct Rt {
    rt: SkinRuntime,
    now: Rc<Cell<i64>>,
    zone_offset: Rc<Cell<i32>>,
    failure: Failure,
    /// Plans each frame, and writes it with only what changed since the frame before.
    planner: DrawPlanner,
    frames: FrameWriter,
    /// Nanoseconds spent planning, the drawing itself left out; for timing runs.
    plan_nanos: u64,
    /// What this call has read of the player and asked of the host; cleared as a call starts.
    player_cache: Rc<PlayerCache>,
    host_cache: Rc<HostCache>,
}

/// The runtime behind handle `h`.
///
/// # Safety
/// `h` came from `rt_new` and has not been given to `rt_free`: the Kotlin side keeps that.
fn rt<'a>(h: jlong) -> Result<&'a mut Rt> {
    if h == 0 {
        return Err(Error::NullPtr("runtime handle"));
    }
    // SAFETY: the handle is a pointer `rt_new` made with `Box::into_raw`, alive until `rt_free`.
    Ok(unsafe { &mut *(h as *mut Rt) })
}

/// Runs `f` on the runtime of `h`, reading the player and the host afresh, then reports what a
/// callback failed with, if one did.
fn with_rt<T>(env: &mut Env, h: jlong, f: impl FnOnce(&mut Rt) -> T) -> Result<T> {
    let r = rt(h)?;
    r.player_cache.clear();
    r.host_cache.clear();
    // a callback that failed is what the call reports, even when the engine then panicked on
    // the answer it did not get (a skin whose files cannot be read has no skin.xml)
    let out = catch_unwind(AssertUnwindSafe(|| f(r)));
    if let Some(t) = r.failure.borrow_mut().take() {
        return Err(report(env, t));
    }
    match out {
        Ok(v) => Ok(v),
        Err(panic) => resume_unwind(panic),
    }
}

/// Throws what a callback failed with as the native call's exception: the host's exception itself, or a
/// `RuntimeException` naming the failure.
fn report(env: &mut Env, t: Thrown) -> Error {
    match t {
        Thrown::Exception(e) => match env.throw(&*e) {
            Ok(()) => Error::JavaException,
            Err(e) => e,
        },
        Thrown::Message(m) => throw(env, &format!("walkit host callback failed: {m}")),
    }
}

/// A small packed answer: ints, then strings as a length and UTF-8 bytes, little-endian.
#[derive(Default)]
struct Out(Vec<u8>);

impl Out {
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }

    fn string(&mut self, s: &str) {
        self.i32(s.len() as i32);
        self.0.extend_from_slice(s.as_bytes());
    }

    fn bytes<'l>(self, env: &mut Env<'l>) -> Result<JByteArray<'l>> {
        env.byte_array_from_slice(&self.0)
    }
}

/// A small packed argument, as `Out` writes an answer: ints, and strings as a length and UTF-8
/// bytes, little-endian. `None` past the end or on bytes that are not UTF-8.
struct In<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl In<'_> {
    fn i32(&mut self) -> Option<i32> {
        let b = self.bytes.get(self.at..self.at + 4)?;
        self.at += 4;
        Some(i32::from_le_bytes(b.try_into().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        let n = usize::try_from(self.i32()?).ok()?;
        let b = self.bytes.get(self.at..self.at + n)?;
        self.at += n;
        String::from_utf8(b.to_vec()).ok()
    }

    /// `n` pairs of a string and what `value` reads.
    fn pairs<T>(
        &mut self,
        mut value: impl FnMut(&mut Self) -> Option<T>,
    ) -> Option<Vec<(String, T)>> {
        let n = usize::try_from(self.i32()?).ok()?;
        let mut out = Vec::new();
        for _ in 0..n {
            let k = self.string()?;
            out.push((k, value(self)?));
        }
        Some(out)
    }
}

fn pair<'l>(env: &mut Env<'l>, v: Option<(i32, i32)>) -> Result<JIntArray<'l>> {
    match v {
        Some((a, b)) => jints(env, &[a, b]),
        None => jints(env, &[]),
    }
}

fn edge(e: Option<Edge>) -> i32 {
    match e {
        None => -1,
        Some(Edge::Start) => 0,
        Some(Edge::End) => 1,
    }
}

// ---- a runtime's life ------------------------------------------------------------------------

/// A runtime for the skin in `files`, with `host` and `player` (the bridge objects), set up with
/// the skin's name and the two flags, the viewport scripts see, and its clock at `start_ms`.
#[jni_mangle("nl.mattix.walkit.Native")]
#[allow(clippy::too_many_arguments)]
pub fn rt_new<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    files: JObject<'l>,
    host: JObject<'l>,
    player: JObject<'l>,
    skin_name: JString<'l>,
    have_library: jboolean,
    host_actions: jboolean,
    viewport_w: jint,
    viewport_h: jint,
    start_ms: jlong,
) -> jlong {
    env.with_env(|env| -> Result<jlong> {
        let failure: Failure = Rc::new(RefCell::new(None));
        let files = JniFiles(Bridge::new(env, &files, failure.clone())?);
        let host_cache = Rc::new(HostCache::default());
        let player_cache = Rc::new(PlayerCache::default());
        let host = JniHost::new(
            Bridge::new(env, &host, failure.clone())?,
            host_cache.clone(),
        );
        let player = JniPlayer::new(
            Bridge::new(env, &player, failure.clone())?,
            player_cache.clone(),
        );
        let name = skin_name.to_string();
        let now = Rc::new(Cell::new(start_ms));
        let clock_now = now.clone();
        let clock: walkit::host::Clock = Rc::new(move || clock_now.get());
        let mut rt = SkinRuntime::new(
            Box::new(files),
            Box::new(host),
            Box::new(player),
            SkinSetup {
                skin_name: name,
                have_library,
                host_actions,
            },
            clock,
        );
        rt.set_viewport(viewport_w, viewport_h);
        let zone_offset = Rc::new(Cell::new(0));
        let zone = zone_offset.clone();
        rt.set_zone_offset(Box::new(move |_| zone.get()));
        if let Some(t) = failure.borrow_mut().take() {
            return Err(report(env, t));
        }
        let boxed = Box::new(Rt {
            rt,
            now,
            zone_offset,
            failure,
            planner: DrawPlanner::new(),
            frames: FrameWriter::new(),
            plan_nanos: 0,
            player_cache,
            host_cache,
        });
        Ok(Box::into_raw(boxed) as jlong)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Drops the runtime; the handle is dead after.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_free<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|_env| -> Result<()> {
        if h != 0 {
            // SAFETY: the handle is the pointer `rt_new` made, given back once by the Kotlin side.
            drop(unsafe { Box::from_raw(h as *mut Rt) });
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Sets the clock the runtime reads, in ms, and how far local time is from UTC at that moment.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_set_now<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    now_ms: jlong,
    zone_offset_ms: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.now.set(now_ms);
            r.zone_offset.set(zone_offset_ms);
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Loads the skin: `skin.xml` and everything it includes, the windows, the scripts.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_load<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.load()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Advances timers, tickers and player-driven objects, with the next target slide placed
/// `slide_lead` ms ahead when that is 0 or more; true when a redraw is due.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_tick<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    slide_lead: jlong,
) -> jboolean {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            let dirty = if slide_lead >= 0 {
                r.rt.tick_with_lead(slide_lead)
            } else {
                r.rt.tick()
            };
            dirty as jboolean
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Every window's drawing with its plan, as the packed frame of `walkit::packed` writes it:
/// planned at `now_ms` in the host's clock, with only the commands that changed since the frame
/// before.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_draw_windows<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    now_ms: jlong,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let bytes = with_rt(env, h, |r| {
            let ids = r.rt.window_ids();
            let moving = r.rt.take_tick_changed();
            let drawn = r.rt.draw_windows_owned();
            let start = Instant::now();
            let planned = r.planner.plan(&ids, drawn, &moving, now_ms);
            r.plan_nanos += start.elapsed().as_nanos() as u64;
            let infos: Vec<_> = planned.iter().map(|w| r.rt.window_info(w.id)).collect();
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
        })?;
        env.byte_array_from_slice(&bytes)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ---- windows ---------------------------------------------------------------------------------

/// The windows on screen, top to bottom, the main one first.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_ids<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let ids: Vec<i32> = with_rt(env, h, |r| {
            r.rt.window_ids().iter().map(|w| w.0 as i32).collect()
        })?;
        jints(env, &ids)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The main window as it shows now, or -1 when the skin has none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_main_window<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.main_window().map(|w| w.0 as i32).unwrap_or(-1)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// What window `id` is like now: id, width, height, min and max sizes, whether it resizes both
/// ways, the edges last placed against (-1 none, 0 start, 1 end), then the container and layout ids.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_info<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: jint,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let out = with_rt(env, h, |r| {
            let info = r.rt.window_info(WindowId(id as u32));
            let mut out = Out::default();
            out.i32(info.id.0 as i32);
            out.i32(info.size.0);
            out.i32(info.size.1);
            out.i32(info.min_w);
            out.i32(info.min_h);
            out.i32(info.max_w);
            out.i32(info.max_h);
            out.i32(info.resizable_both_ways as i32);
            out.i32(edge(info.edge_x));
            out.i32(edge(info.edge_y));
            out.string(&info.container);
            out.string(&info.layout);
            out
        })?;
        out.bytes(env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The object with id `id` in the windows on screen: its window, its rect in the window, its
/// rect in the stack, whether it shows, then its class name; null when no window has it.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_object_info<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: JString<'l>,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let id = id.to_string();
        let out = with_rt(env, h, |r| {
            r.rt.object_info(&id).map(|o| {
                let mut out = Out::default();
                out.i32(o.window.0 as i32);
                for r in [o.rect, o.stack_rect] {
                    out.i32(r.left);
                    out.i32(r.top);
                    out.i32(r.width);
                    out.i32(r.height);
                }
                out.i32(o.visible as i32);
                out.string(&o.class_name);
                out
            })
        })?;
        match out {
            Some(out) => out.bytes(env),
            None => Ok(JByteArray::null()),
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The window container `container` shows now, or -1 when the skin has no such container.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_of<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    container: JString<'l>,
) -> jint {
    env.with_env(|env| {
        let c = container.to_string();
        with_rt(env, h, |r| {
            r.rt.window_of(&c).map(|w| w.0 as i32).unwrap_or(-1)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Where window `id` sits in the stack: left, top, width, height.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_rect<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: jint,
) -> JIntArray<'l> {
    env.with_env(|env| {
        let r = with_rt(env, h, |r| r.rt.window_rect(WindowId(id as u32)))?;
        jints(env, &[r.left, r.top, r.width, r.height])
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Opens the window of `container`; false when the skin has no such container.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_open_window<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    container: JString<'l>,
) -> jboolean {
    env.with_env(|env| {
        let c = container.to_string();
        with_rt(env, h, |r| r.rt.open_window(&c) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Closes the window of `container`; false when the skin has no such container.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_close_window<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    container: JString<'l>,
) -> jboolean {
    env.with_env(|env| {
        let c = container.to_string();
        with_rt(env, h, |r| r.rt.close_window(&c) as jboolean)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Window `id`'s size as it shows now.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_size<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: jint,
) -> JIntArray<'l> {
    env.with_env(|env| {
        let s = with_rt(env, h, |r| r.rt.window_size(WindowId(id as u32)))?;
        jints(env, &[s.0, s.1])
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Sizes window `id` to `width` by `height` where it is.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_resize_window<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: jint,
    width: jint,
    height: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.resize_window(WindowId(id as u32), width, height)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether a window besides the main one is open.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_window_open<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jboolean {
    env.with_env(|env| with_rt(env, h, |r| r.rt.window_open() as jboolean))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Closes the window opened last.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_close_top_window<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.close_top_window()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The space the app has for the stack; 0 by 0 leaves each window at its size.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_set_view_size<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    w: jint,
    height: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.set_view_size(if w > 0 && height > 0 {
                Some((w, height))
            } else {
                None
            })
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The space the app has for the stack, or an empty array for none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_view_size<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let v = with_rt(env, h, |r| r.rt.view_size())?;
        pair(env, v)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The screen scripts may place windows on before there is a view size.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_set_viewport<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    w: jint,
    height: jint,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.set_viewport(w, height)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The screen as scripts ask for it.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_viewport<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let v = with_rt(env, h, |r| r.rt.viewport())?;
        jints(env, &[v.0, v.1])
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// How many times a script placed the main window against an edge after a menu pick.
/// What getSkinName() answers.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_skin_name<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JString<'l> {
    env.with_env(|env| {
        let name = with_rt(env, h, |r| r.rt.skin_name().to_string())?;
        env.new_string(&name)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Names the skin for its scripts, before or after it loads.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_set_skin_name<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    name: JString<'l>,
) {
    env.with_env(|env| {
        let name = name.to_string();
        with_rt(env, h, |r| r.rt.set_skin_name(&name))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// How the host shows the windows: whether there is a view size and its width and height, the
/// viewport's width and height, the columns, the column width, whether the last list window
/// stretches, and the touch slop.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_presentation<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let p = with_rt(env, h, |r| r.rt.presentation())?;
        let (vw, vh) = p.view_size.unwrap_or((0, 0));
        jints(
            env,
            &[
                p.view_size.is_some() as i32,
                vw,
                vh,
                p.viewport.0,
                p.viewport.1,
                p.columns,
                p.column_width,
                p.stretch_last as i32,
                p.touch_slop,
            ],
        )
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Sets how the host shows the windows, in the order `rtPresentation` reads them.
#[jni_mangle("nl.mattix.walkit.Native")]
#[allow(clippy::too_many_arguments)]
pub fn rt_set_presentation<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    has_view: jboolean,
    view_w: jint,
    view_h: jint,
    viewport_w: jint,
    viewport_h: jint,
    columns: jint,
    column_width: jint,
    stretch_last: jboolean,
    touch_slop: jint,
) {
    env.with_env(|env| {
        let p = Presentation {
            view_size: has_view.then_some((view_w, view_h)),
            viewport: (viewport_w, viewport_h),
            columns,
            column_width,
            stretch_last,
            touch_slop,
        };
        with_rt(env, h, |r| r.rt.set_presentation(&p))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Where window `id` visibly ends on the right; with `lay_out_first` after laying it out.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_content_right<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: jint,
    lay_out_first: jboolean,
) -> jint {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.content_right_of(WindowId(id as u32), lay_out_first)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether a window docked under the main one shows a list.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_list_window_open<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jboolean {
    env.with_env(|env| with_rt(env, h, |r| r.rt.list_window_open()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// How far right the stack shows in one column while the main window shows `width` across (0
/// for what it shows now).
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_shown_width<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    width: jint,
) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.shown_width(width)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// How large the stack is in `columns` at every window's natural height, with a right column
/// `column_width` wide; empty without a main window.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_natural_stack_size<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    columns: jint,
    column_width: jint,
) -> JIntArray<'l> {
    env.with_env(|env| {
        let size = with_rt(env, h, |r| r.rt.natural_stack_size(columns, column_width))?;
        pair(env, size)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The left column of the stack in two columns, as a rect; empty for a stack in one column.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_left_column<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let r = with_rt(env, h, |r| r.rt.left_column())?;
        match r {
            Some(r) => jints(env, &[r.left, r.top, r.width, r.height]),
            None => jints(env, &[]),
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Where the visualizer shows in the stack, as a rect; empty when none shows.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_vis_rect<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let r = with_rt(env, h, |r| r.rt.vis_rect())?;
        match r {
            Some(r) => jints(env, &[r.left, r.top, r.width, r.height]),
            None => jints(env, &[]),
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The host closed its visualizer window: what a script had it show goes back into the skin.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_visualizer_window_closed<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.visualizer_window_closed()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_edge_placements<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.edge_placements()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

// ---- themes, settings, stats -----------------------------------------------------------------

/// The skin's color themes, in the order the skin defines them.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_color_themes<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
) -> JObjectArray<'l, JString<'l>> {
    env.with_env(|env| {
        let names = with_rt(env, h, |r| r.rt.color_themes())?;
        let empty = env.new_string("")?;
        let arr = JObjectArray::<JString>::new(env, names.len(), &empty)?;
        for (i, n) in names.iter().enumerate() {
            let s = env.new_string(n)?;
            arr.set_element(env, i, &s)?;
        }
        Ok::<_, Error>(arr)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The color theme in use, or null.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_color_theme<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JString<'l> {
    env.with_env(|env| {
        let t = with_rt(env, h, |r| r.rt.color_theme().map(|s| s.to_string()))?;
        jstring(env, t.as_deref())
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Wears the color theme named, matched ignoring case.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_select_color_theme<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    name: JString<'l>,
) {
    env.with_env(|env| {
        let n = name.to_string();
        with_rt(env, h, |r| r.rt.select_color_theme(&n))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Counts changes to what a host keeps of the skin between runs.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_settings_version<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.settings_version()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// What the host keeps of the skin between runs: the attributes, the strings and the ints, each
/// as a count and that many key-value pairs (the value a string, a string, an int).
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_saved_settings<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let out = with_rt(env, h, |r| {
            let saved = r.rt.saved_settings();
            let mut out = Out::default();
            out.i32(saved.attributes.len() as i32);
            for (k, v) in &saved.attributes {
                out.string(k);
                out.string(v);
            }
            out.i32(saved.strings.len() as i32);
            for (k, v) in &saved.strings {
                out.string(k);
                out.string(v);
            }
            out.i32(saved.ints.len() as i32);
            for (k, v) in &saved.ints {
                out.string(k);
                out.i32(*v);
            }
            out
        })?;
        out.bytes(env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Puts back what `rtSavedSettings` gave in an earlier run, in the same form, before the skin loads.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_restore_settings<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    saved: JByteArray<'l>,
) {
    env.with_env(|env| {
        let bytes = bytes_of(env, saved.into())?.unwrap_or_default();
        let mut r = In {
            bytes: &bytes,
            at: 0,
        };
        let read = (|| {
            Some(SavedSettings {
                attributes: r.pairs(In::string)?.into_iter().collect(),
                strings: r.pairs(In::string)?.into_iter().collect(),
                ints: r.pairs(In::i32)?.into_iter().collect(),
            })
        })();
        let Some(saved) = read else {
            return Err(throw(env, "not packed settings"));
        };
        with_rt(env, h, |r| r.rt.restore_settings(&saved))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Counts changes to the skin's Config items.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_config_version<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.config_version()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

fn option_entries(out: &mut Out, entries: &[OptionEntry]) {
    out.i32(entries.len() as i32);
    for e in entries {
        match e {
            OptionEntry::Toggle {
                key,
                text,
                checked,
                radio,
            } => {
                out.i32(1);
                out.string(&key.item);
                out.string(&key.attribute);
                out.string(text);
                out.i32(*checked as i32);
                out.i32(*radio as i32);
            }
            OptionEntry::Separator => out.i32(2),
            OptionEntry::Submenu { text, entries } => {
                out.i32(3);
                out.string(text);
                option_entries(out, entries);
            }
        }
    }
}

/// The options the skin's scripts list for Winamp's Options menu: the config version, then
/// the entries as a count and each as a kind (1 toggle: item, attribute, text, checked,
/// radio; 2 separator; 3 submenu: text, then its entries).
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_skin_options<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let out = with_rt(env, h, |r| {
            let options = r.rt.skin_options();
            let mut out = Out::default();
            out.i32(options.version);
            option_entries(&mut out, &options.entries);
            out
        })?;
        out.bytes(env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Flips the option of attribute `attribute` (lowercase) of the Config item `item` (a GUID);
/// false for an unknown key, a separator or a submenu.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_toggle_skin_option<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    item: JString<'l>,
    attribute: JString<'l>,
) -> jboolean {
    env.with_env(|env| {
        let key = OptionKey {
            item: item.to_string(),
            attribute: attribute.to_string(),
        };
        with_rt(env, h, |r| r.rt.toggle_skin_option(&key))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// What the engine has done so far: script steps, timer fires, layout passes, shapes worked out.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_stats<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JLongArray<'l> {
    env.with_env(|env| {
        let (s, plan_nanos, full, quick) = with_rt(env, h, |r| {
            (
                r.rt.stats(),
                r.plan_nanos,
                r.planner.full_plans(),
                r.planner.quick_plans(),
            )
        })?;
        let arr = env.new_long_array(7)?;
        arr.set_region(
            env,
            0,
            &[
                s.script_steps,
                s.timer_fires,
                s.layout_passes,
                s.shapes_worked_out,
                plan_nanos as i64,
                full as i64,
                quick as i64,
            ],
        )?;
        Ok::<_, Error>(arr)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ---- layout and frames -----------------------------------------------------------------------

/// Lays the main window out now.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_ensure_layout<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.ensure_layout()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The stack's size.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_stack_size<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let s = with_rt(env, h, |r| r.rt.stack_size())?;
        jints(env, &[s.0, s.1])
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The part of the stack its windows show.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_drawn_size<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JIntArray<'l> {
    env.with_env(|env| {
        let s = with_rt(env, h, |r| r.rt.drawn_size())?;
        jints(env, &[s.0, s.1])
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether a target slide runs now.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_sliding<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jboolean {
    env.with_env(|env| with_rt(env, h, |r| r.rt.sliding() as jboolean))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether something moves by itself right now.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_animating<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jboolean {
    env.with_env(|env| with_rt(env, h, |r| r.rt.animating() as jboolean))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Whether something on the skin wants a frame on every display frame.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_frames_wanted<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jboolean {
    env.with_env(|env| with_rt(env, h, |r| r.rt.frames_wanted() as jboolean))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// When, in clock ms, something on the skin next changes by itself; `Long.MIN_VALUE` when nothing will.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_next_due_at<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jlong {
    env.with_env(|env| with_rt(env, h, |r| r.rt.next_due_at().unwrap_or(jlong::MIN)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The engine asks for a redraw.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_invalidate<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.invalidate()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

// ---- touches ---------------------------------------------------------------------------------

/// The finger goes down at a point of the stack; `seq` names the touch.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_down<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    x: jint,
    y: jint,
    seq: jint,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.pointer_down(x, y, seq)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The finger moves to a point of the stack.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_move<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    x: jint,
    y: jint,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.pointer_move(x, y)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The finger lifts at a point of the stack.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_up<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong, x: jint, y: jint) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.pointer_up(x, y)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The finger goes down at a point of window `window`, in that window's pixels.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_down_in<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    window: jint,
    x: jint,
    y: jint,
    seq: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.pointer_down_in(WindowId(window as u32), x, y, seq)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The finger moves to a point of window `window`, in that window's pixels.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_move_in<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    window: jint,
    x: jint,
    y: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.pointer_move_in(WindowId(window as u32), x, y)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The finger lifts at a point of window `window`, in that window's pixels.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_up_in<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    window: jint,
    x: jint,
    y: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.pointer_up_in(WindowId(window as u32), x, y)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// A second tap of a double tap, at a point of the stack.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_double_tap<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    x: jint,
    y: jint,
    seq: jint,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.double_tap(x, y, seq)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// A second tap of a double tap, at a point of window `window`, in that window's pixels.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_double_tap_in<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    window: jint,
    x: jint,
    y: jint,
    seq: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            r.rt.double_tap_in(WindowId(window as u32), x, y, seq)
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The seq of the last press that landed on an object with a skin menu, or `Int.MIN_VALUE`.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_menu_target<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.menu_target().unwrap_or(jint::MIN)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The seq of the last press that moves the window, or `Int.MIN_VALUE`.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_move_target<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jint {
    env.with_env(|env| with_rt(env, h, |r| r.rt.move_target().unwrap_or(jint::MIN)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// A right click on the object the touch `seq` landed on.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_right_click<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong, seq: jint) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.right_click(seq)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Ends the press under the finger without a release.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_pointer_cancel<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.pointer_cancel()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Starts a touch the listener made, in which a script may pop a menu; `fresh` starts the count over.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_gesture_begin<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    fresh: jboolean,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.gesture_begin(fresh)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Ends the touch `rtGestureBegin` started.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_gesture_end<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.gesture_end()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The listener's pick from the menu (packed as `walkit::packed::encode_menu` writes), by
/// command id, or 0; a buffer that is not a menu is an error.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_menu_choice<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    menu: JByteArray<'l>,
) -> jint {
    env.with_env(|env| {
        let bytes = bytes_of(env, menu.into())?.unwrap_or_default();
        let Some(menu) = walkit::packed::decode_menu(&bytes) else {
            return Err(throw(env, "not a packed menu"));
        };
        with_rt(env, h, |r| r.rt.menu_choice(&menu))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

// ---- what a test does as a script would (`nl.mattix.walkit.fixtures`) --------------------------
//
// A handle crosses as a kind (0 object, 1 container, 2 System, 3 made) and an index, -1 for
// none. A value crosses packed: a tag (0 null, 1 int, 2 string, 3 handle), then the int, the
// string, or the kind and index.

fn handle_of(kind: jint, index: jint) -> Option<Handle> {
    if index < 0 {
        return None;
    }
    let i = index as usize;
    Some(match kind {
        0 => Handle::Gui(i),
        1 => Handle::Container(i),
        2 => Handle::System(i),
        _ => Handle::Made(i),
    })
}

fn kind_index(h: Handle) -> (i32, i32) {
    match h {
        Handle::Gui(i) => (0, i as i32),
        Handle::Container(i) => (1, i as i32),
        Handle::System(i) => (2, i as i32),
        Handle::Made(i) => (3, i as i32),
    }
}

fn handle_index(h: Option<Handle>) -> jint {
    h.map(|h| kind_index(h).1).unwrap_or(-1)
}

impl Out {
    fn value(&mut self, v: &Value) {
        match v {
            Value::Null => self.i32(0),
            Value::Int(i) => {
                self.i32(1);
                self.i32(*i);
            }
            Value::Num(n) => {
                self.i32(1);
                self.i32(*n as i32);
            }
            Value::Bool(b) => {
                self.i32(1);
                self.i32(*b as i32);
            }
            Value::Str(s) => {
                self.i32(2);
                self.string(s);
            }
            Value::Obj(h) => {
                let (k, i) = kind_index(*h);
                self.i32(3);
                self.i32(k);
                self.i32(i);
            }
        }
    }
}

impl In<'_> {
    fn value(&mut self) -> Option<Value> {
        Some(match self.i32()? {
            0 => Value::Null,
            1 => Value::Int(self.i32()?),
            2 => Value::string(self.string()?),
            _ => Value::Obj(handle_of(self.i32()?, self.i32()?)?),
        })
    }
}

/// The object with id `id` in the main window, as a script finds it; -1 for none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_object<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: JString<'l>,
) -> jint {
    env.with_env(|env| {
        let id = id.to_string();
        with_rt(env, h, |r| handle_index(r.rt.script_object(&id)))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The layout of window `window`, as a script holds it; -1 for none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_layout<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    window: jint,
) -> jint {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            handle_index(r.rt.script_layout(WindowId(window as u32)))
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The container with id `id`, as a script finds it; -1 for none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_container<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    id: JString<'l>,
) -> jint {
    env.with_env(|env| {
        let id = id.to_string();
        with_rt(env, h, |r| handle_index(r.rt.script_container(&id)))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// An object of class `class` as a script's `new` makes it; -1 when it cannot be made.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_make<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    class: JString<'l>,
) -> jint {
    env.with_env(|env| {
        let class = class.to_string();
        with_rt(env, h, |r| handle_index(r.rt.script_make(&class)))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// The `Config` item `name` with `guid`, as scripts reach it; -1 for none.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_config_item<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    name: JString<'l>,
    guid: JString<'l>,
) -> jint {
    env.with_env(|env| {
        let name = name.to_string();
        let guid = guid.to_string();
        with_rt(env, h, |r| {
            handle_index(r.rt.script_config_item(&name, &guid))
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Runs `method` on the handle with packed `args`; the packed answer, or null for no such method.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_call<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    kind: jint,
    index: jint,
    method: JString<'l>,
    args: JByteArray<'l>,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let method = method.to_string();
        let bytes = bytes_of(env, args.into())?.unwrap_or_default();
        let mut r = In {
            bytes: &bytes,
            at: 0,
        };
        let read = (|| {
            let n = r.i32()?;
            (0..n).map(|_| r.value()).collect::<Option<Vec<Value>>>()
        })();
        let Some(args) = read else {
            return Err(throw(env, "not packed script arguments"));
        };
        let Some(receiver) = handle_of(kind, index) else {
            return Err(throw(env, "no such object"));
        };
        let out = with_rt(env, h, |r| {
            r.rt.script_call(receiver, &method, &args).map(|v| {
                let mut out = Out::default();
                out.value(&v);
                out
            })
        })?;
        match out {
            Some(out) => out.bytes(env),
            None => Ok(JByteArray::null()),
        }
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Puts the object on its way to a target, as `gotoTarget` does; see `rtScriptEndSlides`.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_begin_slide<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    index: jint,
) {
    env.with_env(|env| {
        with_rt(env, h, |r| {
            if let Some(o) = handle_of(0, index) {
                r.rt.begin_slide_as_script(o);
            }
        })
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Ends every slide, as the slides reaching their targets do.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_end_slides<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.end_slides_as_script()))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// When the next of the skin's timers fires, in the clock's ms; `Long.MIN_VALUE` when none runs.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_next_timer_at<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> jlong {
    env.with_env(|env| with_rt(env, h, |r| r.rt.next_timer_at().unwrap_or(i64::MIN)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Starts or stops writing down the events the engine fires, for `rtScriptTakeEvents`.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_record_events<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    on: jboolean,
) {
    env.with_env(|env| with_rt(env, h, |r| r.rt.record_events(on)))
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// The events fired since the last take: a count, then each as a kind, an index, the event's
/// name, and its arguments as a count and packed values.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_script_take_events<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let out = with_rt(env, h, |r| {
            let events = r.rt.take_recorded_events();
            let mut out = Out::default();
            out.i32(events.len() as i32);
            for e in events {
                let (k, i) = kind_index(e.target);
                out.i32(k);
                out.i32(i);
                out.string(&e.event);
                out.i32(e.args.len() as i32);
                for a in &e.args {
                    out.value(a);
                }
            }
            out
        })?;
        out.bytes(env)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}
