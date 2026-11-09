// SPDX-License-Identifier: GPL-3.0-or-later
//! Touches on the classic windows, as Winamp's classic windows handle the mouse: each control
//! answers in a box of the main window, the equalizer and the playlist, most act on
//! release over the control and a few on the press, the sliders follow the finger from where
//! they were grabbed, the control buttons hand the press along as the finger slides over them, a
//! drag on any place without a control moves the window, and double taps do what Winamp's double
//! clicks do.

use crate::classic::ClassicWindow;
use crate::draw::Rect;
use crate::gui::{Kind, ObjId};
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// A control of a classic window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// The bar a drag moves the window by.
    TitleBar,
    /// One of the video window's bottom buttons, 0 the leftmost; a press slides along them.
    VideoButton(i32),
    /// A place without a control: a drag there moves the window too (Winamp's "easy move").
    Background,
    /// The song title's row: a drag scrolls it by hand.
    TitleText,
    /// The lightning logo at the main window's lower right, which opens the about box.
    Logo,
    /// The shade bar's small vis.
    ShadeVis,
    /// The playlist's small transport buttons by its time: 0 previous .. 4 next, 5 eject.
    PlMini(i32),
    /// The playlist's time display.
    PlTime,
    /// The playlist's arrow that scrolls the list a row up.
    PlScrollUp,
    /// The playlist's arrow that scrolls the list a row down.
    PlScrollDown,
    /// The menu button at the title bar's left.
    Menu,
    /// The minimize button.
    Minimize,
    /// The shade button.
    Shade,
    /// The close button.
    Close,
    /// Previous track.
    Previous,
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Stop.
    Stop,
    /// Next track.
    Next,
    /// Eject: the open-file button.
    Eject,
    /// The shuffle toggle.
    Shuffle,
    /// The repeat toggle.
    Repeat,
    /// The equalizer window toggle.
    EqToggle,
    /// The playlist window toggle.
    PlToggle,
    /// The position bar.
    Position,
    /// The volume slider.
    Volume,
    /// The balance slider.
    Balance,
    /// The clutter bar's letters: O for the options menu, A for always on top, I for the file
    /// info, D for double size, V for the visualization menu.
    ClutterO,
    /// The clutter bar's A.
    ClutterA,
    /// The clutter bar's I.
    ClutterI,
    /// The clutter bar's D.
    ClutterD,
    /// The clutter bar's V.
    ClutterV,
    /// The time display: a press switches elapsed and remaining.
    Time,
    /// The visualizer area.
    Vis,
    /// The equalizer's ON button.
    EqOn,
    /// The equalizer's AUTO button.
    EqAuto,
    /// The equalizer's PRESETS button.
    EqPresets,
    /// A band slider: 0 the preamp, 1..=10 the bands.
    EqSlider(usize),
    /// A drag across the bands between two of them: a band left sideways lets go, and the
    /// next one the finger reaches takes the drag. Only a press holds it.
    EqSweep,
    /// One of the "+12", "0" and "-12" labels between the preamp and the bands, with the value
    /// a release on it gives all ten bands (0..255 as the player keeps them).
    EqLevel(i32),
    /// The shade bar's volume slider.
    ShadeVolume,
    /// The shade bar's balance slider.
    ShadeBalance,
    /// The shade bar's position slider.
    ShadePosition,
    /// A playlist row, by index from the top of the list.
    PlRow(i32),
    /// The playlist's scrollbar.
    PlScroll,
    /// One of the playlist's bottom buttons, whose press pops its menu of pictures.
    PlButton(PlMenu),
}

/// The playlist's bottom buttons, each with a menu of stacked pictures from `pledit.bmp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlMenu {
    /// ADD: a URL, a folder, files.
    Add,
    /// REM, top to bottom: miscellaneous, all, crop, the selected songs (Winamp's base skin
    /// letters them misc, all, crop, sel).
    Remove,
    /// SEL: invert, none, all.
    Select,
    /// MISC: sort, file info, options.
    Misc,
    /// LIST: new, save, load.
    List,
}

impl PlMenu {
    /// How many pictures its menu stacks.
    pub(crate) fn items(self) -> i32 {
        if self == PlMenu::Remove {
            4
        } else {
            3
        }
    }

    /// The menu's left edge for a playlist `width` wide: the bar's column, the pictures 3 on.
    pub(crate) fn menu_left(self, width: i32) -> i32 {
        match self {
            PlMenu::Add => 11,
            PlMenu::Remove => 40,
            PlMenu::Select => 69,
            PlMenu::Misc => 98,
            PlMenu::List => width - 47,
        }
    }

    /// The `pledit.bmp` column of the bar beside the menu.
    pub(crate) fn bar_source(self) -> i32 {
        match self {
            PlMenu::Add => 48,
            PlMenu::Remove => 100,
            PlMenu::Select => 150,
            PlMenu::Misc => 200,
            PlMenu::List => 250,
        }
    }

    /// The `pledit.bmp` column of its pictures at rest; the pressed ones sit 23 to the right.
    pub(crate) fn picture_source(self) -> i32 {
        match self {
            PlMenu::Add => 0,
            PlMenu::Remove => 54,
            PlMenu::Select => 104,
            PlMenu::Misc => 154,
            PlMenu::List => 204,
        }
    }

    /// What the host hears for item `index` (0 the lowest picture, as Winamp counts them).
    fn choice(self, index: i32) -> Option<&'static str> {
        Some(match (self, index) {
            (PlMenu::Add, 2) => "add_url",
            (PlMenu::Add, 1) => "add_dir",
            (PlMenu::Add, 0) => "add_file",
            (PlMenu::Remove, 3) => "remove_misc",
            (PlMenu::Remove, 2) => "remove_all",
            (PlMenu::Remove, 1) => "crop",
            (PlMenu::Remove, 0) => "remove_selected",
            (PlMenu::Select, 2) => "select_invert",
            (PlMenu::Select, 1) => "select_none",
            (PlMenu::Select, 0) => "select_all",
            (PlMenu::Misc, 2) => "sort",
            (PlMenu::Misc, 1) => "file_info",
            (PlMenu::Misc, 0) => "misc_options",
            (PlMenu::List, 2) => "list_new",
            (PlMenu::List, 1) => "list_save",
            (PlMenu::List, 0) => "list_load",
            _ => return None,
        })
    }
}

/// The playlist picks after which no row stays selected: they clear the selection or change the
/// list it pointed into.
const CLEARS_SELECTION: [&str; 6] = [
    "select_none",
    "remove_selected",
    "remove_all",
    "crop",
    "list_new",
    "list_load",
];

