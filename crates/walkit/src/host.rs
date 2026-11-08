// SPDX-License-Identifier: GPL-3.0-or-later

//! What the engine asks of its host, and what it tells the host about its windows.

use std::collections::BTreeMap;
use std::rc::Rc;

use crate::draw::{FontDef, Rect};

/// Where a host action came from on screen, in the stack's layout pixels (as pointer events
/// take them): a menu it opens can go against the thing that sent it, or at the finger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionFrom {
    /// The thing that sent it: the button, a classic window's control, or the point a long press
    /// held as a 1 x 1 box.
    pub rect: Rect,
    /// Where the finger went down for it; `None` when no touch sent it (a script's timer).
    pub point: Option<(i32, i32)>,
}

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

    /// Whether one of the app's windows is open: "pl" (playlist), "ml" (library), "eq",
    /// "video" or "vis". `None` leaves it to the skin's container. Buttons that toggle these light up.
    /// A classic skin's clutter bar also asks for two of the app's switches: "ontop" (always on
    /// top, its A) and "doublesize" (its D); `None` reads as off.
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
    /// was set up with [`SkinSetup::host_actions`]: the action uppercase, its param or window
    /// name, and where on screen it came from ([`ActionFrom`]), for a menu it opens; `None` when
    /// nothing on screen sent it.
    fn host_action(&mut self, _action: &str, _param: Option<&str>, _from: Option<ActionFrom>) {}
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

/// How the host shows a skin's windows ([`crate::SkinRuntime::presentation`]), in layout pixels:
/// `view_size` is the space the host has for the stack (`None` leaves each window at the size
/// it has); `viewport` the screen scripts may place windows on before there is a view size;
/// `columns` 2 sets the windows that show a list out right of the main window, anything else
/// docks every window under it; `column_width` how wide that right column is, 0 for as wide as
/// what shows of the main window; `stretch_last` whether the last docked list window takes the
/// height left under the others; `touch_slop` how far beside an object a finger may land and
/// still mean it; `touch` whether a finger drives the skin rather than a mouse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presentation {
    /// The space the host has for the stack.
    pub view_size: Option<(i32, i32)>,
    /// The screen scripts see before there is a view size.
    pub viewport: (i32, i32),
    /// How many columns the stack is set out in: 2, or one.
    pub columns: i32,
    /// How wide the right column is; 0 for as wide as what shows of the main window.
    pub column_width: i32,
    /// Whether the last docked list window takes the height left.
    pub stretch_last: bool,
    /// How far beside an object a finger may land.
    pub touch_slop: i32,
    /// Whether the windows are set out apart, each where it is, moved and docked as on Winamp's
    /// desktop (see `SkinRuntime::place_window`); false for one stack.
    pub free_windows: bool,
    /// Whether a finger drives the skin rather than a mouse: a tap on a classic playlist's bottom
    /// button leaves its menu open for a second tap, where a mouse holds the button and lets go
    /// over a picture.
    pub touch: bool,
    /// Whether a classic playlist spells its rows in the skin's `text.bmp` letters, in the colors
    /// `pledit.txt` gives them, rather than in the font `pledit.txt` names; a row the letters
    /// cannot spell keeps that font.
    pub skin_list_font: bool,
    /// Pixels a classic playlist adds to each row's height, for rows a finger can pick more
    /// easily; 0 keeps the font's row (Winamp's 15).
    pub list_row_extra: i32,
}

impl Default for Presentation {
    fn default() -> Self {
        Presentation {
            view_size: None,
            viewport: (1080, 1920),
            columns: 1,
            column_width: 0,
            stretch_last: true,
            touch_slop: 6,
            free_windows: false,
            touch: false,
            skin_list_font: false,
            list_row_extra: 0,
        }
    }
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

/// The options a skin's scripts list for Winamp's Options menu, as of `version`
/// ([`crate::SkinRuntime::config_version`] when the list was read). Two are equal when they
/// list the same entries, whatever their versions.
#[derive(Debug, Clone, Default)]
pub struct SkinOptions {
    /// The config version the list was read at.
    pub version: i32,
    /// The lines of the menu, in order.
    pub entries: Vec<OptionEntry>,
}

impl PartialEq for SkinOptions {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for SkinOptions {}

/// One line of a skin's options menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionEntry {
    /// A checkable option; `checked` as Winamp ticks it: the value read as a number is not 0.
    /// `radio` when the skin's page shows it as one of a set of radio buttons, which
    /// Winamp's menu does not tell apart.
    Toggle {
        /// The option, for [`crate::SkinRuntime::toggle_skin_option`].
        key: OptionKey,
        /// The text shown.
        text: String,
        /// Whether it is ticked.
        checked: bool,
        /// Whether the skin's page draws it as a radio button.
        radio: bool,
    },
    /// A line between groups of options.
    Separator,
    /// An attribute whose value is another item's GUID; `text` is the attribute's name.
    Submenu {
        /// The text shown.
        text: String,
        /// The lines inside.
        entries: Vec<OptionEntry>,
    },
}

/// An option by item GUID and lowercased attribute name; stays valid across snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptionKey {
    /// The Config item's GUID, with braces, as the skin wrote it.
    pub item: String,
    /// The attribute's name, lowercase.
    pub attribute: String,
}

/// What the host tells a skin before it loads. `skin_name` is what `getSkinName()` answers: the
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
