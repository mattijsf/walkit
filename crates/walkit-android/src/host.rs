// SPDX-License-Identifier: GPL-3.0-or-later

//! The host of a runtime as the Kotlin side answers it: every question goes to the
//! `HostBridge`, whose methods take a font as its id and file (two strings, null for none), a
//! menu as the packed bytes `walkit::packed::encode_menu` writes, and the open windows as an
//! int array of ids. A question the bridge cannot answer (an exception on its side) gets the
//! engine's default and is reported by the native call in progress.
//!
//! What never changes for a skin is asked once and kept: an image's size, a text's width or
//! advance, a font's line metrics. Whether a window is open is asked once per native call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use jni::objects::JValue;
use jni::{jni_sig, jni_str};
use walkit::draw::FontDef;
use walkit::host::{SkinHost, SkinMenu, WindowId};

use crate::bridge::{bytes_of, floats_of, ints_of, jints, jstring, Bridge};

/// A text as the host measures it: the text, the font's id and file, the size, bold.
type TextKey = (String, Option<(String, String)>, i32, bool);

/// A font as the host gives its line metrics: the font's id and file, the size, bold.
type FontKey = (Option<(String, String)>, i32, bool);

/// More measured texts than this, and the measurements start over.
const TEXTS_KEPT: usize = 8192;

/// What a native call has asked of the host that may be answered otherwise by the next.
#[derive(Default)]
pub struct HostCache {
    open: RefCell<HashMap<String, Option<bool>>>,
}

impl HostCache {
    /// Forgets the call's answers.
    pub fn clear(&self) {
        self.open.borrow_mut().clear();
    }
}

/// The host bridge as the engine asks its host, with what it has answered before.
pub struct JniHost {
    bridge: Bridge,
    sizes: HashMap<String, Option<(i32, i32)>>,
    widths: HashMap<TextKey, i32>,
    advances: HashMap<TextKey, f32>,
    metrics: HashMap<FontKey, Option<(f32, f32)>>,
    cache: Rc<HostCache>,
}

fn font_key(font: Option<&FontDef>) -> Option<(String, String)> {
    font.map(|f| (f.id.to_string(), f.file.to_string()))
}

impl JniHost {
    pub fn new(bridge: Bridge, cache: Rc<HostCache>) -> JniHost {
        JniHost {
            bridge,
            sizes: HashMap::new(),
            widths: HashMap::new(),
            advances: HashMap::new(),
            metrics: HashMap::new(),
            cache,
        }
    }

    fn ask_image_size(&self, file: &str) -> Option<(i32, i32)> {
        self.bridge
            .with(|env, obj| {
                let f = env.new_string(file)?;
                let r = env
                    .call_method(
                        obj,
                        jni_str!("imageSize"),
                        jni_sig!("(Ljava/lang/String;)[I"),
                        &[JValue::Object(&f)],
                    )?
                    .l()?;
                Ok(ints_of(env, r)?
                    .filter(|v| v.len() == 2)
                    .map(|v| (v[0], v[1])))
            })
            .flatten()
    }

    fn ask_text_width(&self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        self.bridge
            .with(|env, obj| {
                let t = env.new_string(text)?;
                let id = jstring(env, font.map(|f| &*f.id))?;
                let file = jstring(env, font.map(|f| &*f.file))?;
                env.call_method(
                    obj,
                    jni_str!("textWidth"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;IZ)I"),
                    &[
                        JValue::Object(&t),
                        JValue::Object(&id),
                        JValue::Object(&file),
                        JValue::Int(size),
                        JValue::Bool(bold),
                    ],
                )?
                .i()
            })
            .unwrap_or(0)
    }

    fn ask_text_advance(&self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        self.bridge
            .with(|env, obj| {
                let t = env.new_string(text)?;
                let id = jstring(env, font.map(|f| &*f.id))?;
                let file = jstring(env, font.map(|f| &*f.file))?;
                env.call_method(
                    obj,
                    jni_str!("textAdvance"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;IZ)F"),
                    &[
                        JValue::Object(&t),
                        JValue::Object(&id),
                        JValue::Object(&file),
                        JValue::Int(size),
                        JValue::Bool(bold),
                    ],
                )?
                .f()
            })
            .unwrap_or(0.0)
    }

    fn ask_line_metrics(
        &self,
        font: Option<&FontDef>,
        size: i32,
        bold: bool,
    ) -> Option<(f32, f32)> {
        self.bridge
            .with(|env, obj| {
                let id = jstring(env, font.map(|f| &*f.id))?;
                let file = jstring(env, font.map(|f| &*f.file))?;
                let r = env
                    .call_method(
                        obj,
                        jni_str!("lineMetrics"),
                        jni_sig!("(Ljava/lang/String;Ljava/lang/String;IZ)[F"),
                        &[
                            JValue::Object(&id),
                            JValue::Object(&file),
                            JValue::Int(size),
                            JValue::Bool(bold),
                        ],
                    )?
                    .l()?;
                Ok(floats_of(env, r)?
                    .filter(|v| v.len() == 2)
                    .map(|v| (v[0], v[1])))
            })
            .flatten()
    }