/// The menu picture under `x`, `y` of a playlist `width` by `height`, for button `b`: the
/// pictures are 22 wide and 18 tall, the lowest at `height - 30`, each next one 18 above.
fn pl_menu_item_at(b: PlMenu, width: i32, height: i32, x: i32, y: i32) -> Option<i32> {
    let left = b.menu_left(width) + 3;
    if x < left || x >= left + 22 {
        return None;
    }
    let from_bottom = height - 12 - y;
    if from_bottom < 0 {
        return None;
    }
    let index = from_bottom / 18;
    (index < b.items()).then_some(index)
}

/// A press on a classic window that has not ended.
#[derive(Debug, Clone, Copy)]
pub struct ClassicPress {
    /// The object pressed.
    pub object: ObjId,
    /// What was pressed.
    pub control: Control,
    /// Where the finger is now, in the window.
    pub at: (i32, i32),
    /// Whether the finger is over the control now.
    pub inside: bool,
    /// The position bar's value while it is dragged, 0..255.
    pub position: i32,
    /// The playlist menu picture the finger is over, 0 the lowest, while a bottom button is held.
    pub hover: Option<i32>,
    /// Where the press began, in the window.
    pub from: (i32, i32),
    /// The playlist's scroll when the press began.
    pub scroll_from: i32,
    /// Where on a slider's thumb the finger took it, or the title row's last x.
    pub grab: i32,
    /// Whether the press already did what it does (some controls act on the press).
    pub acted: bool,
    /// Whether a press on the playlist's rows has moved far enough to scroll them.
    pub dragged: bool,
    /// Whether the press landed on a picture of a menu a tap left open, under touch.
    pub on_open_menu: bool,
    /// The point of the control the press took: where it began, or, under touch, the point of
    /// the control nearest to a finger that landed beside it.
    pub aim: (i32, i32),
}

/// How far a finger moves on the playlist's rows before the press scrolls them instead of
/// picking a row.
const DRAG_SLOP: i32 = 8;

/// Whether `x`, `y` lies in the box.
fn hit(x: i32, y: i32, bx: i32, by: i32, bw: i32, bh: i32) -> bool {
    x >= bx && x < bx + bw && y >= by && y < by + bh
}

/// The box in window `w` (sized `width` x `height`, its playlist rows `row` tall) of the control
/// at `x`, `y`: as far each way as the same control answers, so a menu the control opens can go
/// against it. `None` where no control is.
pub fn control_box(
    w: ClassicWindow,
    width: i32,
    height: i32,
    row: i32,
    (x, y): (i32, i32),
) -> Option<Rect> {
    let c = control_at(w, width, height, row, x, y)?;
    let same = |px: i32, py: i32| control_at(w, width, height, row, px, py) == Some(c);
    let (mut left, mut right, mut top, mut bottom) = (x, x, y, y);
    while left > 0 && same(left - 1, y) {
        left -= 1;
    }
    while right + 1 < width && same(right + 1, y) {
        right += 1;
    }
    while top > 0 && same(x, top - 1) {
        top -= 1;
    }
    while bottom + 1 < height && same(x, bottom + 1) {
        bottom += 1;
    }
    Some(Rect::new(left, top, right - left + 1, bottom - top + 1))
}

