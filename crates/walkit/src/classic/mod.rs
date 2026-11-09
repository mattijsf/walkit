// SPDX-License-Identifier: GPL-3.0-or-later
//! Classic skins (`.wsz`: `main.bmp`, `cbuttons.bmp`, `titlebar.bmp`, `text.bmp`, `numbers.bmp`
//! and the rest), drawn as Winamp 5.666 draws them: each window is a generated container with
//! one classic window object whose drawing and touches are the ones Winamp does, pixel for pixel
//! (measured against Winamp 5.666 in a VM). The windows themselves, their docking,
//! raising and the touch routing are the engine's, as for a modern skin.

use std::rc::Rc;

use crate::files::SkinFiles;

pub mod cursor;
pub(crate) mod eq;
pub mod font;
pub(crate) mod gen;
pub mod genex;
pub(crate) mod listfont;
pub(crate) mod main;
mod pl;
pub mod pledit;
pub mod region;
pub mod touch;
pub(crate) mod video;
pub mod vis;
pub mod viscolor;

/// The classic windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassicWindow {
    /// The main window, 275 x 116.
    Main,
    /// The main window's shade bar, 275 x 14.
    MainShade,
    /// The equalizer, 275 x 116.
    Equalizer,
    /// The equalizer's shade bar, 275 x 14.
    EqualizerShade,
    /// The playlist editor, 275 x 116 at least.
    Playlist,
    /// The playlist editor's shade bar, as wide as the editor, 14 high.
    PlaylistShade,
    /// A window framed with `gen.bmp` (the media library, a plug-in's window), 275 x 116 at
    /// least; what it holds is the host's.
    Gen,
    /// The video window, framed with `video.bmp`, 275 x 116 at least; what it shows is the host's.
    Video,
}

impl ClassicWindow {
    /// The window `name` names in the generated XML.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "main" => Some(ClassicWindow::Main),
            "mainshade" => Some(ClassicWindow::MainShade),
            "equalizer" => Some(ClassicWindow::Equalizer),
            "equalizershade" => Some(ClassicWindow::EqualizerShade),
            "playlist" => Some(ClassicWindow::Playlist),
            "playlistshade" => Some(ClassicWindow::PlaylistShade),
            "gen" => Some(ClassicWindow::Gen),
            "video" => Some(ClassicWindow::Video),
            _ => None,
        }
    }

    /// The window's size in skin pixels.
    pub fn size(self) -> (i32, i32) {
        match self {
            ClassicWindow::Main
            | ClassicWindow::Equalizer
            | ClassicWindow::Playlist
            | ClassicWindow::Gen
            | ClassicWindow::Video => (275, 116),
            ClassicWindow::MainShade
            | ClassicWindow::EqualizerShade
            | ClassicWindow::PlaylistShade => (275, 14),
        }
    }

    /// The `region.txt` section that shapes the window.
    pub fn region_section(self) -> &'static str {
        match self {
            ClassicWindow::Main => "normal",
            ClassicWindow::MainShade => "windowshade",
            ClassicWindow::Equalizer => "equalizer",
            ClassicWindow::EqualizerShade => "equalizerws",
            ClassicWindow::Playlist
            | ClassicWindow::PlaylistShade
            | ClassicWindow::Gen
            | ClassicWindow::Video => "",
        }
    }
}

/// The file the generated windows are written to.
const CLASSIC_XML: &str = "classic.xml";

/// Whether `files` hold a classic skin: a `main.bmp` and no `skin.xml`.
pub fn is_classic(files: &dyn SkinFiles) -> bool {
    !files.exists("skin.xml") && files.exists("main.bmp")
}

