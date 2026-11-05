// SPDX-License-Identifier: GPL-3.0-or-later

//! MAKI, the compiled script language of a modern skin: values, programs, the parser, the
//! standard library's table, and what a loaded script keeps. The runtime runs the bytecode.

pub mod parser;
pub mod program;
pub mod script;
pub mod stdapi;
pub mod value;

pub use program::Program;
pub use script::{Script, SystemObject};
pub use value::{Cell, Handle, Kind, Value};
