// SPDX-License-Identifier: GPL-3.0-or-later

//! The engine for Android: `libwalkit.so`, a JNI library the Kotlin side of the `walkit` AAR
//! drives. The surface is the one the web module has (a runtime made on a stack of files,
//! loaded, clocked, ticked, drawn, touched), as static native methods of
//! `nl.mattix.walkit.Native`; the host's answers go the other way, through the bridge objects
//! the Kotlin side registers when it makes a runtime (`FilesBridge`, `HostBridge`,
//! `PlayerBridge`), whose methods take and give only primitives, strings and arrays.
//!
//! A frame crosses the boundary once, as the packed buffer `walkit::packed` writes, decoded on
//! the Kotlin side into draw commands. A menu a script pops crosses the same way, and a menu
//! the host passes back is read with `walkit::packed::decode_menu`.
//!
//! A runtime is one thread's: the Kotlin side calls it from one thread at a time. The clock is the Kotlin side's, set before each call (`rtSetNow`), so
//! the engine reads it without crossing the boundary.
//!
//! This crate is the boundary: a native method's handle is a pointer the Kotlin side holds as a
//! `long`, and reading it back is what Rust marks unsafe. Nothing else here is, and the engine
//! itself forbids unsafe code. An error on the Kotlin side of a callback (an exception the
//! host threw) ends the native call that caused it with a `RuntimeException` naming it.

#![allow(unsafe_code)]
#![warn(missing_docs)]

mod bridge;
mod files;
mod hinted;
mod host;
mod native;
mod player;
