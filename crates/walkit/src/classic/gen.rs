// SPDX-License-Identifier: GPL-3.0-or-later
//! The generic window of a classic skin, framed with `gen.bmp`: Winamp's media library and the
//! windows plug-ins embed. Drawn as Winamp 5.666 draws its embed window: the frame, the title
//! bar, the close button and the font.

use super::main::Painter;
use crate::draw::{DrawCmd, Rect, Rgb};
use crate::gui::ObjId;
use crate::runtime::SkinRuntime;

/// The sheet the frame and its font come from.
pub(crate) const SHEET: &str = "gen.bmp";
/// The title bar's height.
pub(crate) const TITLE_H: i32 = 20;
/// The frame's left side, right side and bottom bar.
pub(crate) const LEFT: i32 = 11;
pub(crate) const RIGHT: i32 = 8;
pub(crate) const BOTTOM: i32 = 14;
/// The most characters of a window's text the title shows.
const TITLE_CHARS: usize = 30;
/// The gap a character without a glyph leaves.
const GAP: i32 = 5;

/// Where each glyph of `gen.bmp`'s font sits: x and width in its row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GenFont {
    /// A to Z, on rows 88 (active) and 96 (inactive).
    pub(crate) letters: Vec<(i32, i32)>,
    /// 0 to 9, '-' and ':', on rows 72 and 80.
    pub(crate) digits: Vec<(i32, i32)>,
}

/// The glyphs along one scan row of the sheet: the color of its first pixel parts them, and a
/// glyph that the row's end cuts off has no width.
pub(crate) fn scan(row: &[u32], count: usize) -> Vec<(i32, i32)> {
    let Some(&sep) = row.first() else {
        return vec![(0, 0); count];
    };
    let mut out = Vec::with_capacity(count);
    let mut x = 0usize;
    for _ in 0..count {
        while x < row.len() && row[x] == sep {
            x += 1;
        }
        let start = x as i32;
        let mut width = 0;
        let mut ended = false;
        while x < row.len() {
            let c = row[x];
            x += 1;
            if c == sep {
                ended = true;
                break;
            }
            width += 1;
        }
        out.push((start, if ended { width } else { 0 }));
    }
    out.resize(count, (0, 0));
    out
}

impl GenFont {
    /// The glyph for `c`: its x, width and the row it sits on (active), or `None` for a gap.
    fn glyph(&self, c: char, digits: bool) -> Option<(i32, i32, i32)> {
        let c = c.to_ascii_uppercase();
        let (x, w, row) = if c.is_ascii_uppercase() {
            let (x, w) = self.letters.get((c as u8 - b'A') as usize).copied()?;
            (x, w, 88)
        } else if digits {
            let i = match c {
                '0'..='9' => (c as u8 - b'0') as usize,
                '-' => 10,
                ':' => 11,
                _ => return None,
            };
            let (x, w) = self.digits.get(i).copied()?;
            (x, w, 72)
        } else {
            return None;
        };
        (w > 0).then_some((x, w, row))
    }

    /// The width of `text` in this font.
    pub(crate) fn width(&self, text: &str, digits: bool) -> i32 {
        text.chars()
            .map(|c| self.glyph(c, digits).map_or(GAP, |g| g.1))
            .sum()
    }
}

/// `v` rounded up to whole 25-pixel tiles.
fn tiles_up(v: i32) -> i32 {
    (v + 24) - (v + 24) % 25
}

impl SkinRuntime {
    /// `gen.bmp`'s font, read from its scan rows once.
    pub(crate) fn classic_gen_font(&mut self) -> std::rc::Rc<GenFont> {
        if let Some(f) = &self.classic_gen_font {
            return f.clone();
        }
        let width = self.image_size(SHEET).map_or(0, |s| s.0);
        let letters = self.host.pixels(SHEET, 0, 90, width, 1);
        let digits = self.host.pixels(SHEET, 0, 74, width, 1);
        let font = std::rc::Rc::new(GenFont {
            letters: letters
                .as_deref()
                .map_or_else(|| vec![(0, 0); 26], |px| scan(px, 26)),
            digits: digits
                .as_deref()
                .map_or_else(|| vec![(0, 0); 12], |px| scan(px, 12)),
        });
        // a sheet the host cannot read yet is read again next time
        if width == 0 || letters.is_none() {
            return font;
        }
        self.classic_gen_font = Some(font.clone());
        font
    }

    /// The title a gen window shows: its container's name.
    fn classic_gen_title(&self, o: ObjId) -> String {
        self.layout_object(o)
            .and_then(|l| self.objects[l].layout().and_then(|d| d.container))
            .map(|c| self.containers[c].name.clone())
            .unwrap_or_default()
    }

    /// Draws gen window object `o` at `at`.
    pub(crate) fn draw_classic_gen(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let (w, h) = (at.width, at.height);
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let pressed = self
            .classic_press_on(o)
            .is_some_and(|p| p.control == super::touch::Control::Close && p.inside);
        let font = self.classic_gen_font();
        let digits = self.classic_pledit.use_gen_nums && self.classic_style.gen_digits();
        let title: String = self
            .classic_gen_title(o)
            .chars()
            .take(TITLE_CHARS)
            .collect();
        let mut p = Painter { at, out };
        // the client area, black under what the window holds
        p.fill(
            LEFT,
            TITLE_H,
            w - LEFT - RIGHT,
            h - TITLE_H - BOTTOM,
            Rgb(0),
        );
        gen_title(&mut p, &font, &title, digits, w, active);
        // the close button is part of the title's right corner; held, it shows (148,42)
        if pressed {
            p.blit(SHEET, 148, 42, 9, 9, w - 11, 3);
        }
        // every gen window here sizes, so it shows the grip
        gen_frame(&mut p, w, h, true);
    }