/// The control of window `w` (sized `width` x `height`, its playlist rows `row` tall) at `x`, `y`.
#[allow(clippy::enum_glob_use)]
pub fn control_at(
    w: ClassicWindow,
    width: i32,
    height: i32,
    row: i32,
    x: i32,
    y: i32,
) -> Option<Control> {
    use Control::*;
    match w {
        ClassicWindow::Main => {
            // the box each control answers in (probed in the VM): title buttons, clutter bar,
            // time and vis, sliders, position bar, control buttons, eject, shuffle and repeat,
            // the window toggles, the logo, the title row
            let boxes: [(Control, i32, i32, i32, i32); 26] = [
                (Menu, 5, 3, 13, 11),
                (Minimize, 244, 3, 10, 10),
                (Shade, 254, 3, 9, 10),
                (Close, 264, 3, 9, 10),
                (ClutterO, 11, 24, 9, 8),
                (ClutterA, 11, 32, 9, 8),
                (ClutterI, 11, 40, 9, 8),
                (ClutterD, 11, 48, 9, 8),
                (ClutterV, 11, 56, 9, 7),
                (Time, 36, 26, 61, 14),
                (Vis, 27, 40, 73, 22),
                (TitleText, 117, 24, 150, 12),
                (Volume, 107, 58, 69, 9),
                (Balance, 177, 58, 39, 9),
                (EqToggle, 219, 58, 24, 13),
                (PlToggle, 243, 58, 23, 13),
                (Position, 18, 73, 246, 9),
                (Previous, 15, 87, 23, 20),
                (Play, 38, 87, 22, 20),
                (Pause, 60, 87, 24, 20),
                (Stop, 84, 87, 23, 20),
                (Next, 107, 87, 24, 20),
                (Eject, 136, 89, 23, 17),
                (Shuffle, 165, 90, 45, 13),
                (Repeat, 210, 90, 30, 13),
                (Logo, 253, 91, 13, 15),
            ];
            for (c, bx, by, bw, bh) in boxes {
                if hit(x, y, bx, by, bw, bh) {
                    return Some(c);
                }
            }
            Some(if y < 14 { TitleBar } else { Background })
        }
        ClassicWindow::MainShade => {
            // the bar's transport buttons split x 168..=224 at 176, 186, 195, 204 and 215; its
            // mini position bar is x 228..=242, its time x 129..=157, its small vis x 78..=116
            let boxes: [(Control, i32, i32, i32, i32); 13] = [
                (Menu, 5, 3, 13, 11),
                (Minimize, 244, 3, 10, 10),
                (Shade, 254, 3, 9, 10),
                (Close, 264, 3, 9, 10),
                (Previous, 168, 2, 8, 10),
                (Play, 176, 2, 10, 10),
                (Pause, 186, 2, 9, 10),
                (Stop, 195, 2, 9, 10),
                (Next, 204, 2, 11, 10),
                (Eject, 215, 2, 10, 10),
                (ShadePosition, 228, 3, 15, 10),
                (Time, 129, 3, 29, 7),
                (ShadeVis, 78, 4, 39, 8),
            ];
            for (c, bx, by, bw, bh) in boxes {
                if hit(x, y, bx, by, bw, bh) {
                    return Some(c);
                }
            }
            Some(TitleBar)
        }
        ClassicWindow::Equalizer => {
            // the title buttons answer a column beside their pictures and a row below (probed
            // in the VM: shade x 253..=263, close x 264..=273, both y 3..=12)
            if hit(x, y, 253, 3, 11, 10) {
                return Some(Shade);
            }
            if hit(x, y, 264, 3, 10, 10) {
                return Some(Close);
            }
            // ON and AUTO answer a row more than their pictures; a slider's box stops a column
            // short of its picture's right edge (probed in the VM)
            if hit(x, y, 14, 18, 25, 13) {
                return Some(EqOn);
            }
            if hit(x, y, 39, 18, 34, 13) {
                return Some(EqAuto);
            }
            if hit(x, y, 217, 18, 45, 13) {
                return Some(EqPresets);
            }
            // the labels between the preamp and the bands: x 42..=67, "+12" on
            // rows 33..=42, "0" on 65..=74, "-12" on 92..=101
            for (top, level) in [(33, 255), (65, crate::runtime::EQ_FLAT), (92, 0)] {
                if hit(x, y, 42, top, 26, 10) {
                    return Some(EqLevel(level));
                }
            }
            // a slider answers rows 39..=98
            if hit(x, y, 21, 39, 13, 60) {
                return Some(EqSlider(0));
            }
            for n in 1..=10 {
                if hit(x, y, n * 18 + 60, 39, 13, 60) {
                    return Some(EqSlider(n as usize));
                }
            }
            Some(if y < 14 { TitleBar } else { Background })
        }
        ClassicWindow::EqualizerShade => {
            if hit(x, y, 253, 3, 11, 10) {
                return Some(Shade);
            }
            if hit(x, y, 264, 3, 10, 10) {
                return Some(Close);
            }
            // the bar's sliders answer two rows more than their pictures and a column or two
            // beyond them (probed in the VM: volume x 61..=162, balance x 163..=206, y 3..=11)
            if hit(x, y, 61, 3, 102, 9) {
                return Some(ShadeVolume);
            }
            if hit(x, y, 163, 3, 44, 9) {
                return Some(ShadeBalance);
            }
            Some(TitleBar)
        }
        ClassicWindow::Playlist => {
            // probed in the VM at width 275: shade x 255..=264, close x 265..=274, y 3..=12
            if hit(x, y, width - 10, 3, 10, 10) {
                return Some(Close);
            }
            if hit(x, y, width - 20, 3, 10, 10) {
                return Some(Shade);
            }
            // the small arrows above the corner and the scrollbar's track,
            // a column beyond its picture (x 260..=268 at width 275)
            if hit(x, y, width - 15, height - 36, 9, 5) {
                return Some(PlScrollUp);
            }
            if hit(x, y, width - 15, height - 31, 9, 5) {
                return Some(PlScrollDown);
            }
            if hit(x, y, width - 15, 20, 9, height - 58) {
                return Some(PlScroll);
            }
            // the rows: x 12..=w-20, y 20..=h-38, a row counted from y 22
            if hit(x, y, 12, 20, width - 31, height - 57) {
                return Some(PlRow((y - crate::classic::pl::ROW_TOP) / row.max(1)));
            }
            // the small transport by the time, x w-144..=w-91 split at 7, 17, 27, 36 and 45
            // and the time beside it
            if hit(x, y, width - 144, height - 15, 54, 8) {
                let dx = x - (width - 144);
                let index = [7, 17, 27, 36, 45].iter().filter(|e| dx >= **e).count();
                return Some(PlMini(index as i32));
            }
            if hit(x, y, width - 87, height - 18, 35, 10) {
                return Some(PlTime);
            }
            // the bottom buttons: ADD x 14..=35, REM 43..=64, SEL
            // 72..=93, MISC 101..=122, LIST w-44..=w-22, rows h-30..=h-13
            for (b, left, wide) in [
                (PlMenu::Add, 14, 22),
                (PlMenu::Remove, 43, 22),
                (PlMenu::Select, 72, 22),
                (PlMenu::Misc, 101, 22),
                (PlMenu::List, width - 44, 23),
            ] {
                if hit(x, y, left, height - 30, wide, 18) {
                    return Some(PlButton(b));
                }
            }
            Some(if y < 14 { TitleBar } else { Background })
        }
        ClassicWindow::PlaylistShade => {
            if hit(x, y, width - 10, 3, 10, 10) {
                return Some(Close);
            }
            if hit(x, y, width - 20, 3, 10, 10) {
                return Some(Shade);
            }
            Some(TitleBar)
        }
        ClassicWindow::Video => {
            use crate::classic::video as v;
            // close: x w-10..=w-1, y 3..=12
            if hit(x, y, width - 10, 3, 10, 10) {
                return Some(Close);
            }
            if hit(x, y, v::BUTTONS_X, height - 29, 81, 19) {
                return Some(VideoButton((x - v::BUTTONS_X) / v::BUTTON_W));
            }
            if y < v::TITLE_H {
                return Some(TitleBar);
            }
            let client = x >= v::LEFT && x < width - v::RIGHT && y < height - v::BOTTOM;
            (!client).then_some(Background)
        }
        ClassicWindow::Gen => {
            // close: x w-10..=w-1, y 3..=12, acting on release inside
            if hit(x, y, width - 10, 3, 10, 10) {
                return Some(Close);
            }
            if y < crate::classic::gen::TITLE_H {
                return Some(TitleBar);
            }
            // what the window holds is the host's; the frame around it moves the window
            let client = x >= crate::classic::gen::LEFT
                && x < width - crate::classic::gen::RIGHT
                && y < height - crate::classic::gen::BOTTOM;
            (!client).then_some(Background)
        }
    }
}

/// Under touch, the control a finger at `x`, `y` of window `w` (sized `width` x `height`, its
/// playlist rows `row` tall) means when it lands beside every control: the nearest that `takes`
/// accepts within `reach`, with its point nearest the finger and the square of the distance. A
/// slider is reached only across its track, never past its ends, so the finger takes it at the
/// value under the finger.
pub(crate) fn nearest_control(
    w: ClassicWindow,
    (width, height, row): (i32, i32, i32),
    (x, y): (i32, i32),
    reach: i32,
    takes: impl Fn(Control) -> bool,
) -> Option<(Control, (i32, i32), i32)> {
    let mut best: Option<(Control, (i32, i32), i32)> = None;
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let d2 = dx * dx + dy * dy;
            if d2 > reach * reach || best.is_some_and(|(_, _, b)| d2 >= b) {
                continue;
            }
            let (cx, cy) = (x + dx, y + dy);
            if cx < 0 || cy < 0 || cx >= width || cy >= height {
                continue;
            }
            let Some(c) = control_at(w, width, height, row, cx, cy) else {
                continue;
            };
            let across = match c {
                Control::Volume
                | Control::Balance
                | Control::Position
                | Control::ShadeVolume
                | Control::ShadeBalance
                | Control::ShadePosition => dx == 0,
                Control::EqSlider(_) | Control::PlScroll => dy == 0,
                _ => true,
            };
            if across && takes(c) {
                best = Some((c, (cx, cy), d2));
            }
        }
    }
    best
}

