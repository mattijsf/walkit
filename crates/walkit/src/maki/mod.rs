// SPDX-License-Identifier: GPL-3.0-or-later

//! MAKI, the compiled script language of a modern skin: values, programs, the parser and the VM.

pub mod script;
pub mod value;

pub use script::{Script, SystemObject};
pub use value::{Cell, Handle, Kind, Value};
