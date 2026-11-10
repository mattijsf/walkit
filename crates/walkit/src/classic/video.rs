// SPDX-License-Identifier: GPL-3.0-or-later
//! The video window of a classic skin, framed with `video.bmp`. Drawn as Winamp 5.666 draws
//! it: the title bar, the sides and the bottom bar, the buttons, the close button, the status
//! line.

use super::main::Painter;
use super::touch::Control;
use crate::draw::{DrawCmd, Rect, Rgb};
use crate::gui::ObjId;
use crate::runtime::SkinRuntime;

/// The sheet the frame comes from.
pub(crate) const SHEET: &str = "video.bmp";
/// The title bar's height, the sides' widths and the bottom bar's height.
pub(crate) const TITLE_H: i32 = 20;
pub(crate) const LEFT: i32 = 11;
pub(crate) const RIGHT: i32 = 8;
pub(crate) const BOTTOM: i32 = 38;
/// The bottom bar's buttons: five of 15 x 18 from x 9 at y h-29 (full screen, 1x, 2x, TV,
/// the menu); a press is taken from x 9 to 89 and y h-29 to h-11.
pub(crate) const BUTTONS_X: i32 = 9;
pub(crate) const BUTTON_W: i32 = 15;
pub(crate) const BUTTON_H: i32 = 18;
pub(crate) const BUTTONS: i32 = 5;
/// The actions of the buttons, in order.
pub(crate) const ACTIONS: [&str; 5] = ["VID_FS", "VID_1X", "VID_2X", "VID_TV", "VID_MISC"];

impl SkinRuntime {
    /// Draws video window object `o` at `at`.
    pub(crate) fn draw_classic_video(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let (w, h) = (at.width, at.height);
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let press = self.classic_press_on(o);
        let mut p = Painter {
            at,
            out,
            sheets: self.classic_sheets.clone(),
        };
        p.fill(
            LEFT,
            TITLE_H,
            w - LEFT - RIGHT,
            h - TITLE_H - BOTTOM,
            Rgb(0),
        );
        video_title(&mut p, w, active);
        video_frame(&mut p, w, h);
        match press.map(|p| (p.control, p.inside)) {
            // held, close shows (148,42)
            Some((Control::Close, true)) => p.blit(SHEET, 148, 42, 9, 9, w - 11, 3),
            Some((Control::VideoButton(i), true)) if (0..BUTTONS).contains(&i) => {
                let x = BUTTONS_X + i * BUTTON_W;
                p.blit(SHEET, x + 149, 42, BUTTON_W, BUTTON_H, x, h - 29);
            }
            _ => {}
        }
    }

    /// The line of video window `container` where the host writes what plays, in the window,
    /// and its colors from `pledit.txt` (`mbFG` on `mbBG`). `None` without a video window.
    pub fn classic_status_line(&mut self, container: &str) -> Option<(Rect, Rgb, Rgb)> {
        let c = self.container(container)?;
        let l = self.containers[c].current?;
        let o = self.all_objects.iter().copied().find(|&o| {
            matches!(
                self.objects[o].kind,
                crate::gui::Kind::ClassicWindow(super::ClassicWindow::Video)
            ) && self.layout_object(o) == Some(l)
        })?;
        let r = self.objects[o].base.rect;
        let style = &self.classic_pledit;
        Some((
            Rect::new(r.left + 92, r.top + r.height - 27, r.width - 25 - 92, 14),
            style.mb_fg,
            style.mb_bg,
        ))
    }
}

/// The title bar: corners, fills and the title picture, 100 wide, between.
fn video_title(p: &mut Painter, w: i32, active: bool) {
    let row = if active { 0 } else { 21 };
    let (fills, rest) = ((w - 150) / 25, (w - 150) % 25);
    p.blit(SHEET, 0, row, 25, 20, 0, 0);
    let mut x = 25;
    if fills % 2 == 1 {
        p.blit(SHEET, 127, row, 12, 20, x, 0);
        x += 12;
    }
    for _ in 0..fills / 2 {
        p.blit(SHEET, 127, row, 25, 20, x, 0);
        x += 25;
    }
    p.blit(SHEET, 26, row, 100, 20, x, 0);
    x += 100;
    if fills % 2 == 1 {
        p.blit(SHEET, 127, row, 13, 20, x, 0);
        x += 13;
    }
    for _ in 0..fills / 2 {
        p.blit(SHEET, 127, row, 25, 20, x, 0);
        x += 25;
    }
    if rest > 0 {
        p.stretch(SHEET, 127, row, 25, 20, x, 0, rest, 20);
        x += rest;
    }
    p.blit(SHEET, 153, row, 25, 20, x, 0);
}

/// The sides and the bottom bar with its buttons.
fn video_frame(p: &mut Painter, w: i32, h: i32) {
    let mut y = TITLE_H;
    let side = h - 58;
    for _ in 0..side / 29 {
        p.blit(SHEET, 127, 42, LEFT, 29, 0, y);
        p.blit(SHEET, 139, 42, RIGHT, 29, w - RIGHT, y);
        y += 29;
    }
    let rest = side % 29;
    if rest > 0 {
        p.stretch(SHEET, 127, 42, LEFT, 29, 0, y, LEFT, rest);
        p.stretch(SHEET, 139, 42, RIGHT, 29, w - RIGHT, y, RIGHT, rest);
        y += rest;
    }
    p.blit(SHEET, 0, 42, 125, BOTTOM, 0, y);
    let mut x = 125;
    for _ in 0..(w - 250) / 25 {
        p.blit(SHEET, 127, 81, 25, BOTTOM, x, y);
        x += 25;
    }
    let rest = (w - 250) % 25;
    if rest > 0 {
        p.stretch(SHEET, 127, 81, 25, BOTTOM, x, y, rest, BOTTOM);
        x += rest;
    }
    p.blit(SHEET, 0, 81, 125, BOTTOM, x, y);
}
