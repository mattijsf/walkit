// SPDX-License-Identifier: GPL-3.0-or-later

//! The host of a runtime as the Kotlin side answers it: every question goes to the
//! `HostBridge`, whose methods take a font as its id and file (two strings, null for none), a
//! menu as the packed bytes `walkit::packed::encode_menu` writes, and the open windows as an
//! int array of ids. A question the bridge cannot answer (an exception on its side) gets the
//! engine's default and is reported by the native call in progress.

use std::rc::Rc;

use jni::objects::JValue;
use jni::{jni_sig, jni_str};
use walkit::draw::FontDef;
use walkit::host::{SkinHost, SkinMenu, WindowId};

use crate::bridge::{bytes_of, floats_of, ints_of, jints, jstring, Bridge};

/// The host bridge as the engine asks its host.
pub struct JniHost(pub Bridge);

impl SkinHost for JniHost {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        self.0
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

    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        self.pixels(file, x, y, 1, 1)
            .and_then(|v| v.first().copied())
    }

    fn pixels(&mut self, file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
        self.0
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

    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        self.0
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

    fn text_advance(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        self.0
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

    fn line_metrics(
        &mut self,
        font: Option<&FontDef>,
        size: i32,
        bold: bool,
    ) -> Option<(f32, f32)> {
        self.0
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

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        self.0
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
        self.0.with(|env, obj| {
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
        self.0.with(|env, obj| {
            env.call_method(obj, jni_str!("invalidate"), jni_sig!("()V"), &[])?;
            Ok(())
        });
    }

    fn window_open(&mut self, kind: &str) -> Option<bool> {
        self.0
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

    fn choose_from_menu(&mut self, menu: &SkinMenu) -> Option<i32> {
        let packed = walkit::packed::encode_menu(menu);
        self.0
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
        self.0.with(|env, obj| {
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
        self.0.with(|env, obj| {
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
