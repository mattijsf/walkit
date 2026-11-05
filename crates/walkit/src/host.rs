// SPDX-License-Identifier: GPL-3.0-or-later

//! What the engine asks of its host, and what it tells the host about its windows.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::draw::{FontDef, Rect};

/// Services the app supplies to the skin: its pictures, its text and its windows.
pub trait SkinHost {
    /// The size of a skin image, decoded by the host, or `None` when it cannot be read.
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)>;

    /// ARGB of one pixel of a skin image, or `None` when it cannot be read.
    fn pixel(&mut self, _file: &str, _x: i32, _y: i32) -> Option<u32> {
        None
    }

    /// ARGB of the `w` x `h` block of a skin image at `x`, `y`, row by row, or `None` when a
    /// pixel of it cannot be read; one [`SkinHost::pixel`] at a time unless the host reads a
    /// block at once.
    fn pixels(&mut self, file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
        let n = (w.max(0) as usize) * (h.max(0) as usize);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(self.pixel(
                file,
                x + (i % w as usize) as i32,
                y + (i / w as usize) as i32,
            )?);
        }
        Some(out)
    }

    /// How wide `text` draws in whole layout pixels, in `font` at `size`, `bold` or not.
    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32;

    /// How wide `text` draws, in fractional layout pixels, as this host's renderer draws it:
    /// the engine places text by it. The whole-pixel [`SkinHost::text_width`] by default.
    fn text_advance(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        self.text_width(text, font, size, bold) as f32
    }

    /// Ascent and descent in layout pixels as this host's renderer lays the font out, for a
    /// font whose file gives Windows' line no figures; `None` leaves it to a share of the em.
    fn line_metrics(
        &mut self,
        _font: Option<&FontDef>,
        _size: i32,
        _bold: bool,
    ) -> Option<(f32, f32)> {
        None
    }

    /// The bytes of a free font that stands in for a Windows system font (`fonts/LiberationSans.ttf`
    /// and the like), for the line Windows lays it out with; `None` leaves the line to the host's measure.
    fn font_file(&mut self, _file: &str) -> Option<Rc<[u8]>> {
        None
    }

    /// A line the skin logs: a missing file, an unknown tag, a script's message.
    fn log(&mut self, message: &str);

    /// The engine asks for a redraw.
    fn invalidate(&mut self);

    /// Whether one of the app's own windows is open: "pl" (playlist), "ml" (library), "eq",
    /// "video" or "vis". `None` leaves it to the skin's container. Buttons that toggle these light up.
    fn window_open(&mut self, _kind: &str) -> Option<bool> {
        None
    }

    /// Shows a skin's popup menu and returns the id of the command the listener picked, or 0
    /// for none. The script waits for it, so a host that shows menus blocks here until the
    /// answer. `None` means the menu closed without the listener (the app went away or the
    /// wait ran out): the script hears 0, and the touch shows no more menus.
    fn choose_from_menu(&mut self, _menu: &SkinMenu) -> Option<i32> {
        Some(0)
    }

    /// The windows on screen changed (main first): one opened, closed or switched layout (a
    /// shade). A host that shows each window apart makes or drops its windows here.
    fn windows_changed(&mut self, _windows: &[WindowId]) {}

    /// A button action the host answers, as Winamp answered it outside the skin, when the skin
    /// was set up with [`SkinSetup::host_actions`]: the action uppercase and its param or window name.
    fn host_action(&mut self, _action: &str, _param: Option<&str>) {}
}

/// A popup menu a script shows: its items, in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkinMenu {
    /// The items.
    pub items: Vec<MenuItem>,
}

/// One entry of a [`SkinMenu`].
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    /// A command the listener may pick.
    Command {
        /// What the script hears back.
        id: i32,
        /// The text shown.
        text: String,
        /// Shown with a check mark.
        checked: bool,
        /// Shown greyed out, not pickable.
        disabled: bool,
    },
    /// A line between items.
    Separator,
    /// A menu inside the menu.
    Submenu {
        /// The text shown.
        text: String,
        /// The menu it opens.
        menu: SkinMenu,
    },
}

