// SPDX-License-Identifier: GPL-3.0-or-later

//! The runtime on small skins written in place, checked against what the reference engine does
//! on the same skins: the tests of Andamp's `:modernskin` module that the trace cannot show.

mod options;
mod presentation;
mod settings;
mod stack;

use std::rc::Rc;

use super::SkinRuntime;
use crate::draw::FontDef;
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinHost, SkinSetup};
use crate::player::FakePlayer;

/// A host without pictures that measures text by the reference fixture's rule and drops the log.
struct QuietHost;

impl SkinHost for QuietHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        text.chars().count() as i32 * size * if bold { 7 } else { 6 } / 10
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}
}

const SKIN_XML: &str = r#"<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>"#;

/// A runtime on a skin whose `player.xml` is `player_xml`, on a clock stuck at 0, not loaded yet.
pub(super) fn runtime(player_xml: &str) -> SkinRuntime {
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", SKIN_XML.as_bytes());
    files.insert("player.xml", player_xml.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    )
}
