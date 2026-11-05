// SPDX-License-Identifier: GPL-3.0-or-later

//! A Winamp skin engine. It reads a modern skin (a `.wal` file: XML, images and MAKI
//! scripts) or a classic skin (a `.wsz` file, see [`classic`]), runs the scripts, lays the
//! windows out and hands the host a draw list for each frame. The host owns the screen, the
//! images, the fonts, the touch input and the player; the engine never draws, reads a file or
//! plays a sound.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod draw;
pub mod files;
pub mod gdi;
pub mod generated;
pub mod gui;
pub mod host;
pub mod listing;
pub mod maki;
pub mod packed;
pub mod player;
pub mod runtime;
pub mod text;
pub mod xml;

pub use runtime::SkinRuntime;

/// The version of the trace text the engine writes in a scripted run (`trace v1`).
pub const TRACE_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    #[test]
    fn the_trace_format_is_the_first() {
        assert_eq!(super::TRACE_VERSION, 1);
    }
}