/// Whether `c` is one of a title bar's buttons.
fn is_title_button(c: Control) -> bool {
    matches!(
        c,
        Control::Menu | Control::Minimize | Control::Shade | Control::Close
    )
}

impl SkinRuntime {
    /// The classic window kind of object `o`.
    fn classic_kind(&self, o: ObjId) -> Option<ClassicWindow> {
        match self.objects[o].kind {
            Kind::ClassicWindow(w) => Some(w),
            _ => None,
        }
    }

    /// The press on `o` now, when there is one.
    pub(crate) fn classic_press_on(&self, o: ObjId) -> Option<ClassicPress> {
        self.classic_press.filter(|p| p.object == o)
    }

    /// What a press at `px`, `py` of classic window `o` (of kind `w`) takes, and the point of
    /// it: the control there, the main window's vis only while something plays. Under touch a
    /// finger on a place without a control takes the nearest control within the touch slop, and
    /// one on the title bar the nearest of its buttons; farther off, the press moves the window.
    pub(crate) fn classic_touch_control(
        &self,
        o: ObjId,
        w: ClassicWindow,
        px: i32,
        py: i32,
    ) -> Option<(Control, (i32, i32))> {
        let rect = self.objects[o].base.rect;
        let size = (rect.width, rect.height, self.classic_row());
        let playing = self.player.status() == Status::Playing;
        let shown = |c: Control| c != Control::Vis || playing;
        let mut control = control_at(w, size.0, size.1, size.2, px, py)?;
        if !shown(control) {
            control = Control::Background;
        }
        if !self.touch {
            return Some((control, (px, py)));
        }
        let near = match control {
            // the equalizer's level labels set every band, too much for a finger beside them
            Control::Background => nearest_control(w, size, (px, py), self.touch_slop, |c| {
                shown(c)
                    && !matches!(
                        c,
                        Control::Background | Control::TitleBar | Control::EqLevel(_)
                    )
            }),
            Control::TitleBar => {
                nearest_control(w, size, (px, py), self.touch_slop, is_title_button)
            }
            _ => None,
        };
        Some(near.map_or((control, (px, py)), |(c, at, _)| (c, at)))
    }

    /// Under touch, the corner grip of classic window `o`'s window that a finger at `px`, `py`
    /// of it means: where the window has no control there and the grip lies within the touch
    /// slop, nearer than any control.
    pub(crate) fn classic_grip_near(&self, o: ObjId, px: i32, py: i32) -> Option<ObjId> {
        let w = self.classic_kind(o)?;
        let window = self.layout_object(o)?;
        let rect = self.objects[o].base.rect;
        let (x, y) = (px - rect.left, py - rect.top);
        let size = (rect.width, rect.height, self.classic_row());
        if !self.touch
            || self.classic_filled()
            || control_at(w, size.0, size.1, size.2, x, y) != Some(Control::Background)
        {
            return None;
        }
        let slop2 = i64::from(self.touch_slop) * i64::from(self.touch_slop);
        let (grip, d) = self
            .all_objects
            .iter()
            .copied()
            .filter(|g| {
                self.layout_object(*g) == Some(window)
                    && self.is_grip(*g)
                    && self.touchable(*g, true)
            })
            .map(|g| (g, self.distance2(g, px, py)))
            .min_by_key(|(_, d)| *d)
            .filter(|(_, d)| *d <= slop2)?;
        let control = nearest_control(w, size, (x, y), self.touch_slop, |c| {
            !matches!(c, Control::Background | Control::TitleBar)
        });
        control
            .is_none_or(|(_, _, c)| d <= i64::from(c))
            .then_some(grip)
    }

    /// The controls of the classic window `press` is on, in stack pixels, each by the box it
    /// answers in; while a playlist button is held, its menu's pictures too.
    pub(crate) fn classic_controls(
        &mut self,
        press: ClassicPress,
    ) -> Vec<crate::host::TouchControl> {
        let o = press.object;
        let (Some(w), Some(window)) = (self.classic_kind(o), self.layout_object(o)) else {
            return Vec::new();
        };
        let at = self.window_rect_of(window);
        let r = self.objects[o].base.rect;
        let row = self.classic_row();
        let mut boxes: Vec<(Control, (i32, i32, i32, i32))> = Vec::new();
        let mut index: std::collections::HashMap<Control, usize> = std::collections::HashMap::new();
        for y in 0..r.height {
            for x in 0..r.width {
                let Some(c) = control_at(w, r.width, r.height, row, x, y) else {
                    continue;
                };
                if matches!(c, Control::Background | Control::TitleBar) {
                    continue;
                }
                if let Some(i) = index.get(&c) {
                    let b = &mut boxes[*i].1;
                    *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
                } else {
                    index.insert(c, boxes.len());
                    boxes.push((c, (x, y, x, y)));
                }
            }
        }
        let left = at.left + r.left;
        let top = at.top + r.top;
        let mut out: Vec<crate::host::TouchControl> = boxes
            .into_iter()
            .map(|(_, (l, t, rr, b))| crate::host::TouchControl {
                rect: Rect::new(left + l, top + t, rr - l + 1, b - t + 1),
                magnify: false,
            })
            .collect();
        if let Control::PlButton(b) = press.control {
            // pl_menu_item_at: 22 by 18, the lowest ending at height - 12
            let x = b.menu_left(r.width) + 3;
            for i in 0..b.items() {
                let bottom = r.height - 12 - 18 * i;
                out.push(crate::host::TouchControl {
                    rect: Rect::new(left + x, top + bottom - 17, 22, 18),
                    magnify: true,
                });
            }
        }
        out
    }

    /// Whether a drag from the press on `o` moves the window: it began on the title bar or on
    /// a place without a control.
    pub(crate) fn classic_moves_window(&self, o: ObjId) -> bool {
        self.classic_press_on(o)
            .is_some_and(|p| matches!(p.control, Control::TitleBar | Control::Background))
    }

