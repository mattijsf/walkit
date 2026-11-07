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
    /// The clutter bar's letters.
    Clutter,
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
    /// The playlist's button row.
    PlButtons,
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
            let boxes: [(Control, i32, i32, i32, i32); 20] = [
                // the title buttons answer a row below their pictures and, at the right, a
                // column beyond; the VM showed minimize reaching x 253 and the equalizer's
                // shade and close boxes at x 253..=263 and 264..=273, y 3..=12
                (Menu, 6, 3, 9, 10),
                (Minimize, 244, 3, 10, 10),
                (Shade, 254, 3, 10, 10),
                (Close, 264, 3, 10, 10),
                (Clutter, 10, 22, 8, 43),
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
            if hit(x, y, width - 15, 20, 8, height - 58) {
                return Some(PlScroll);
            }
            if hit(x, y, 12, 20, width - 32, height - 58) {
                return Some(PlRow((y - 20) / 14));
            }
            if y >= height - 38 {
                return Some(PlButtons);
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
        let seeks = matches!(press.control, Control::Position | Control::ShadePosition);
        self.classic_press = None;
        if (over || seeks) && !press.dragged {
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
                let v = (63 - step) * 255 / 63;
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
