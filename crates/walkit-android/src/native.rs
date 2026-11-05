// SPDX-License-Identifier: GPL-3.0-or-later

//! The native methods of `nl.mattix.walkit.Native`, one per engine operation. A runtime is a
//! handle (`long`) made by `rtNew` and dropped by `rtFree`; every other method takes it first.
//! Sizes and rects come back as int arrays, a window's or an object's description as a small
//! packed buffer (ints, then strings as a length and UTF-8 bytes), a frame as the buffer of
//! `walkit::packed`. "None" answers are -1 for ids, `Int.MIN_VALUE` for seqs, `Long.MIN_VALUE`
//! for a time, an empty array for a size.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use jni::errors::{Error, Result, ThrowRuntimeExAndDefault};
use jni::objects::{JByteArray, JClass, JIntArray, JLongArray, JObject, JObjectArray, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::{jni_mangle, Env, EnvUnowned};
use walkit::host::{Edge, SkinSetup, WindowId};
use walkit::packed::PackedWindow;
use walkit::SkinRuntime;

use crate::bridge::{bytes_of, jints, jstring, throw, Bridge, Failure};
use crate::files::JniFiles;
use crate::host::JniHost;
use crate::player::JniPlayer;

/// A runtime the Kotlin side drives, with the clock it sets.
struct Rt {
    rt: SkinRuntime,
    now: Rc<Cell<i64>>,
    zone_offset: Rc<Cell<i32>>,
    failure: Failure,
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

/// Runs `f` on the runtime of `h`, then reports what a callback failed with, if one did.
fn with_rt<T>(env: &mut Env, h: jlong, f: impl FnOnce(&mut Rt) -> T) -> Result<T> {
    let r = rt(h)?;
    let out = f(r);
    if let Some(msg) = r.failure.borrow_mut().take() {
        return Err(throw(env, &format!("walkit host callback failed: {msg}")));
    }
    Ok(out)
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
        let host = JniHost(Bridge::new(env, &host, failure.clone())?);
        let player = JniPlayer(Bridge::new(env, &player, failure.clone())?);
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
        if let Some(msg) = failure.borrow_mut().take() {
            return Err(throw(env, &format!("walkit host callback failed: {msg}")));
        }
        let boxed = Box::new(Rt {
            rt,
            now,
            zone_offset,
            failure,
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

/// Every window's drawing as the packed frame of `walkit::packed`.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_draw_windows<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
) -> JByteArray<'l> {
    env.with_env(|env| {
        let bytes = with_rt(env, h, |r| {
            let ids = r.rt.window_ids();
            let drawn = r.rt.draw_windows();
            let infos: Vec<_> = ids
                .iter()
                .map(|id| (r.rt.window_info(*id), r.rt.window_rect(*id)))
                .collect();
            let windows: Vec<PackedWindow> = ids
                .iter()
                .zip(drawn.iter())
                .zip(infos.iter())
                .map(|((id, (_, cmds)), (info, rect))| PackedWindow {
                    id: id.0,
                    container: &info.container,
                    layout: &info.layout,
                    rect: *rect,
                    cmds,
                })
                .collect();
            walkit::packed::encode(&windows)
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

/// What the engine has done so far: script steps, timer fires, layout passes, shapes worked out.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn rt_stats<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) -> JLongArray<'l> {
    env.with_env(|env| {
        let s = with_rt(env, h, |r| r.rt.stats())?;
        let arr = env.new_long_array(4)?;
        arr.set_region(
            env,
            0,
            &[
                s.script_steps,
                s.timer_fires,
                s.layout_passes,
                s.shapes_worked_out,
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