    /// Whether a classic window's corner grip (the playlist's, a gen window's) takes a press at
    /// `px`, `py` of its window: the corner's triangle where the distances to the right and
    /// bottom edges add up to 30 or less, as Winamp has it; the rest of the 20 by 20 square goes
    /// to the window.
    pub(crate) fn classic_grip_takes(&self, o: ObjId, px: i32, py: i32) -> bool {
        let id = self.objects[o].base.id.as_str();
        if !matches!(
            id,
            "classic.pl.sizer" | "classic.gen.sizer" | "classic.video.sizer"
        ) {
            return true;
        }
        let r = self.objects[o].base.rect;
        let (right, bottom) = (r.right() - px, r.bottom() - py);
        right < 20 && bottom < 20 && right + bottom <= 30
    }

    /// Whether `c` is one of the main window's control buttons, which hand a press along.
    fn is_cbutton(c: Control) -> bool {
        matches!(
            c,
            Control::Previous | Control::Play | Control::Pause | Control::Stop | Control::Next
        )
    }

    /// Whether a press on `from` slides to `to` under the finger: the main window's control
    /// buttons hand it along, and so do the video window's.
    fn hands_along(from: Control, to: Control) -> bool {
        (Self::is_cbutton(from) && Self::is_cbutton(to))
            || (matches!(from, Control::VideoButton(_)) && matches!(to, Control::VideoButton(_)))
    }

    pub(crate) fn classic_pointer_down(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        // a menu a tap left open takes the next press on its window: a picture is pressed, as a
        // held button's is, and a press anywhere else only closes the menu
        if let Some((menu_on, b)) = self.classic_open_menu {
            self.repaint();
            if menu_on == o {
                if let Some(i) = pl_menu_item_at(b, rect.width, rect.height, px, py) {
                    self.classic_press = Some(ClassicPress {
                        object: o,
                        control: Control::PlButton(b),
                        at: (px, py),
                        inside: true,
                        position: 0,
                        hover: Some(i),
                        from: (px, py),
                        scroll_from: self.classic_pl_scroll,
                        grab: px,
                        acted: false,
                        dragged: false,
                        on_open_menu: true,
                        aim: (px, py),
                    });
                    return;
                }
                self.classic_open_menu = None;
                return;
            }
            self.classic_open_menu = None;
        }
        let Some((control, aim)) = self.classic_touch_control(o, w, px, py) else {
            return;
        };
        let mut press = ClassicPress {
            object: o,
            control,
            at: (px, py),
            inside: true,
            position: 0,
            hover: None,
            from: (px, py),
            scroll_from: self.classic_pl_scroll,
            grab: px,
            acted: false,
            dragged: false,
            on_open_menu: false,
            aim,
        };
        self.classic_grab(&mut press, px);
        self.classic_follow(&mut press, px, py);
        // these act on the press, as Winamp handles them on the button going down
        if matches!(
            control,
            Control::Time
                | Control::Vis
                | Control::ShadeVis
                | Control::PlMini(_)
                | Control::PlTime
                | Control::PlScrollUp
                | Control::PlScrollDown
        ) {
            self.classic_act(o, w, press);
            press.acted = true;
        }
        if control == Control::TitleText {
            self.classic_title_hold();
        }
        self.classic_press = Some(press);
        self.repaint();
    }

    /// Where the finger took a slider's thumb: on the thumb, the spot it pressed; beside it, the
    /// thumb's middle.
    fn classic_grab(&mut self, press: &mut ClassicPress, px: i32) {
        match press.control {
            Control::Volume => {
                let left = 107 + self.player.volume().clamp(0, 255) * 51 / 255;
                press.grab = if (left..left + 15).contains(&px) {
                    px - left
                } else {
                    7
                };
            }
            Control::Balance => {
                let left = 189 + self.player.pan().clamp(-127, 127) * 12 / 127;
                press.grab = if (left..left + 15).contains(&px) {
                    px - left
                } else {
                    7
                };
            }
            Control::EqSlider(n) => {
                // on the thumb (its 12 rows from 89 + its offset) the spot taken, else 5
                let v = if n == 0 {
                    self.player.eq_preamp()
                } else {
                    self.player.eq_bands().get(n - 1).copied().unwrap_or(128)
                };
                let off = (crate::classic::eq::band_step(v) - 63) * 52 / 64;
                let py = press.from.1;
                press.grab = if (off + 89..off + 101).contains(&py) {
                    py - off - 90
                } else {
                    5
                };
            }
            Control::Position => {
                let length = self.player.length_ms();
                let at = if length > 0 {
                    (i64::from(self.player.position_ms()) * 218 / i64::from(length)) as i32
                } else {
                    0
                };
                press.grab = if (at + 17..=at + 47).contains(&px) {
                    px - (at + 18)
                } else {
                    14
                };
            }
            _ => {}
        }
    }

    pub(crate) fn classic_pointer_move(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(mut press) = self.classic_press_on(o) else {
            return;
        };
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        press.at = (px, py);
        let under = control_at(w, rect.width, rect.height, self.classic_row(), px, py);
        // a finger that wobbles within the touch slop of where it went down stays on its
        // control, where a mouse would have left a box this small
        let near = self.classic_near(&press, px, py);
        press.inside = under == Some(press.control) || near;
        // the control buttons hand the press to the one under the finger
        if let Some(c) = under.filter(|c| !near && Self::hands_along(press.control, *c)) {
            press.control = c;
            press.inside = true;
        }
        if matches!(press.control, Control::PlRow(_)) && (py - press.from.1).abs() >= DRAG_SLOP {
            press.dragged = true;
        }
        // a drag between the bands goes on with the slider the finger reaches
        if press.control == Control::EqSweep {
            if let Some(c @ Control::EqSlider(_)) = under {
                press.control = c;
                press.inside = true;
            }
        }
        self.classic_follow(&mut press, px, py);
        // a band dragged sideways off its column lets go (not the preamp)
        if let Control::EqSlider(n) = press.control {
            let x = n as i32 * 18 + 60;
            if n > 0
                && (px < x - 3 || px > x + 15)
                && (78..=258).contains(&px)
                && (39..=98).contains(&py)
            {
                press.control = Control::EqSweep;
                press.inside = false;
                self.classic_status_end();
            }
        }
        // a press that moves its window shows nothing new: the window only goes along
        let moving = matches!(press.control, Control::TitleBar | Control::Background);
        self.classic_press = Some(press);
        if !moving {
            self.repaint();
        }
    }

