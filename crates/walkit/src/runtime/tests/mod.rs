// SPDX-License-Identifier: GPL-3.0-or-later

//! The runtime on small skins written in place: what the engine does that a trace cannot show.

mod classic;
mod free;
mod options;
mod playlist_menus;
mod presentation;
mod settings;
mod skin_name;
mod sliders;
mod stack;
mod text;
mod unknown_objects;
mod vis;

use std::cell::RefCell;
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

/// A host that keeps the actions the skin sends it, and where on screen each came from.
pub(super) struct RecordingHost(pub(super) Rc<RefCell<Vec<String>>>, pub(super) Froms);

/// Where each host action came from, in order.
pub(super) type Froms = Rc<RefCell<Vec<Option<crate::host::ActionFrom>>>>;

impl SkinHost for RecordingHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        text.chars().count() as i32 * size * if bold { 7 } else { 6 } / 10
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}

    fn host_action(
        &mut self,
        action: &str,
        param: Option<&str>,
        from: Option<crate::host::ActionFrom>,
    ) {
        self.0.borrow_mut().push(
            format!("{action} {}", param.unwrap_or(""))
                .trim_end()
                .to_string(),
        );
        self.1.borrow_mut().push(from);
    }
}

pub(super) const SKIN_XML: &str = r#"<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>"#;

/// A runtime on a skin whose `player.xml` is `player_xml`, on a clock stuck at 0, not loaded yet.
pub(super) fn runtime(player_xml: &str) -> SkinRuntime {
    runtime_with(player_xml, SkinSetup::default())
}

/// [`runtime`] set up as `setup` says.
pub(super) fn runtime_with(player_xml: &str, setup: SkinSetup) -> SkinRuntime {
    runtime_files(SKIN_XML, player_xml, setup)
}

/// A runtime on a skin whose `skin.xml` and `player.xml` are given, set up as `setup` says.
pub(super) fn runtime_files(skin_xml: &str, player_xml: &str, setup: SkinSetup) -> SkinRuntime {
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", skin_xml.as_bytes());
    files.insert("player.xml", player_xml.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    SkinRuntime::new(
        Box::new(files),
        Box::new(QuietHost),
        Box::new(FakePlayer::new(clock.clone())),
        setup,
        clock,
    )
}