    fn ask_window_open(&self, kind: &str) -> Option<bool> {
        self.bridge
            .with(|env, obj| {
                let k = env.new_string(kind)?;
                env.call_method(
                    obj,
                    jni_str!("windowOpen"),
                    jni_sig!("(Ljava/lang/String;)I"),
                    &[JValue::Object(&k)],
                )?
                .i()
            })
            .and_then(|v| match v {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            })
    }
}

impl SkinHost for JniHost {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        if let Some(size) = self.sizes.get(file) {
            return *size;
        }
        let size = self.ask_image_size(file);
        self.sizes.insert(file.to_string(), size);
        size
    }

    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        let key = (text.to_string(), font_key(font), size, bold);
        if let Some(w) = self.widths.get(&key) {
            return *w;
        }
        if self.widths.len() >= TEXTS_KEPT {
            self.widths.clear();
        }
        let w = self.ask_text_width(text, font, size, bold);
        self.widths.insert(key, w);
        w
    }

    fn text_advance(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        let key = (text.to_string(), font_key(font), size, bold);
        if let Some(a) = self.advances.get(&key) {
            return *a;
        }
        if self.advances.len() >= TEXTS_KEPT {
            self.advances.clear();
        }
        let a = self.ask_text_advance(text, font, size, bold);
        self.advances.insert(key, a);
        a
    }

    fn line_metrics(
        &mut self,
        font: Option<&FontDef>,
        size: i32,
        bold: bool,
    ) -> Option<(f32, f32)> {
        let key = (font_key(font), size, bold);
        if let Some(m) = self.metrics.get(&key) {
            return *m;
        }
        let m = self.ask_line_metrics(font, size, bold);
        self.metrics.insert(key, m);
        m
    }

    fn window_open(&mut self, kind: &str) -> Option<bool> {
        if let Some(open) = self.cache.open.borrow().get(kind) {
            return *open;
        }
        let open = self.ask_window_open(kind);
        self.cache.open.borrow_mut().insert(kind.to_string(), open);
        open
    }

    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        self.pixels(file, x, y, 1, 1)
            .and_then(|v| v.first().copied())
    }

    fn pixels(&mut self, file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
        self.bridge
            .with(|env, obj| {
                let f = env.new_string(file)?;
                let r = env
                    .call_method(
                        obj,
                        jni_str!("pixels"),
                        jni_sig!("(Ljava/lang/String;IIII)[I"),
                        &[
                            JValue::Object(&f),
                            JValue::Int(x),
                            JValue::Int(y),
                            JValue::Int(w),
                            JValue::Int(h),
                        ],
                    )?
                    .l()?;
                Ok(ints_of(env, r)?.map(|v| v.into_iter().map(|p| p as u32).collect()))
            })
            .flatten()
    }

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        self.bridge
            .with(|env, obj| {
                let f = env.new_string(file)?;
                let r = env
                    .call_method(
                        obj,
                        jni_str!("fontFile"),
                        jni_sig!("(Ljava/lang/String;)[B"),
                        &[JValue::Object(&f)],
                    )?
                    .l()?;
                bytes_of(env, r)
            })
            .flatten()
            .map(Rc::from)
    }

    fn log(&mut self, message: &str) {
        self.bridge.with(|env, obj| {
            let m = env.new_string(message)?;
            env.call_method(
                obj,
                jni_str!("log"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&m)],
            )?;
            Ok(())
        });
    }

    fn invalidate(&mut self) {
        self.bridge.with(|env, obj| {
            env.call_method(obj, jni_str!("invalidate"), jni_sig!("()V"), &[])?;
            Ok(())
        });
    }

    fn choose_from_menu(&mut self, menu: &SkinMenu) -> Option<i32> {
        let packed = walkit::packed::encode_menu(menu);
        self.bridge
            .with(|env, obj| {
                let m = env.byte_array_from_slice(&packed)?;
                env.call_method(
                    obj,
                    jni_str!("chooseFromMenu"),
                    jni_sig!("([B)I"),
                    &[JValue::Object(&m)],
                )?
                .i()
            })
            .filter(|&v| v != i32::MIN)
    }

    fn windows_changed(&mut self, windows: &[WindowId]) {
        let ids: Vec<i32> = windows.iter().map(|w| w.0 as i32).collect();
        self.bridge.with(|env, obj| {
            let arr = jints(env, &ids)?;
            env.call_method(
                obj,
                jni_str!("windowsChanged"),
                jni_sig!("([I)V"),
                &[JValue::Object(&arr)],
            )?;
            Ok(())
        });
    }

    fn host_action(&mut self, action: &str, param: Option<&str>) {
        self.bridge.with(|env, obj| {
            let a = env.new_string(action)?;
            let p = jstring(env, param)?;
            env.call_method(
                obj,
                jni_str!("hostAction"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                &[JValue::Object(&a), JValue::Object(&p)],
            )?;
            Ok(())
        });
    }
}