    /// Selects playlist row `row` (or none) and tells the host, which keeps the selection the
    /// bottom buttons' picks act on (`PLAYLIST_SELECT` with the row, or no param for none).
    fn classic_select_row(&mut self, o: ObjId, row: Option<i32>) {
        if self.classic_selected_row == row {
            return;
        }
        self.classic_selected_row = row;
        let param = row.map(|r| r.to_string());
        self.action(o, "PLAYLIST_SELECT", param.as_deref(), None);
    }

    /// Whether the finger at `px`, `py` is still within the touch slop of where `press` went
    /// down.
    fn classic_near(&self, press: &ClassicPress, px: i32, py: i32) -> bool {
        let (dx, dy) = (px - press.from.0, py - press.from.1);
        !self.pointer.precise && dx * dx + dy * dy <= self.touch_slop * self.touch_slop
    }

    pub(crate) fn classic_pointer_up(&mut self, o: ObjId, px: i32, py: i32, inside: bool) {
        let Some(mut press) = self.classic_press_on(o) else {
            return;
        };
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        press.at = (px, py);
        self.classic_follow(&mut press, px, py);
        // a release acts over the control; the position bar seeks wherever the finger let go,
        // since its thumb followed the drag
        let under = control_at(w, rect.width, rect.height, self.classic_row(), px, py);
        let near = self.classic_near(&press, px, py);
        if let Some(c) = under.filter(|c| !near && Self::hands_along(press.control, *c)) {
            press.control = c;
        }
        let over = inside && (under == Some(press.control) || near);
        // a bottom button's menu is picked by the picture under the finger, above the button
        let seeks = matches!(press.control, Control::Position | Control::ShadePosition);
        let picks = matches!(press.control, Control::PlButton(_)) && press.hover.is_some();
        // under touch a tap on a bottom button leaves its menu open; a slide to a picture still
        // picks it on release, and a press on a picture of the open menu closes it, picked or not
        if let (true, Control::PlButton(b)) = (self.touch, press.control) {
            if !press.on_open_menu && near {
                self.classic_open_menu = Some((o, b));
                self.classic_press = None;
                self.classic_release(press);
                self.repaint();
                return;
            }
            self.classic_open_menu = None;
        }
        self.classic_press = None;
        self.classic_release(press);
        if (over || seeks || picks) && !press.dragged && !press.acted {
            self.classic_act(o, w, press);
        }
        self.repaint();
    }

    pub(crate) fn classic_pointer_cancel(&mut self, o: ObjId) {
        if let Some(press) = self.classic_press_on(o) {
            self.classic_press = None;
            self.classic_release(press);
            self.repaint();
        }
    }

    /// What ends with any press: the title row's text and its scroll come back, a balance near
    /// the middle snaps to it (within 26 either side).
    fn classic_release(&mut self, press: ClassicPress) {
        if self.classic_style.status_hold_ms().is_none() {
            self.classic_status_end();
        }
        match press.control {
            Control::Balance => {
                if (-26..=26).contains(&self.player.pan()) {
                    self.player.set_pan(0);
                }
            }
            Control::TitleText => self.classic_title_resume(2000),
            _ => {}
        }
    }

    /// Shows `text` on the title row in place of the song while a slider is dragged, the title's
    /// scroll held meanwhile.
    fn classic_say(&mut self, text: String) {
        if self.classic_status.is_none() {
            self.classic_title_hold();
        }
        self.classic_status = Some(text);
        self.classic_status_until = self
            .classic_style
            .status_hold_ms()
            .map_or(i64::MAX, |ms| self.now() + ms);
    }

    /// Gives the title row back to the song, its scroll going on.
    pub(crate) fn classic_status_end(&mut self) {
        if self.classic_status.take().is_some() {
            self.classic_title_resume(0);
            self.repaint();
        }
        self.classic_status_until = i64::MAX;
    }

    /// A right click, or a long press: the host's menu, as the title bar's menu button opens it.
    pub(crate) fn classic_right_click(&mut self, o: ObjId) {
        // the menu pops where the finger held
        self.action_from = self.press_point().map(|(x, y)| Rect::new(x, y, 1, 1));
        self.action(o, "MENU", None, None);
    }

    /// A double tap, as Winamp's double clicks go in the main window, the equalizer and the
    /// playlist: the title row toggles the shade bar; the main window's song title asks for the
    /// file info; its vis puts the mode back to what it was before the taps and asks for the
    /// visualization plugin; a playlist row plays its song; a playlist bottom button picks its
    /// lowest item.
    pub(crate) fn classic_double_tap(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        let at = control_at(w, rect.width, rect.height, self.classic_row(), px, py);
        // the shade bar is all title row: its buttons, time and vis take a double tap as
        // another press (or the vis's revert) rather than a toggle of the shade
        let title_row = py < 15
            && px
                < match w {
                    ClassicWindow::Main | ClassicWindow::MainShade => 244,
                    ClassicWindow::Equalizer | ClassicWindow::EqualizerShade => 252,
                    _ => rect.width - 20,
                }
            && (w != ClassicWindow::MainShade
                || matches!(at, Some(Control::TitleBar | Control::Menu)));
        if title_row {
            return self.classic_toggle_shade(o, w);
        }
        match (w, at) {
            (ClassicWindow::Main, Some(Control::TitleText)) if (25..35).contains(&py) => {
                self.action(o, "TRACKINFO", None, None);
            }
            (ClassicWindow::Main, _) if (24..100).contains(&px) && (43..59).contains(&py) => {
                self.classic_vis_mode = self.classic_vis_before;
                self.action(o, "VIS_FS", None, None);
            }
            (ClassicWindow::MainShade, Some(Control::ShadeVis)) => {
                self.classic_vis_mode = self.classic_vis_before;
                self.action(o, "VIS_FS", None, None);
            }
            (_, Some(Control::PlRow(i))) => {
                let index = self.classic_pl_scroll(rect.height) + i;
                if (index as usize) < self.player.tracks().len() {
                    self.classic_select_row(o, Some(index));
                    self.player.play_track(index);
                }
            }
            (_, Some(Control::PlButton(b))) => {
                if let Some(choice) = b.choice(0) {
                    self.action(o, "PLAYLIST", Some(choice), None);
                    // the list the selection pointed into changed, or the selection was cleared
                    if CLEARS_SELECTION.contains(&choice) {
                        self.classic_select_row(o, None);
                    }
                }
            }
            _ => {}
        }
    }

