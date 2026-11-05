// SPDX-License-Identifier: GPL-3.0-or-later

//! Records a scenario of the reference set through the engine on a stepped clock and writes
//! what the engine did as a trace (`trace v1`), for a line-by-line diff against the traces of
//! another engine. The recorder and its fixed host run on any platform: a desktop build
//! (`walkit-trace`) and the web build both use them. See `trace` for the format.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "native")]
pub mod archive;
pub mod host;
pub mod scenario;
pub mod trace;

pub use host::{Decoder, Picture, TraceHost};
pub use scenario::Scenario;
pub use trace::{record, Recording, Stats};
