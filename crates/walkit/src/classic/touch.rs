// SPDX-License-Identifier: GPL-3.0-or-later
//! Touches on the classic windows, as Winamp's classic windows handle the mouse: each
//! control answers in a box (in the main window,
//! the equalizer and the playlist), most act on release over
//! the control and a few on the press, the sliders follow the finger from where they were
//! grabbed, the control buttons hand the press along as the finger slides over them, a drag on
//! any place without a control moves the window, and double taps do what Winamp's double clicks
//! do.

use crate::classic::ClassicWindow;
use crate::gui::{Kind, ObjId};
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// A control of a classic window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

/// How far a finger moves on the playlist's rows before the press scrolls them instead of
/// picking a row.
const DRAG_SLOP: i32 = 8;

/// Whether `x`, `y` lies in the box.
fn hit(x: i32, y: i32, bx: i32, by: i32, bw: i32, bh: i32) -> bool {
    x >= bx && x < bx + bw && y >= by && y < by + bh
}

/// The control of window `w` (sized `width` x `height`) at `x`, `y`.
#[allow(clippy::enum_glob_use)]
pub fn control_at(w: ClassicWindow, width: i32, height: i32, x: i32, y: i32) -> Option<Control> {
    use Control::*;
    match w {
        ClassicWindow::Main => {
            // the box each control answers in (probed in the VM): title buttons, clutter bar,
            // time and vis, sliders,
            // position bar, control buttons, eject,
            // shuffle and repeat, the window toggles, the
            // logo, the title row
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
            // the bar's transport buttons split x 168..=224 at 176, 186, 195, 204 and 215
            // and its mini position bar is x 228..=242, its time
            // x 129..=157, its small vis x 78..=116
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
                let row = (y - crate::classic::pl::ROW_TOP) / crate::classic::pl::ROW_HEIGHT;
                return Some(PlRow(row));
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

    /// Whether a drag from the press on `o` moves the window: it began on the title bar or on
    /// a place without a control.
    pub(crate) fn classic_moves_window(&self, o: ObjId) -> bool {
        self.classic_press_on(o)
            .is_some_and(|p| matches!(p.control, Control::TitleBar | Control::Background))
    }

    /// Whether a classic window's corner grip (the playlist's, a gen window's) takes a press at
    /// `px`, `py` of its window: the corner's triangle where the distances to the right and
    /// bottom edges add up to 30 or less, as Winamp
    /// has it; the rest of the 20 by 20 square goes to the window.
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
        let Some(mut control) = control_at(w, rect.width, rect.height, px, py) else {
            return;
        };
        let playing = self.player.status() == Status::Playing;
        // the main window's vis answers only while something plays; else it is a place to
        // move the window by
        if control == Control::Vis && !playing {
            control = Control::Background;
        }
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
        let under = control_at(w, rect.width, rect.height, px, py);
        press.inside = under == Some(press.control);
        // the control buttons hand the press to the one under the finger
        if let Some(c) = under.filter(|c| Self::hands_along(press.control, *c)) {
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
                if self.classic_status.take().is_some() {
                    self.classic_title_resume(0);
                }
            }
        }
        self.classic_press = Some(press);
        self.repaint();
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
        let under = control_at(w, rect.width, rect.height, px, py);
        if let Some(c) = under.filter(|c| Self::hands_along(press.control, *c)) {
            press.control = c;
        }
        let over = inside && under == Some(press.control);
        // a bottom button's menu is picked by the picture under the finger, above the button
        let seeks = matches!(press.control, Control::Position | Control::ShadePosition);
        let picks = matches!(press.control, Control::PlButton(_)) && press.hover.is_some();
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
        if self.classic_status.take().is_some() {
            self.classic_title_resume(0);
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
    }

    /// A right click, or a long press: the host's menu, as the title bar's menu button opens it.
    pub(crate) fn classic_right_click(&mut self, o: ObjId) {
        self.action(o, "MENU", None, None);
    }

    /// A double tap, as Winamp's double clicks go (main window,
    /// equalizer, playlist): the title row toggles the shade bar;
    /// the main window's song title asks for the file info; its vis puts the mode back to what
    /// it was before the taps and asks for the visualization plugin; a playlist row plays its
    /// song; a playlist bottom button picks its lowest item.
    pub(crate) fn classic_double_tap(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        let title_row = py < 15
            && px
                < match w {
                    ClassicWindow::Main | ClassicWindow::MainShade => 244,
                    ClassicWindow::Equalizer | ClassicWindow::EqualizerShade => 252,
                    _ => rect.width - 20,
                };
        if title_row {
            return self.classic_toggle_shade(o, w);
        }
        match (w, control_at(w, rect.width, rect.height, px, py)) {
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
                    self.classic_selected_row = Some(index);
                    self.player.play_track(index);
                }
            }
            (_, Some(Control::PlButton(b))) => {
                if let Some(choice) = b.choice(0) {
                    if choice == "select_none" {
                        self.classic_selected_row = None;
                    }
                    self.action(o, "PLAYLIST", Some(choice), None);
                }
            }
            _ => {}
        }
    }

    /// The sliders follow the finger while pressed, telling their value on the title row; the
    /// position bar only notes where it is; the title row scrolls with the finger.
    fn classic_follow(&mut self, press: &mut ClassicPress, px: i32, py: i32) {
        match press.control {
            Control::Volume => {
                let v = ((px - press.grab - 107) * 255 / 51).clamp(0, 255);
                self.player.set_volume(v);
                self.classic_say_volume();
            }
            Control::Balance => {
                // the thumb's place 0..=24; a step either side of the middle snaps to it
                let at = (px - press.grab - 177).clamp(0, 24);
                let b = if (11..=13).contains(&at) {
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
                // 42 columns from x 164, no grab; 8 either side of the middle
                // snaps to it while dragged (unless Shift, which a touch never holds), and the
                // release keeps what the drag gave
                let b = ((px - 164).clamp(0, 42) * 255 / 42 - 127).min(127);
                let b = if (-8..=8).contains(&b) { 0 } else { b };
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
                let rows = (press.from.1 - py) / crate::classic::pl::ROW_HEIGHT;
                let past = self.player.tracks().len() as i32 - SkinRuntime::classic_pl_rows(h);
                self.classic_pl_scroll = (press.scroll_from + rows).clamp(0, past.max(0));
            }
            Control::PlScroll => {
                // the thumb's middle follows the finger along the track; the row is the
                // thumb's share of the rows past the window, cut down as Winamp cuts it (a
                // click on the track at 50 of a 116-tall list of 20 shows row 9 in the VM)
                let h = self.objects[press.object].base.rect.height;
                let past = self.player.tracks().len() as i32 - SkinRuntime::classic_pl_rows(h);
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

    /// "EQ: 70Hz: +3.0 dB" for slider `n` (0 the preamp) at step `step` 0..=63
    /// ((step - 31.5) / 31.5 * -12 dB, within 0.32 of zero shown as zero).
    fn classic_say_eq(&mut self, n: usize, step: i32) {
        const BANDS: [&str; 10] = ["70", "180", "320", "600", "1", "3", "6", "12", "14", "16"];
        let db = (f64::from(step) - 31.5) / 31.5 * -12.0;
        let db = if (-0.32..0.32).contains(&db) { 0.0 } else { db };
        let sign = if db < 0.0 { "" } else { "+" };
        let name = if n == 0 {
            "Preamp".to_string()
        } else {
            let unit = if n >= 5 { "KHz" } else { "Hz" };
            format!("{}{unit}", BANDS[n - 1])
        };
        self.classic_say(format!("EQ: {name}: {sign}{db:.1} dB"));
    }

    /// What a release over the pressed control does.
    fn classic_act(&mut self, o: ObjId, w: ClassicWindow, press: ClassicPress) {
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
                let past = self.player.tracks().len() as i32 - SkinRuntime::classic_pl_rows(h);
                let step = if press.control == Control::PlScrollUp {
                    -1
                } else {
                    1
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
                    if choice == "select_none" {
                        self.classic_selected_row = None;
                    }
                    self.action(o, "PLAYLIST", Some(choice), None);
                }
            }
            Control::Time => self.classic_remaining = !self.classic_remaining,
            Control::Vis => self.classic_vis_step(),
            Control::PlRow(i) => {
                let h = self.objects[o].base.rect.height;
                let index = self.classic_pl_scroll(h) + i;
                if (index as usize) < self.player.tracks().len() {
                    self.classic_selected_row = Some(index);
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