    /// The sliders follow the finger while pressed, telling their value on the title row; the
    /// position bar only notes where it is; the title row scrolls with the finger.
    fn classic_follow(&mut self, press: &mut ClassicPress, px: i32, py: i32) {
        match press.control {
            Control::ClutterO
            | Control::ClutterA
            | Control::ClutterI
            | Control::ClutterD
            | Control::ClutterV => {
                // the letter under the held finger names what it does on the title row
                if let Some(text) = self.classic_clutter_text(px, py) {
                    self.classic_say(text);
                }
            }
            Control::Volume => {
                let v = ((px - press.grab - 107) * 255 / 51).clamp(0, 255);
                self.player.set_volume(v);
                self.classic_say_volume();
            }
            Control::Balance => {
                // the thumb's place 0..=24; a step either side of the middle snaps to it
                let at = (px - press.grab - 177).clamp(0, 24);
                let b = if self.classic_style.balance_snap().contains(&at) {
                    0
                } else {
                    (at - 12) * 127 / 12
                };
                self.player.set_pan(b);
                self.classic_say_balance();
            }
            Control::Position => {
                press.position = (px - press.grab - 17).clamp(0, 218);
                self.classic_say_seek(press.position);
            }
            Control::ShadePosition => {
                press.position = ((px - 228) * 218 / 13).clamp(0, 218);
                self.classic_say_seek(press.position);
            }
            Control::ShadeVolume => {
                // 96 columns from x 61, no grab
                let v = (px - 61).clamp(0, 96) * 255 / 96;
                self.player.set_volume(v);
                self.classic_say_volume();
            }
            Control::ShadeBalance => {
                // 42 columns from x 164, no grab; near the middle snaps to it while dragged
                // (5.666 skips that with Shift, which a touch never holds), and the release
                // keeps what the drag gave
                let b = self.classic_style.shade_balance((px - 164).clamp(0, 42));
                self.player.set_pan(b);
                self.classic_say_balance();
            }
            Control::TitleText => {
                // dragged left the title moves on, the step added to the scroll
                self.classic_title_base += press.grab - px;
                press.grab = px;
            }
            Control::PlButton(b) => {
                let rect = self.objects[press.object].base.rect;
                press.hover = pl_menu_item_at(b, rect.width, rect.height, px, py);
            }
            Control::PlRow(_) if press.dragged => {
                // the rows scroll with the finger, a row per row height
                let h = self.objects[press.object].base.rect.height;
                let rows = (press.from.1 - py) / self.classic_row();
                let past = self.player.tracks().len() as i32 - self.classic_pl_rows(h);
                self.classic_pl_scroll = (press.scroll_from + rows).clamp(0, past.max(0));
            }
            Control::PlScroll => {
                // the thumb's middle follows the finger along the track; the row is the
                // thumb's share of the rows past the window, cut down as Winamp cuts it (a
                // click on the track at 50 of a 116-tall list of 20 shows row 9 in the VM)
                let h = self.objects[press.object].base.rect.height;
                let past = self.player.tracks().len() as i32 - self.classic_pl_rows(h);
                let track = h - 76;
                if past > 0 && track > 0 {
                    let along = (py - 20 - 9).clamp(0, track);
                    self.classic_pl_scroll = along * past / track;
                }
            }
            Control::EqSlider(n) => {
                // the band's step 0..=63 (0 at the top) from the finger less where
                // it took the thumb, over 52 rows to 64 steps; a step either side of the middle
                // snaps to it (unless Shift, which a touch never holds)
                let step = ((py - press.grab - 39) * 64 / 52).clamp(0, 63);
                let step = if (30..=32).contains(&step) { 31 } else { step };
                let v = if step == 31 {
                    crate::runtime::EQ_FLAT
                } else {
                    (63 - step) * 255 / 63
                };
                if n == 0 {
                    self.player.set_eq_preamp(v);
                } else {
                    self.player.set_eq_band(n as i32 - 1, v);
                }
                self.classic_say_eq(n, step);
            }
            _ => {}
        }
    }

    /// "Volume: 50%".
    fn classic_say_volume(&mut self) {
        let pct = self.player.volume().clamp(0, 255) * 100 / 255;
        self.classic_say(format!("Volume: {pct}%"));
    }

    /// "Balance: 30% left", or "Balance: Center".
    fn classic_say_balance(&mut self) {
        let pct = self.player.pan().clamp(-127, 127) * 100 / 127;
        let text = match pct {
            0 => "Balance: Center".to_string(),
            p if p < 0 => format!("Balance: {}% left", -p),
            p => format!("Balance: {p}% right"),
        };
        self.classic_say(text);
    }

    /// "Seek To: 01:23/10:00 (13%)" for a position bar at `at` of its 218 steps.
    fn classic_say_seek(&mut self, at: i32) {
        let length = self.player.length_ms().max(0);
        let to = (i64::from(length) * i64::from(at) / 218 / 1000) as i32;
        let of = length / 1000;
        let pct = to * 100 / of.max(1);
        self.classic_say(format!(
            "Seek To: {:02}:{:02}/{:02}:{:02} ({pct}%)",
            to / 60,
            to % 60,
            of / 60,
            of % 60
        ));
    }

    /// What the clutter bar's letter at `px`, `py` says while held, the always-on-top and
    /// double size ones by what a press would do.
    fn classic_clutter_text(&mut self, px: i32, py: i32) -> Option<String> {
        let row = self.classic_row();
        let on = |rt: &mut Self, kind| rt.host.window_open(kind) == Some(true);
        let text = match control_at(ClassicWindow::Main, 275, 116, row, px, py)? {
            Control::ClutterO => "Options Menu",
            Control::ClutterA if on(self, "ontop") => "Disable Always-On-Top",
            Control::ClutterA => "Enable Always-On-Top",
            Control::ClutterI => "File Info Box",
            Control::ClutterD if on(self, "doublesize") => "Disable Doublesize Mode",
            Control::ClutterD => "Enable Doublesize Mode",
            Control::ClutterV => "Visualization Menu",
            _ => return None,
        };
        Some(text.to_string())
    }

    /// The title row's text for equalizer slider `n` (0 the preamp) at step `band_step`.
    fn classic_say_eq(&mut self, n: usize, band_step: i32) {
        self.classic_say(self.classic_style.eq_text(n, band_step));
    }