/// The containers of a classic skin, as the engine reads a modern skin's: the main window,
/// the equalizer under it and the playlist under that, each one classic window object that
/// shapes its window (`sysregion`).
fn generated_xml() -> String {
    let (w, h) = ClassicWindow::Main.size();
    format!(
        concat!(
            r#"<WinampAbstractionLayer version="0.8">"#,
            r#"<container id="main" name="Main Window" default_visible="1" default_x="0" default_y="0">"#,
            r#"<layout id="normal" w="{w}" h="{h}">"#,
            r#"<classicwindow id="classic.main" window="main" x="0" y="0" w="{w}" h="{h}" sysregion="1"/>"#,
            r#"</layout>"#,
            r#"<layout id="shade" w="{w}" h="14">"#,
            r#"<classicwindow id="classic.mainshade" window="mainshade" x="0" y="0" w="{w}" h="14" sysregion="1"/>"#,
            r#"</layout></container>"#,
            r#"<container id="equalizer" name="Equalizer" default_visible="0" default_x="0" default_y="{h}">"#,
            r#"<layout id="normal" w="{w}" h="{h}">"#,
            r#"<classicwindow id="classic.eq" window="equalizer" x="0" y="0" w="{w}" h="{h}" sysregion="1"/>"#,
            r#"</layout>"#,
            r#"<layout id="shade" w="{w}" h="14">"#,
            r#"<classicwindow id="classic.eqshade" window="equalizershade" x="0" y="0" w="{w}" h="14" sysregion="1"/>"#,
            r#"</layout></container>"#,
            r#"<container id="pledit" name="Playlist Editor" default_visible="0" default_x="0" default_y="{h2}">"#,
            r#"<layout id="normal" w="{w}" h="{h}" minimum_w="{w}" minimum_h="{h}" linkwidth="shade">"#,
            r#"<classicwindow id="classic.pl" window="playlist" x="0" y="0" w="0" h="0" relatw="1" relath="1" sysregion="1"/>"#,
            r#"<layer id="classic.pl.sizer" resize="bottomright" x="-20" relatx="1" y="-20" relaty="1" w="20" h="20"/>"#,
            r#"</layout>"#,
            r#"<layout id="shade" w="{w}" h="14" minimum_w="{w}" linkwidth="normal">"#,
            r#"<classicwindow id="classic.plshade" window="playlistshade" x="0" y="0" w="0" h="14" relatw="1" sysregion="1"/>"#,
            r#"<layer id="classic.plshade.sizer" resize="right" x="-28" relatx="1" y="0" w="8" h="14"/>"#,
            r#"</layout></container>"#,
            r#"<container id="library" name="Winamp Library" default_visible="0" default_x="{w}" default_y="0">"#,
            r#"<layout id="normal" w="{w}" h="{h}" minimum_w="{w}" minimum_h="{h}">"#,
            r#"<classicwindow id="classic.library" window="gen" x="0" y="0" w="0" h="0" relatw="1" relath="1"/>"#,
            r#"<layer id="classic.gen.sizer" resize="bottomright" x="-20" relatx="1" y="-20" relaty="1" w="20" h="20"/>"#,
            r#"</layout></container>"#,
            r#"<container id="video" name="Winamp Video" default_visible="0" default_x="0" default_y="{h}">"#,
            r#"<layout id="normal" w="{w}" h="{h2}" minimum_w="{w}" minimum_h="{h}">"#,
            r#"<classicwindow id="classic.video" window="video" x="0" y="0" w="0" h="0" relatw="1" relath="1"/>"#,
            r#"<layer id="classic.video.sizer" resize="bottomright" x="-20" relatx="1" y="-20" relaty="1" w="20" h="20"/>"#,
            r#"</layout></container>"#,
            r#"</WinampAbstractionLayer>"#
        ),
        w = w,
        h = h,
        h2 = h * 2
    )
}

/// A classic skin's files with the generated `skin.xml` in front of them.
pub struct ClassicFiles {
    inner: Box<dyn SkinFiles>,
    xml: Rc<[u8]>,
}

impl ClassicFiles {
    /// Wraps `inner`, a classic skin's files.
    pub fn new(inner: Box<dyn SkinFiles>) -> Self {
        ClassicFiles {
            inner,
            xml: Rc::from(generated_xml().into_bytes().into_boxed_slice()),
        }
    }
}

impl SkinFiles for ClassicFiles {
    fn read(&self, path: &str) -> Option<Rc<[u8]>> {
        let p = crate::files::normalize(path);
        if p == "skin.xml" || p == CLASSIC_XML {
            return Some(self.xml.clone());
        }
        self.inner.read(path)
    }

    fn names(&self) -> Vec<String> {
        let mut names = self.inner.names();
        names.push("skin.xml".to_string());
        names
    }
}