/// A window of a loaded skin, as the host names it: one layout of one container. A container
/// that switches layout (to its shade, say) shows another window. An id stays the same for as
/// long as the skin is loaded, and no other window of that skin gets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u32);

/// An edge of the viewport on one axis: the left or top one, or the right or bottom one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The left or top edge.
    Start,
    /// The right or bottom edge.
    End,
}

/// What the host reads of a window at one moment: its size as it shows now, the minimum and
/// maximum the skin sets (0 for none), whether the skin declares resize handles across and up
/// and down, the edges of the viewport a script last moved it against, and the ids of its
/// container and layout in the skin's XML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// The window.
    pub id: WindowId,
    /// Its size now.
    pub size: (i32, i32),
    /// The minimum width, 0 for none.
    pub min_w: i32,
    /// The minimum height, 0 for none.
    pub min_h: i32,
    /// The maximum width, 0 for none.
    pub max_w: i32,
    /// The maximum height, 0 for none.
    pub max_h: i32,
    /// Whether the skin declares resize handles both ways.
    pub resizable_both_ways: bool,
    /// The side edge a script last moved it against.
    pub edge_x: Option<Edge>,
    /// The top or bottom edge a script last moved it against.
    pub edge_y: Option<Edge>,
    /// The container's id.
    pub container: String,
    /// The layout's id.
    pub layout: String,
}

/// What the host reads of one object of a skin: the window it is in, where it sits in that
/// window and in the stack, in layout pixels, whether it shows, and its class as scripts name
/// it (Button, Slider, Text, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectInfo {
    /// The window it is in.
    pub window: WindowId,
    /// Where it sits in its window.
    pub rect: Rect,
    /// Where it sits in the stack.
    pub stack_rect: Rect,
    /// Whether it shows.
    pub visible: bool,
    /// Its class as scripts name it.
    pub class_name: String,
}

/// What the engine has done so far, for a host's timing log or a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineStats {
    /// Script steps since the last tick began, across every script.
    pub script_steps: i64,
    /// Timer events fired.
    pub timer_fires: i64,
    /// Layout passes run.
    pub layout_passes: i64,
    /// Window shapes worked out.
    pub shapes_worked_out: i64,
}

/// What a skin keeps between runs: its settings by attribute name, as Winamp keeps them in
/// studio.xnf (`attributes`), and its scripts' private and public values (setPrivateString,
/// setPublicInt and the like) by lowercase "section/item" or "public/item" (`strings`, `ints`).
/// The color theme is kept apart ([`crate::SkinRuntime::color_theme`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavedSettings {
    /// The skin's settings by attribute name.
    pub attributes: BTreeMap<String, String>,
    /// The scripts' string values by lowercase key.
    pub strings: BTreeMap<String, String>,
    /// The scripts' integer values by lowercase key.
    pub ints: BTreeMap<String, i32>,
}

/// What the host tells a skin before it loads. `skin_name` is what getSkinName() answers: the
/// skin's folder name, which scripts key what they remember by. `have_library` says whether
/// Winamp's Media Library lives inside the skin (`@HAVE_LIBRARY@`). With `host_actions` the host
/// answers the button actions Winamp answered outside the skin ([`SkinHost::host_action`]) and
/// shows the library, the visualizer and video itself, so the skin's frames for them stay shut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinSetup {
    /// The skin's folder name.
    pub skin_name: String,
    /// Whether the Media Library lives inside the skin.
    pub have_library: bool,
    /// Whether the host answers the actions Winamp answered outside the skin.
    pub host_actions: bool,
}

impl Default for SkinSetup {
    fn default() -> Self {
        SkinSetup {
            skin_name: String::new(),
            have_library: true,
            host_actions: false,
        }
    }
}

/// The clock every timer, animation and script reads, in milliseconds. The app passes the wall
/// clock; a test steps one so animations and timers land on the same frame every run.
pub type Clock = Rc<dyn Fn() -> i64>;