    /// What a release over the pressed control does.
    fn classic_act(&mut self, o: ObjId, w: ClassicWindow, press: ClassicPress) {
        // a menu the control opens goes against the control
        let r = self.objects[o].base.rect;
        self.action_from = control_box(w, r.width, r.height, self.classic_row(), press.aim)
            .and_then(|b| {
                let l = self.layout_object(o)?;
                let b = Rect::new(r.left + b.left, r.top + b.top, b.width, b.height);
                Some(self.stack_rect_in(l, b))
            });
        self.classic_act_on(o, w, &press);
        self.action_from = None;
    }

    /// What `press` on `o`, a window of kind `w`, does as it ends.
    fn classic_act_on(&mut self, o: ObjId, w: ClassicWindow, press: &ClassicPress) {
        match press.control {
            Control::Previous => self.player.previous(),
            Control::Play => self.player.play(),
            Control::Pause => self.player.pause(),
            Control::Stop => self.player.stop(),
            Control::Next => self.player.next(),
            Control::Eject => self.action(o, "EJECT", None, None),
            Control::Shuffle => {
                let on = !self.player.shuffle();
                self.player.set_shuffle(on);
            }
            Control::Repeat => {
                let on = !self.player.repeat();
                self.player.set_repeat(on);
            }
            Control::EqOn => {
                let on = !self.player.eq_enabled();
                self.player.set_eq_enabled(on);
            }
            Control::EqAuto => self.classic_eq_auto = !self.classic_eq_auto,
            Control::EqToggle => self.classic_toggle_window("equalizer"),
            Control::PlToggle => self.classic_toggle_window("pledit"),
            Control::Shade => self.classic_toggle_shade(o, w),
            Control::Close => {
                if let Some(c) = self.objects[o]
                    .layout()
                    .and_then(|d| d.container)
                    .or_else(|| {
                        self.layout_object(o)
                            .and_then(|l| self.objects[l].layout().and_then(|d| d.container))
                    })
                {
                    if Some(c) != self.main_container() {
                        self.close_container(c);
                    }
                }
            }
            Control::Position | Control::ShadePosition => {
                let length = self.player.length_ms();
                if self.player.status() != Status::Stopped && length > 0 {
                    let ms = (i64::from(press.position) * i64::from(length) / 218) as i32;
                    self.player.seek_to(ms);
                }
            }
            Control::Logo => self.action(o, "ABOUT", None, None),
            Control::VideoButton(i) => {
                if let Some(a) = usize::try_from(i)
                    .ok()
                    .and_then(|i| crate::classic::video::ACTIONS.get(i))
                {
                    self.action(o, a, None, None);
                }
            }
            Control::ShadeVis => self.classic_vis_step(),
            Control::PlMini(i) => match i {
                0 => self.player.previous(),
                1 => self.player.play(),
                2 => self.player.pause(),
                3 => self.player.stop(),
                4 => self.player.next(),
                _ => self.action(o, "EJECT", None, None),
            },
            Control::PlTime => self.classic_remaining = !self.classic_remaining,
            Control::PlScrollUp | Control::PlScrollDown => {
                let h = self.objects[o].base.rect.height;
                let past = self.player.tracks().len() as i32 - self.classic_pl_rows(h);
                let rows = self.classic_style.arrow_rows();
                let step = if press.control == Control::PlScrollUp {
                    -rows
                } else {
                    rows
                };
                self.classic_pl_scroll = (self.classic_pl_scroll(h) + step).clamp(0, past.max(0));
            }
            Control::Menu => self.action(o, "MENU", None, None),
            Control::ClutterO => self.action(o, "MENU", Some("wa5:options"), None),
            Control::ClutterA => self.action(o, "ALWAYS_ON_TOP", None, None),
            Control::ClutterI => self.action(o, "TRACKINFO", None, None),
            Control::ClutterD => self.action(o, "DOUBLESIZE", None, None),
            Control::ClutterV => self.action(o, "VIS_MENU", None, None),
            Control::EqPresets => self.action(o, "EQ_PRESETS", None, None),
            Control::EqLevel(level) => {
                for band in 0..10 {
                    self.player.set_eq_band(band, level);
                }
            }
            Control::PlButton(b) => {
                if let Some(choice) = press.hover.and_then(|i| b.choice(i)) {
                    self.action(o, "PLAYLIST", Some(choice), None);
                    // the list the selection pointed into changed, or the selection was cleared
                    if CLEARS_SELECTION.contains(&choice) {
                        self.classic_select_row(o, None);
                    }
                }
            }
            Control::Time => self.classic_remaining = !self.classic_remaining,
            Control::Vis => self.classic_vis_step(),
            Control::PlRow(i) => {
                let h = self.objects[o].base.rect.height;
                let index = self.classic_pl_scroll(h) + i;
                if (index as usize) < self.player.tracks().len() {
                    self.classic_select_row(o, Some(index));
                }
            }
            _ => {}
        }
    }

    /// The vis mode one on, keeping the mode before the first of two quick taps for a double
    /// tap to put back.
    fn classic_vis_step(&mut self) {
        let now = self.now();
        if now - self.classic_vis_tapped_at > 500 {
            self.classic_vis_before = self.classic_vis_mode;
        }
        self.classic_vis_tapped_at = now;
        self.classic_vis_mode = self.classic_vis_mode.next();
    }

    /// Opens the window of container `id`, or closes it when open.
    fn classic_toggle_window(&mut self, id: &str) {
        let open = self.container(id).is_some_and(|c| self.is_open(c));
        if open {
            self.close_window(id);
        } else {
            self.open_window(id);
        }
    }

    /// Shows the window of `o` in its other look: the shade bar, or the normal layout again.
    fn classic_toggle_shade(&mut self, o: ObjId, w: ClassicWindow) {
        let Some(l) = self.layout_object(o) else {
            return;
        };
        let Some(c) = self.objects[l].layout().and_then(|d| d.container) else {
            return;
        };
        let to = match w {
            ClassicWindow::MainShade
            | ClassicWindow::EqualizerShade
            | ClassicWindow::PlaylistShade => "normal",
            _ => "shade",
        };
        self.switch_to(c, to);
    }
}

/// The steps a classic window's size takes, wide and tall: 25 by 29 pixels from 275 by 116,
/// rounded up as Winamp rounds them.
pub(crate) const CLASSIC_SIZE_STEP: (i32, i32) = (25, 29);

/// `v` rounded up to a whole number of `step`s.
pub(crate) fn classic_step_up(v: i32, step: i32) -> i32 {
    (v + step - 1).div_euclid(step) * step
}
