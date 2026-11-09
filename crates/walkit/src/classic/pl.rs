// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic playlist editor's frame, pixel for pixel as Winamp 5.666 paints it: the title bar
//! of corners, tiles and the title, the sides tiled down, and the bottom bar with its buttons,
//! the list's background between them.

use crate::classic::font;
use crate::classic::listfont::{self, ListInk};
use crate::classic::main::Painter;
use crate::classic::style::ClassicStyle;
use crate::classic::touch::Control;
use crate::draw::{Align, DrawCmd, Rect};
use crate::gui::ObjId;
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// `seconds` as Winamp writes a playlist time: m:ss, or h:mm:ss from an hour.
fn time_text(seconds: i32) -> String {
    if seconds < 3600 {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    } else {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    }
}

/// `seconds` as a row or the shade bar writes it: m:ss, the minutes unbounded ("75:00").
fn minutes_text(seconds: i32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Entry `n` (1 the first) of `count` as the list numbers it, padded with spaces to the
/// count's digits (" 1. " in a list of 10 or more).
fn numbered(n: usize, count: usize, title: &str) -> String {
    let width = count.max(1).to_string().len();
    format!("{n:>width$}. {title}")
}

/// Where the first row starts (row `i` at 22 + row height x `i`).
pub(crate) const ROW_TOP: i32 = 22;
/// In the skin's letters: the room left of a row's number and right of its length, and the
/// least between the title and the length, as Andamp's playlist (a phone's) sets them.
const LIST_LEFT: i32 = 2;
const LIST_RIGHT: i32 = 3;
const LIST_GAP: i32 = 4;

impl SkinRuntime {
    /// The playlist's font as its rows draw in it, or none for the host's font.
    fn classic_pl_font(face: &str) -> Option<crate::draw::FontDef> {
        (!face.is_empty()).then(|| crate::draw::FontDef::new(&face.to_lowercase(), ""))
    }

    /// `text` as it fits in `width` px of the playlist's font: whole, or cut on a whole letter
    /// with "..." after it.
    fn classic_pl_fitted(
        &mut self,
        text: &str,
        font: Option<&crate::draw::FontDef>,
        width: i32,
    ) -> String {
        let size = self.classic_row_font();
        let fits = |rt: &mut Self, s: &str| rt.host.text_width(s, font, size, false) <= width;
        if fits(self, text) {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let cut = |n: usize| format!("{}...", chars[..n].iter().collect::<String>().trim_end());
        // the most letters that fit with the ellipsis
        let (mut lo, mut hi) = (0, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if fits(self, &cut(mid)) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        cut(lo)
    }

    /// The playlist's shade bar (14 high): the left piece, tiles, a stretched
    /// remainder, and the right piece with the bar's buttons; active y 42, inactive 57.
    pub(crate) fn draw_classic_pl_shade(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let w = at.width;
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let y = if active { 42 } else { 57 };
        let mut p = Painter { at, out };
        p.blit("pledit.bmp", 72, y, 25, 14, 0, 0);
        let mut x = 25;
        for _ in 1..(w - 50) / 25 {
            p.blit("pledit.bmp", 72, 57, 25, 14, x, 0);
            x += 25;
        }
        let rest = (w - 50) % 25;
        if rest != 0 {
            p.stretch("pledit.bmp", 72, 57, 24, 14, x, 0, rest, 14);
            x += rest;
        }
        p.blit("pledit.bmp", 99, y, 50, 14, x, 0);
        // the song's line in text.bmp cells (with the bitmap font): "n. TITLE" from
        // x 4 at y 4, its length ending at w-29, blank cells between
        let index = self.player.track_index();
        let (title, length) = usize::try_from(index)
            .ok()
            .and_then(|i| self.player.tracks().into_iter().nth(i))
            .map_or((String::new(), 0), |t| (t.title, t.length_ms / 1000));
        if self.classic_style == ClassicStyle::Winamp291 {
            let count = self.player.tracks().len();
            let line = numbered(index as usize + 1, count, &title);
            Self::shade_line_291(&mut p, w, &line, length);
            return;
        }
        let time = minutes_text(length);
        let time_cells = time.chars().count() as i32;
        let room = (w - 29 - 4) / 5 - time_cells - 1;
        let mut x = 4;
        let line: Vec<char> = numbered(index as usize + 1, self.player.tracks().len(), &title)
            .chars()
            .collect();
        let room = room.max(0) as usize;
        for (i, &c) in line.iter().take(room).enumerate() {
            // a title cut short ends on the ellipsis cell
            let c = if i + 1 == room && line.len() > room {
                '\u{1}'
            } else {
                c
            };
            let (cx, cy) = font::cell(c);
            p.blit("text.bmp", cx, cy, 5, 6, x, 4);
            x += 5;
        }
        // blank cells on the grid up to where the length starts: the first cell at or past
        // w-29 less the length's cells (confirmed on the VM: the length sits on the grid)
        let time_x = w - 29 - time_cells * 5;
        while x < time_x {
            let (cx, cy) = font::cell(' ');
            p.blit("text.bmp", cx, cy, 5, 6, x, 4);
            x += 5;
        }
        for c in time.chars() {
            let (cx, cy) = font::cell(c);
            p.blit("text.bmp", cx, cy, 5, 6, x, 4);
            x += 5;
        }
        if x < w - 30 {
            let (cx, cy) = font::cell(' ');
            p.blit("text.bmp", cx, cy, (w - 30 - x).min(5), 6, x, 4);
        }
    }

    /// 2.91's song line, in cells: `(w - 40) / 5` cells from x 5 less the
    /// length's and one, a cut title ending on the ellipsis; when the title fits, blanks fill
    /// those cells and one more; then a blank, and "m:ss" from `w - 29` less its cells.
    fn shade_line_291(p: &mut Painter, w: i32, line: &str, secs: i32) {
        let time = format!("{}:{:02}", secs / 60, secs % 60);
        let time_cells = time.chars().count() as i32;
        let cells = ((w - 40) / 5 - time_cells - 1).max(0) as usize;
        let chars: Vec<char> = line.chars().collect();
        let cell = |p: &mut Painter, c: char, x: i32| {
            let (cx, cy) = font::cell(c);
            p.blit("text.bmp", cx, cy, 5, 6, x, 4);
        };
        let mut x = 5;
        for (i, &c) in chars.iter().take(cells).enumerate() {
            let cut = i + 1 == cells && i + 1 < chars.len();
            cell(p, if cut { '\u{1}' } else { c }, x);
            x += 5;
        }
        if chars.len() <= cells {
            for _ in chars.len()..=cells {
                cell(p, ' ', x);
                x += 5;
            }
        }
        cell(p, ' ', x);
        x = w - 29 - time_cells * 5;
        for c in time.chars() {
            cell(p, c, x);
            x += 5;
        }
        if x < w - 30 {
            let (cx, cy) = font::cell(' ');
            p.blit("text.bmp", cx, cy, w - 30 - x, 6, x, 4);
        }
    }

    /// The rows a playlist window `h` tall shows : `(h - 60) / row`.
    pub(crate) fn classic_pl_rows(&self, h: i32) -> i32 {
        ((h - 60) / self.classic_row()).max(0)
    }

    /// A playlist row's height: the font's full height, Arial's 5/4 of the em
    /// (15 at Winamp's 12, where 3 rows fill a playlist 116 tall), and what the host adds
    /// (`Presentation::list_row_extra`).
    pub(crate) fn classic_row(&self) -> i32 {
        (self.classic_list_font * 5 + 2) / 4 + self.list_row_extra
    }

    /// The playlist font's size as the host reads it, in points: 4/3 of the em in pixels.
    pub(crate) fn classic_row_font(&self) -> i32 {
        self.classic_list_font * 4 / 3
    }

    /// Sets the playlist font's size, pixels to the em, as Winamp's "playlist font size"
    /// preference does (`pe_fontsize`, the em in pixels); its rows grow and shrink with it.
    pub fn set_classic_list_font_size(&mut self, px: i32) {
        let px = px.clamp(6, 40);
        if px != self.classic_list_font {
            self.classic_list_font = px;
            self.repaint();
        }
    }

    /// The first row shown, clamped to the rows that fit under the list's end.
    pub(crate) fn classic_pl_scroll(&self, h: i32) -> i32 {
        let past = self.player.tracks().len() as i32 - self.classic_pl_rows(h);
        self.classic_pl_scroll.clamp(0, past.max(0))
    }

    /// How far down the track the thumb sits for `scroll` of `count` rows: the track is
    /// `h - 76` long past the thumb, shared out over the rows past the window.
    pub(crate) fn classic_pl_thumb(&self, h: i32, count: i32, scroll: i32) -> i32 {
        let past = count - self.classic_pl_rows(h);
        if past <= 0 {
            return 0;
        }
        ((h - 76) * scroll.min(past) / past).max(0)
    }

    /// The ink of the skin's `text.bmp` for the playlist's rows, read from the host once.
    fn classic_list_ink(&mut self) -> Option<std::rc::Rc<ListInk>> {
        if !self.classic_list_ink_read {
            self.classic_list_ink_read = true;
            self.classic_list_ink = self
                .image_size("text.bmp")
                .and_then(|(w, h)| {
                    let px = self.host.pixels("text.bmp", 0, 0, w, h)?;
                    ListInk::of(&px, w, h)
                })
                .map(std::rc::Rc::new);
        }
        self.classic_list_ink.clone()
    }

    /// One row in the skin's letters, its letters' tops at `top` of `block` (`width` wide):
    /// "n. " and the title from the left, the length against the right, a title too long for
    /// the room between ending on the ellipsis.
    #[allow(clippy::too_many_arguments)]
    fn letter_row(
        ink: &ListInk,
        block: &mut [u32],
        width: i32,
        top: i32,
        number: usize,
        title: &str,
        time: &str,
        rgb: u32,
    ) {
        let number = format!("{number}. ");
        ink.write(&number, (LIST_LEFT, top), rgb, block, width, width);
        let left = LIST_LEFT + number.chars().count() as i32 * listfont::CELL_W;
        let time_x = width - LIST_RIGHT - time.chars().count() as i32 * listfont::CELL_W;
        ink.write(time, (time_x, top), rgb, block, width, width);
        let room = (time_x - LIST_GAP - left) / listfont::CELL_W;
        ink.write(
            &listfont::fitted(title, room),
            (left, top),
            rgb,
            block,
            width,
            time_x,
        );
    }

    pub(crate) fn draw_classic_pl(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let (w, h) = (at.width, at.height);
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let style = self.classic_pledit.clone();
        let tracks = self.player.tracks();
        let current = self.player.track_index();
        let mut p = Painter { at, out };
        // the list's background, under the frame, and the rows on it: "n. Title" with the
        // length at the right, the current song in the current color
        p.fill(12, 20, w - 32, h - 58, style.normal_bg);
        let rows = self.classic_pl_rows(h);
        let row = self.classic_row();
        let font_size = self.classic_row_font();
        // in the skin's letters, the rows go into one block of pixels over the list
        let ink = if self.skin_list_font {
            self.classic_list_ink()
        } else {
            None
        };
        let block_w = w - 32;
        let mut block = ink
            .as_ref()
            .map(|_| vec![0u32; (block_w.max(0) * rows.max(0) * row) as usize]);
        let mut lettered = false;
        let selected = self.classic_selected_row;
        let scroll = self.classic_pl_scroll(h).max(0) as usize;
        for (i, t) in tracks
            .iter()
            .enumerate()
            .skip(scroll)
            .take(rows.max(0) as usize)
        {
            let y = ROW_TOP + (i - scroll) as i32 * row;
            if selected == Some(i as i32) {
                p.fill(12, y, w - 32, row, style.selected_bg);
            }
            let color = if i as i32 == current {
                style.current
            } else {
                style.normal
            };
            let time = time_text(t.length_ms / 1000);
            if let (Some(ink), Some(block)) = (ink.as_deref(), block.as_mut()) {
                if listfont::covers(&t.title) {
                    let top = (i - scroll) as i32 * row + (row - listfont::CELL_H) / 2;
                    Self::letter_row(ink, block, block_w, top, i + 1, &t.title, &time, color.0);
                    lettered = true;
                    continue;
                }
            }
            // " m:ss " right-aligned at the row's end, and the title up to it, cut short on "..."
            let face = Some(&style.font);
            let def = Self::classic_pl_font(&style.font);
            let time = format!(" {} ", minutes_text(t.length_ms / 1000));
            let time_w = self.host.text_width(&time, def.as_ref(), font_size, false);
            let line = numbered(i + 1, tracks.len(), &t.title);
            let title = self.classic_pl_fitted(&line, def.as_ref(), block_w - time_w);
            p.text(
                &title,
                face,
                font_size,
                (12, y, w - 32, row),
                color,
                Align::Left,
            );
            p.text(
                &time,
                face,
                font_size,
                (12, y, w - 32, row),
                color,
                Align::Right,
            );
        }
        if let Some(block) = block.filter(|_| lettered) {
            p.out.push(DrawCmd::Pixels {
                rect: Rect::new(at.left + 12, at.top + ROW_TOP, block_w, rows * row),
                w: block_w,
                h: rows * row,
                argb: std::rc::Rc::from(block.into_boxed_slice()),
                alpha: 255,
                stable: false,
            });
        }
        // the title bar: the left corner, tiles either side of the title, the right corner
        let y1 = if active { 0 } else { 21 };
        p.blit("pledit.bmp", 0, y1, 25, 20, 0, 0);
        let tiles = (w - 150) / 25;
        let mut x = 25;
        if tiles != 0 {
            if tiles % 2 != 0 {
                p.blit("pledit.bmp", 127, y1, 12, 20, x, 0);
                x += 12;
            }
            for _ in 0..tiles / 2 {
                p.blit("pledit.bmp", 127, y1, 25, 20, x, 0);
                x += 25;
            }
        }
        p.blit("pledit.bmp", 26, y1, 100, 20, x, 0);
        x += 100;
        if tiles != 0 {
            if tiles % 2 != 0 {
                p.blit("pledit.bmp", 127, y1, 13, 20, x, 0);
                x += 13;
            }
            for _ in 0..tiles / 2 {
                p.blit("pledit.bmp", 127, y1, 25, 20, x, 0);
                x += 25;
            }
        }
        let rest = (w - 150) % 25;
        if rest != 0 {
            p.stretch("pledit.bmp", 127, y1, 25, 20, x, 0, rest, 20);
            x += rest;
        }
        p.blit("pledit.bmp", 153, y1, 25, 20, x, 0);
        // the sides, 29 rows at a time, the last run stretched, the scrollbar's track between
        // the right side's two pieces
        let mut y = 20;
        for _ in 0..(h - 58) / 29 {
            p.blit("pledit.bmp", 0, 42, 12, 29, 0, y);
            p.blit("pledit.bmp", 31, 42, 5, 29, w - 20, y);
            p.blit("pledit.bmp", 44, 42, 7, 29, w - 7, y);
            p.blit("pledit.bmp", 36, 42, 8, 29, w - 15, y);
            y += 29;
        }
        let rest = (h - 58) % 29;
        if rest != 0 {
            p.stretch("pledit.bmp", 0, 42, 12, 29, 0, y, 12, rest);
            p.stretch("pledit.bmp", 31, 42, 5, 29, w - 20, y, 5, rest);
            p.stretch("pledit.bmp", 44, 42, 7, 29, w - 7, y, 7, rest);
            p.blit("pledit.bmp", 36, 42, 8, rest, w - 15, y);
            y += rest;
        }
        // the scrollbar's thumb: down the track by the scroll's share of the
        // rows past the window, pressed while a finger holds it
        let thumb = self.classic_pl_thumb(h, tracks.len() as i32, scroll as i32);
        let held = self
            .classic_press_on(o)
            .is_some_and(|p| p.control == Control::PlScroll);
        p.blit(
            "pledit.bmp",
            if held { 61 } else { 52 },
            53,
            8,
            18,
            w - 15,
            20 + thumb,
        );
        // the bottom: the left part with its buttons, tiles, the visualizer part when there is
        // room for it, and the right part
        p.blit("pledit.bmp", 0, 72, 125, 38, 0, y);
        let mut tiles = (w - 275) / 25;
        let vis = tiles > 2;
        if vis {
            tiles -= 3;
        }
        let mut x = 125;
        for _ in 0..tiles {
            p.blit("pledit.bmp", 179, 0, 25, 38, x, y);
            x += 25;
        }
        let rest = (w - 275) % 25;
        if rest != 0 {
            p.stretch("pledit.bmp", 179, 0, 25, 38, x, y, rest, 38);
            x += rest;
        }
        if vis {
            p.blit("pledit.bmp", 205, 0, 75, 38, x, y);
            x += 75;
        }
        p.blit("pledit.bmp", 126, 72, 150, 38, x, y);
        // a bottom button held, or its menu a tap left open: the pictures stacked over it, the one under the finger
        // from the pressed column, the bar beside them
        let held = self
            .classic_press_on(o)
            .and_then(|p| match p.control {
                Control::PlButton(b) => Some((b, p.hover)),
                _ => None,
            })
            .or(self
                .classic_open_menu
                .filter(|(menu_on, _)| *menu_on == o)
                .map(|(_, b)| (b, None)));
        if let Some((b, hover)) = held {
            let left = b.menu_left(w);
            let items = b.items();
            let top = h - 12 - 18 * items;
            p.blit("pledit.bmp", b.bar_source(), 111, 3, 18 * items, left, top);
            for i in 0..items {
                let sy = match i {
                    3 => 168,
                    2 => 111,
                    1 => 130,
                    _ => 149,
                };
                let sx = b.picture_source() + if hover == Some(i) { 23 } else { 0 };
                p.blit("pledit.bmp", sx, sy, 22, 18, left + 3, h - 30 - 18 * i);
            }
        }
        // the bottom bar's two lines in text.bmp cells: the
        // selected songs' time over the list's, 18 cells from (w-143, h-28); and the song's
        // time at (w-86, h-15), blank while nothing plays
        let total: i32 = tracks.iter().map(|t| t.length_ms / 1000).sum();
        let picked: i32 = selected
            .and_then(|i| tracks.get(i.max(0) as usize))
            .map_or(0, |t| t.length_ms / 1000);
        let line = format!("{}/{}", time_text(picked), time_text(total));
        let mut x = w - 143;
        for c in line.chars().chain(std::iter::repeat(' ')).take(18) {
            let (cx, cy) = font::cell(c);
            p.blit("text.bmp", cx, cy, 5, 6, x, h - 28);
            x += 5;
        }
        let y = h - 15;
        if self.player.status() == Status::Stopped {
            p.blit("text.bmp", 142, 0, 4, 6, w - 86, y);
            for x in [w - 82, w - 77, w - 72, w - 64, w - 59] {
                p.blit("text.bmp", 142, 0, 5, 6, x, y);
            }
        } else {
            // the time played, or the time left with a minus before it (the minus drawn as 3
            // columns of text.bmp's "-" where the blank goes)
            let length = self.player.length_ms();
            let remaining = self.classic_remaining && length > 0;
            let ms = if remaining {
                (length - self.player.position_ms()).max(0)
            } else {
                self.player.position_ms().max(0)
            };
            let seconds = ms / 1000;
            let (minutes, seconds) = (seconds / 60, seconds % 60);
            let digit =
                |p: &mut Painter, n: i32, x: i32| p.blit("text.bmp", (n % 10) * 5, 6, 5, 6, x, y);
            let (sx, sy) = if remaining { (75, 6) } else { (142, 0) };
            if minutes / 100 == 0 {
                p.blit("text.bmp", 142, 0, 4, 6, w - 86, y);
                p.blit("text.bmp", sx, sy, 3, 6, w - 82, y);
            } else {
                p.blit("text.bmp", sx, sy, 3, 6, w - 86, y);
                digit(&mut p, minutes / 100, w - 82);
            }
            digit(&mut p, minutes / 10, w - 77);
            digit(&mut p, minutes, w - 72);
            digit(&mut p, seconds / 10, w - 64);
            digit(&mut p, seconds, w - 59);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::classic::main::Painter;
    use crate::draw::{DrawCmd, Rect};
    use crate::runtime::SkinRuntime;

    #[test]
    fn the_2_91_shade_line_starts_at_5_and_leaves_a_column_before_the_length() {
        let mut out = Vec::new();
        let mut p = Painter {
            at: Rect::new(0, 0, 275, 14),
            out: &mut out,
        };
        SkinRuntime::shade_line_291(&mut p, 275, "1. Andamp", 600);
        let xs: Vec<i32> = out
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Image { dst, .. } => Some(dst.left),
                _ => None,
            })
            .collect();
        // 41 cells of title and blanks, one more blank, then "10:00" from 275 - 29 - 25
        let mut want: Vec<i32> = (0..43).map(|i| 5 + 5 * i).collect();
        want.extend([221, 226, 231, 236, 241]);
        assert_eq!(xs, want);
    }

    #[test]
    fn a_cut_2_91_shade_line_ends_on_the_ellipsis() {
        let mut out = Vec::new();
        let mut p = Painter {
            at: Rect::new(0, 0, 275, 14),
            out: &mut out,
        };
        SkinRuntime::shade_line_291(&mut p, 275, &"x".repeat(60), 61);
        let cells: Vec<(i32, i32)> = out
            .iter()
            .filter_map(|c| match c {
                DrawCmd::Image { bitmap, dst, .. } => Some((dst.left, bitmap.x)),
                _ => None,
            })
            .collect();
        // 47 cells less "1:01" and one leave 42: the last of them is the ellipsis
        assert_eq!(cells[41], (210, crate::classic::font::cell('\u{1}').0));
        assert_eq!(cells[42].0, 215, "the blank after it");
        assert_eq!(cells[43].0, 275 - 29 - 20, "the length");
    }
}
