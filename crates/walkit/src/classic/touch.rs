// SPDX-License-Identifier: GPL-3.0-or-later
//! Touches on the classic windows: the controls under a point are the ones the painters draw
//! (each control's box is where it is drawn), a press shows the control pressed, a release
//! inside it acts, the sliders follow the finger, the title bars move the window and a double
//! tap on one toggles the shade bar.

use crate::classic::ClassicWindow;
use crate::gui::{Kind, ObjId};
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// A control of a classic window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// The bar a drag moves the window by.
    TitleBar,
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
            let boxes: [(Control, i32, i32, i32, i32); 24] = [
                // the title buttons answer a row below their pictures and, at the right, a
                // column beyond; the VM showed minimize reaching x 253 and the equalizer's
                // shade and close boxes at x 253..=263 and 264..=273, y 3..=12
                (Menu, 6, 3, 9, 10),
                (Minimize, 244, 3, 10, 10),
                (Shade, 254, 3, 10, 10),
                (Close, 264, 3, 10, 10),
                (ClutterO, 10, 22, 8, 8),
                (ClutterA, 10, 30, 8, 8),
                (ClutterI, 10, 38, 8, 8),
                (ClutterD, 10, 46, 8, 8),
                (ClutterV, 10, 54, 8, 11),
                // the time's press box as probed in the VM: x 36..=97, y 26..=39
                (Time, 36, 26, 62, 14),
                // the vis area answers presses from 3 rows above its picture to 3 below
                // (Winamp 5.666, probed by clicks in the VM: y 40..=61, x 24..=99)
                (Vis, 24, 40, 76, 22),
                // the sliders answer presses on the rows their thumbs travel (probed in the
                // VM): volume x 107..=175 and balance x 177..=215, both y 58..=66; the
                // position bar x 18..=263 (16 and 17 miss), y 73..=81
                (Volume, 107, 58, 69, 9),
                (Balance, 177, 58, 39, 9),
                // the toggles and the control buttons answer presses on boxes probed in the
                // VM, a column or row beside their blits: EQ x 219..=242, PL x 243..=265, both
                // y 58..=70; previous x 15..=37, play 38..=60, pause 61..=82, stop 83..=106,
                // next 107..=130, all y 87..=106
                (EqToggle, 219, 58, 24, 13),
                (PlToggle, 243, 58, 23, 13),
                (Position, 18, 73, 246, 9),
                (Previous, 15, 87, 23, 20),
                (Play, 38, 87, 23, 20),
                (Pause, 61, 87, 22, 20),
                (Stop, 83, 87, 24, 20),
                (Next, 107, 87, 24, 20),
                (Eject, 136, 89, 22, 16),
                // shuffle and repeat answer presses on boxes of their own, probed in the VM:
                // shuffle x 165..=209, y 90..=102; repeat x 210..=239, y 89..=103
                (Shuffle, 165, 90, 45, 13),
                (Repeat, 210, 89, 30, 15),
            ];
            for (c, bx, by, bw, bh) in boxes {
                if hit(x, y, bx, by, bw, bh) {
                    return Some(c);
                }
            }
            (y < 14).then_some(TitleBar)
        }
        ClassicWindow::MainShade => {
            // the bar's transport buttons sit nine columns apart from x 168 (probed in the VM:
            // play x 177..=185, stop x 195..=203, rows 2..=11); its mini position bar answers
            // x 230..=242, y 3..=12
            let boxes: [(Control, i32, i32, i32, i32); 12] = [
                (Menu, 6, 3, 9, 10),
                (Minimize, 244, 3, 10, 10),
                (Shade, 254, 3, 10, 10),
                (Close, 264, 3, 10, 10),
                (Previous, 168, 2, 9, 10),
                (Play, 177, 2, 9, 10),
                (Pause, 186, 2, 9, 10),
                (Stop, 195, 2, 9, 10),
                (Next, 204, 2, 9, 10),
                (Eject, 213, 2, 9, 10),
                (ShadePosition, 230, 3, 13, 10),
                (Time, 126, 4, 32, 6),
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
            if y < 14 {
                return Some(TitleBar);
            }
            // ON and AUTO answer a row more than their pictures; a slider's box stops a column
            // short of its picture's right edge (probed in the VM)
            if hit(x, y, 14, 18, 25, 13) {
                return Some(EqOn);
            }
            if hit(x, y, 39, 18, 34, 13) {
                return Some(EqAuto);
            }
            if hit(x, y, 217, 18, 44, 12) {
                return Some(EqPresets);
            }
            // the labels between the preamp and the bands: x 42..=67, "+12" on
            // rows 33..=42, "0" on 65..=74, "-12" on 92..=101
            for (top, level) in [(33, 255), (65, crate::runtime::EQ_FLAT), (92, 0)] {
                if hit(x, y, 42, top, 26, 10) {
                    return Some(EqLevel(level));
                }
            }
            if hit(x, y, 21, 38, 13, 63) {
                return Some(EqSlider(0));
            }
            for n in 1..=10 {
                if hit(x, y, n * 18 + 60, 38, 13, 63) {
                    return Some(EqSlider(n as usize));
                }
            }
            None
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
            if y < 20 {
                return Some(TitleBar);
            }
            // the scrollbar's track answers a column beyond its picture (probed in the VM at
            // width 275: x 260..=268, 259 dead)
            if hit(x, y, width - 15, 20, 9, height - 58) {
                return Some(PlScroll);
            }
            // the rows answer presses up to two columns short of the scrollbar (probed in
            // the VM at width 275: x 12..=257, with 250 the last column tried and 259 dead)
            if hit(x, y, 12, 20, width - 29, height - 58) {
                return Some(PlRow((y - 20) / 14));
            }
            // the bottom buttons, 25 by 18 from the bottom-left piece and the right one
            if y >= height - 30 && y < height - 12 {
                for (b, left) in [
                    (PlMenu::Add, 11),
                    (PlMenu::Remove, 40),
                    (PlMenu::Select, 69),
                    (PlMenu::Misc, 98),
                    (PlMenu::List, width - 46),
                ] {
                    if x >= left && x < left + 25 {
                        return Some(PlButton(b));
                    }
                }
            }
            None
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

    /// Whether a drag from the press on `o` moves the window: it began on the title bar.
    pub(crate) fn classic_moves_window(&self, o: ObjId) -> bool {
        self.classic_press_on(o)
            .is_some_and(|p| p.control == Control::TitleBar)
    }

    pub(crate) fn classic_pointer_down(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        let Some(control) = control_at(w, rect.width, rect.height, px, py) else {
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
            dragged: false,
        };
        self.classic_follow(&mut press, px, py);
        self.classic_press = Some(press);
        self.repaint();
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
        press.inside = control_at(w, rect.width, rect.height, px, py) == Some(press.control);
        if matches!(press.control, Control::PlRow(_)) && (py - press.from.1).abs() >= DRAG_SLOP {
            press.dragged = true;
        }
        self.classic_follow(&mut press, px, py);
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
        let over = inside && control_at(w, rect.width, rect.height, px, py) == Some(press.control);
        // a bottom button's menu is picked by the picture under the finger, above the button
        let seeks = matches!(press.control, Control::Position | Control::ShadePosition);
        let picks = matches!(press.control, Control::PlButton(_)) && press.hover.is_some();
        self.classic_press = None;
        if (over || seeks || picks) && !press.dragged {
            self.classic_act(o, w, press);
        }
        self.repaint();
    }

    pub(crate) fn classic_pointer_cancel(&mut self, o: ObjId) {
        if self.classic_press_on(o).is_some() {
            self.classic_press = None;
            self.repaint();
        }
    }

    /// A right click, or a long press: the host's menu, as the title bar's menu button opens it.
    pub(crate) fn classic_right_click(&mut self, o: ObjId) {
        self.action(o, "MENU", None, None);
    }

    /// A double tap: on a title bar it toggles the shade bar; on a playlist row it plays the song.
    pub(crate) fn classic_double_tap(&mut self, o: ObjId, px: i32, py: i32) {
        let Some(w) = self.classic_kind(o) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        match control_at(w, rect.width, rect.height, px, py) {
            Some(Control::TitleBar) => self.classic_toggle_shade(o, w),
            Some(Control::PlRow(i)) => {
                let index = self.classic_pl_scroll(rect.height) + i;
                if (index as usize) < self.player.tracks().len() {
                    self.classic_selected_row = Some(index);
                    self.player.play_track(index);
                }
            }
            _ => {}
        }
    }

    /// The sliders follow the finger while pressed; the position bar only notes where it is.
    fn classic_follow(&mut self, press: &mut ClassicPress, px: i32, py: i32) {
        match press.control {
            Control::Volume => {
                // the thumb's middle under the finger, over the 51 px the thumb travels
                let v = ((px - 107 - 7) * 255 / 51).clamp(0, 255);
                self.player.set_volume(v);
            }
            Control::Balance => {
                let b = ((px - 177 - 7 - 12) * 127 / 12).clamp(-127, 127);
                self.player.set_pan(b);
            }
            Control::Position => {
                press.position = ((px - 16 - 14) * 256 / 219).clamp(0, 255);
            }
            Control::ShadePosition => {
                press.position = ((px - 227) * 256 / 12).clamp(0, 255);
            }
            Control::ShadeVolume => {
                let v = ((px - 62) * 255 / 94).clamp(0, 255);
                self.player.set_volume(v);
            }
            Control::ShadeBalance => {
                let b = ((px - 165) * 255 / 39 - 128).clamp(-127, 127);
                self.player.set_pan(b);
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
                // the thumb's top follows the finger, 6 above it (the VM put the thumb's top at
                // 54 for a click at 60), over its 51 pixels of travel; the band's step is 0..63
                // with 0 at the top
                let top = (py - 6).clamp(38, 89);
                let step = ((top - 38) * 63 + 25) / 51;
                // a value a step either side of the middle snaps to it (Winamp does so
                // unless Shift is held, which a touch never is)
                let v = if (30..=32).contains(&step) {
                    crate::runtime::EQ_FLAT
                } else {
                    (63 - step) * 255 / 63
                };
                if n == 0 {
                    self.player.set_eq_preamp(v);
                } else {
                    self.player.set_eq_band(n as i32 - 1, v);
                }
            }
            _ => {}
        }
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
                    let ms = (i64::from(press.position) * i64::from(length) / 255) as i32;
                    self.player.seek_to(ms);
                }
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
            Control::Vis => self.classic_vis_mode = self.classic_vis_mode.next(),
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