    /// The area of gen window `container` the host fills (the media library's list, a
    /// plug-in's picture), in the window: inside the frame, x 11..w-8 and y 20..h-14. `None`
    /// when the container holds no gen window.
    pub fn classic_client_rect(&mut self, container: &str) -> Option<Rect> {
        let c = self.container(container)?;
        let l = self.containers[c].current?;
        let gen = self.all_objects.iter().copied().find(|&o| {
            matches!(
                self.objects[o].kind,
                crate::gui::Kind::ClassicWindow(super::ClassicWindow::Gen)
            ) && self.layout_object(o) == Some(l)
        })?;
        // the object's rect is in its window
        let r = self.objects[gen].base.rect;
        Some(Rect::new(
            r.left + LEFT,
            r.top + TITLE_H,
            r.width - LEFT - RIGHT,
            r.height - TITLE_H - BOTTOM,
        ))
    }
}

/// The title bar: corners, fills, the plate under the text, the text.
fn gen_title(p: &mut Painter, font: &GenFont, title: &str, digits: bool, w: i32, active: bool) {
    let row = if active { 0 } else { 21 };
    let text_w = font.width(title, digits);
    let plate = tiles_up(text_w).min(w - 100);
    let spare = w - plate - 100;
    let (fills, rest) = (spare / 25, spare % 25);
    p.blit(SHEET, 0, row, 25, 20, 0, 0);
    let mut x = 25;
    if fills % 2 == 1 {
        p.blit(SHEET, 104, row, 12, 20, x, 0);
        x += 12;
    }
    for _ in 0..fills / 2 {
        p.blit(SHEET, 104, row, 25, 20, x, 0);
        x += 25;
    }
    p.blit(SHEET, 26, row, 25, 20, x, 0);
    x += 25;
    let tiles = plate / 25;
    if tiles > 0 {
        let mut text_x = (plate - text_w) / 2 + x;
        if plate != text_w {
            text_x += 1;
        }
        for i in 0..tiles {
            p.blit(SHEET, 52, row, 25, 20, x + i * 25, 0);
        }
        gen_text(p, font, title, digits, text_x, plate, active);
        x += tiles * 25;
    }
    p.blit(SHEET, 78, row, 25, 20, x, 0);
    x += 25;
    if fills % 2 == 1 {
        p.blit(SHEET, 104, row, 13, 20, x, 0);
        x += 13;
    }
    for _ in 0..fills / 2 {
        p.blit(SHEET, 104, row, 25, 20, x, 0);
        x += 25;
    }
    if rest > 0 {
        p.stretch(SHEET, 104, row, 25, 20, x, 0, rest, 20);
        x += rest;
    }
    p.blit(SHEET, 130, row, 25, 20, x, 0);
}

/// The title's glyphs from `x` at y 4, while they fit in `room`.
fn gen_text(
    p: &mut Painter,
    font: &GenFont,
    text: &str,
    digits: bool,
    mut x: i32,
    mut room: i32,
    active: bool,
) {
    let shift = if active { 0 } else { 8 };
    for c in text.chars() {
        let advance = match font.glyph(c, digits) {
            Some((sx, gw, sy)) => {
                if room < gw {
                    return;
                }
                p.blit(SHEET, sx, sy + shift, gw, 7, x, 4);
                gw
            }
            None => GAP,
        };
        x += advance;
        room -= advance;
    }
}

/// The sides, the bottom corners, the bottom bar and the grip.
fn gen_frame(p: &mut Painter, w: i32, h: i32, sizes: bool) {
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
    p.blit(SHEET, 158, 42, LEFT, 24, 0, y);
    p.blit(SHEET, 170, 42, RIGHT, 24, w - RIGHT, y);
    let bar = y + 24;
    p.blit(SHEET, 0, 42, 125, BOTTOM, 0, bar);
    let mut x = 125;
    for _ in 0..(w - 250) / 25 {
        p.blit(SHEET, 127, 72, 25, BOTTOM, x, bar);
        x += 25;
    }
    let rest = (w - 250) % 25;
    if rest > 0 {
        p.stretch(SHEET, 127, 72, 25, BOTTOM, x, bar, rest, BOTTOM);
        x += rest;
    }
    p.blit(SHEET, 0, 57, 125, BOTTOM, x, bar);
    if sizes {
        p.blit(SHEET, 118, 72, 7, 7, x + 112, y + 26);
    }
}

#[cfg(test)]
mod tests {
    use super::scan;

    #[test]
    fn the_font_row_parts_glyphs_by_its_first_pixel() {
        // sep 0, glyphs of 2 and 3 pixels, the last cut by the row's end
        let row = [0, 5, 5, 0, 0, 7, 7, 7, 0, 9];
        assert_eq!(scan(&row, 3), vec![(1, 2), (5, 3), (9, 0)]);
    }
}
