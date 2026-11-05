// SPDX-License-Identifier: GPL-3.0-or-later

//! A Kotlin object the engine calls back into, held across calls: the VM and a global
//! reference, and the calls on it as the engine makes them (an int for an int, a string for a
//! string, arrays for the rest).

use std::cell::RefCell;
use std::rc::Rc;

use jni::errors::{Error, Result};
use jni::objects::{JByteArray, JFloatArray, JIntArray, JObject, JObjectArray, JString};
use jni::refs::Global;
use jni::{Env, JavaVM};

/// The first error a callback met, kept for the native call that is running to report.
pub type Failure = Rc<RefCell<Option<String>>>;

/// A Kotlin bridge object and the VM it lives in.
pub struct Bridge {
    vm: JavaVM,
    obj: Global<JObject<'static>>,
    /// What went wrong in a callback, kept until the native call reports it.
    pub failure: Failure,
}

impl Bridge {
    /// Holds `obj` for as long as the engine needs it; a callback that fails writes into `failure`.
    pub fn new(env: &Env, obj: &JObject, failure: Failure) -> Result<Bridge> {
        Ok(Bridge {
            vm: env.get_java_vm()?,
            obj: env.new_global_ref(obj)?,
            failure,
        })
    }

    /// Runs `f` on the bridge object in a fresh local frame, on the thread the engine
    /// runs on (attached, as a native call is in progress). A failure is kept in `failure`
    /// and gives `None`; the first one is what the native call reports.
    pub fn with<T>(&self, f: impl FnOnce(&mut Env, &JObject) -> Result<T>) -> Option<T> {
        let obj = self.obj.as_obj();
        let r: Result<T> = self.vm.with_local_frame(16, |env| {
            let r = f(env, obj);
            if r.is_err() && env.exception_check() {
                env.exception_describe();
                env.exception_clear();
            }
            r
        });
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                let mut failure = self.failure.borrow_mut();
                if failure.is_none() {
                    *failure = Some(format!("{e}"));
                }
                None
            }
        }
    }
}

/// A Java string as Rust text.
pub fn string_of(env: &mut Env, obj: JObject) -> Result<String> {
    if obj.is_null() {
        return Err(Error::NullPtr("string"));
    }
    let s = JString::cast_local(env, obj)?;
    Ok(s.to_string())
}

/// The elements of an int array, `None` for null.
pub fn ints_of(env: &mut Env, obj: JObject) -> Result<Option<Vec<i32>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let arr = JIntArray::cast_local(env, obj)?;
    let mut v = vec![0i32; arr.len(env)?];
    arr.get_region(env, 0, &mut v)?;
    Ok(Some(v))
}

/// The elements of a float array, `None` for null.
pub fn floats_of(env: &mut Env, obj: JObject) -> Result<Option<Vec<f32>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let arr = JFloatArray::cast_local(env, obj)?;
    let mut v = vec![0f32; arr.len(env)?];
    arr.get_region(env, 0, &mut v)?;
    Ok(Some(v))
}

/// The bytes of a byte array, `None` for null.
pub fn bytes_of(env: &mut Env, obj: JObject) -> Result<Option<Vec<u8>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let arr = JByteArray::cast_local(env, obj)?;
    Ok(Some(env.convert_byte_array(&arr)?))
}

/// The elements of a string array, `None` for null.
pub fn strings_of(env: &mut Env, obj: JObject) -> Result<Option<Vec<String>>> {
    if obj.is_null() {
        return Ok(None);
    }
    let arr = JObjectArray::<JString>::cast_local(env, obj)?;
    let n = arr.len(env)?;
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let s: JString = arr.get_element(env, i)?;
        v.push(s.to_string());
    }
    Ok(Some(v))
}

/// Throws a `RuntimeException` with `msg`; the native call returns its default after.
pub fn throw(env: &mut Env, msg: &str) -> Error {
    let class = jni::jni_str!("java/lang/RuntimeException");
    let text = jni::strings::JNIString::from(msg);
    match env.throw_new(class, &text) {
        Ok(()) => Error::JavaException,
        Err(e) => e,
    }
}

/// A Rust string as a Java one, or null for `None`.
pub fn jstring<'l>(env: &mut Env<'l>, s: Option<&str>) -> Result<JString<'l>> {
    match s {
        Some(s) => env.new_string(s),
        None => Ok(JString::null()),
    }
}

/// An int array from a slice.
pub fn jints<'l>(env: &mut Env<'l>, v: &[i32]) -> Result<JIntArray<'l>> {
    let arr = env.new_int_array(v.len())?;
    arr.set_region(env, 0, v)?;
    Ok(arr)
}
