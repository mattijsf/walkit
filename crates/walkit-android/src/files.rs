// SPDX-License-Identifier: GPL-3.0-or-later

//! The skin's files as the Kotlin side holds them: every read goes to the `FilesBridge`
//! (`read(path): ByteArray?`, `names(): Array<String>`), so the stack of files (a skin, its
//! siblings, the wasabi base) is composed on the Kotlin side and copied nowhere.

use std::rc::Rc;

use jni::objects::JValue;
use jni::{jni_sig, jni_str};
use walkit::files::SkinFiles;

use crate::bridge::{strings_of, Bridge};

/// The files bridge as the engine reads files.
pub struct JniFiles(pub Bridge);

impl SkinFiles for JniFiles {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        self.0
            .with(|env, obj| {
                let p = env.new_string(path)?;
                let r = env
                    .call_method(
                        obj,
                        jni_str!("read"),
                        jni_sig!("(Ljava/lang/String;)[B"),
                        &[JValue::Object(&p)],
                    )?
                    .l()?;
                crate::bridge::bytes_of(env, r)
            })
            .flatten()
            .map(Rc::from)
    }

    fn names(&self) -> Vec<String> {
        self.0
            .with(|env, obj| {
                let r = env
                    .call_method(
                        obj,
                        jni_str!("names"),
                        jni_sig!("()[Ljava/lang/String;"),
                        &[],
                    )?
                    .l()?;
                strings_of(env, r)
            })
            .flatten()
            .unwrap_or_default()
    }
}
