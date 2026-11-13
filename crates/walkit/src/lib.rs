// SPDX-License-Identifier: GPL-3.0-or-later

//! A Winamp skin engine. It reads a modern skin (a `.wal` file: XML, images and MAKI
//! scripts) or a classic skin (a `.wsz` file, see [`classic`]), runs the scripts, lays the
//! windows out and hands the host a draw list for each frame. The host owns the screen, the
//! images, the fonts, the touch input and the player; the engine never draws, reads a file or
//! plays a sound.
//!
//! ```sh
//! cargo add walkit
//! ```
//!
//! # Where to start
//!
//! - [`SkinRuntime`] runs one skin: load it, tick it each frame, take its draw list.
//! - [`host::SkinHost`] is what the host implements for pictures, text measuring and logging.
//! - [`player::Player`] is the transport the skin's buttons and sliders drive;
//!   [`player::FakePlayer`] plays nothing and is enough for a first look.
//! - [`files::MemoryFiles`] holds the entries of a skin archive, and [`files::LayeredFiles`]
//!   puts the skin over a base (Winamp's wasabi folder or a classic base skin).
//! - [`draw::DrawCmd`] is one command of the draw list: images, fills, text, clips, shapes.
//! - [`info`] reads what a skin says about itself (author, version, containers, readme) for a
//!   host that shows a skin's details without loading it.
//!
//! The [guide](https://mattijsf.github.io/walkit/) explains how it works, and its
//! [Rust page](https://mattijsf.github.io/walkit/rust/) builds a first host step by step.
//!
//! # Features
//!
//! - `truetype`: TrueType text drawn as Windows draws it, for hosts that draw their own
//!   letters into a buffer of pixels (adds the `skrifa` crate). It adds the `truetype` module.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub mod classic;
pub mod draw;
mod fasthash;
pub mod files;
pub mod gdi;
pub mod generated;
pub mod gui;
pub mod host;
pub mod info;
pub mod listing;
pub mod maki;
pub mod packed;
pub mod plan;
pub mod player;
pub mod runtime;
pub mod text;
#[cfg(feature = "truetype")]
pub mod truetype;
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
