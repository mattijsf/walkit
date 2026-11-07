// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic playlist editor's frame, pixel for pixel as Winamp 5.666 paints it:
//! the title bar of corners, tiles and the title, the sides
//! tiled down, and the bottom bar with its buttons, the list's background between them.

use crate::classic::font;
use crate::classic::main::Painter;
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

/// The playlist font's size, as Winamp's "playlist font size" default (`pe_fontsize` 12) has it.
pub(super) const ROW_FONT: i32 = 12;
/// A row's height: the font's `tmHeight`, 15 for Arial at 12 in the reference
/// VM, where 3 rows fill a playlist 116 tall.
pub(crate) const ROW_HEIGHT: i32 = 15;
/// Where the first row starts (row `i` at 22 + row height x `i`).
pub(crate) const ROW_TOP: i32 = 22;

impl SkinRuntime {
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
        let time = time_text(length);
        let time_cells = time.chars().count() as i32;
        let room = (w - 29 - 4) / 5 - time_cells - 1;
        let mut x = 4;
        let line = format!("{}. {}", index + 1, title);
        for c in line.chars().take(room.max(0) as usize) {
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

    /// The rows a playlist window `h` tall shows: `(h - 60) / row`.
    pub(crate) fn classic_pl_rows(h: i32) -> i32 {
        ((h - 60) / ROW_HEIGHT).max(0)
    }

    /// The first row shown, clamped to the rows that fit under the list's end.
    pub(crate) fn classic_pl_scroll(&self, h: i32) -> i32 {
        let past = self.player.tracks().len() as i32 - Self::classic_pl_rows(h);
        self.classic_pl_scroll.clamp(0, past.max(0))
    }

    /// How far down the track the thumb sits for `scroll` of `count` rows: the track is
    /// `h - 76` long past the thumb, shared out over the rows past the window.
    pub(crate) fn classic_pl_thumb(h: i32, count: i32, scroll: i32) -> i32 {
        let past = count - Self::classic_pl_rows(h);
        if past <= 0 {
            return 0;
        }
        ((h - 76) * scroll.min(past) / past).max(0)
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
        let rows = Self::classic_pl_rows(h);
        let selected = self.classic_selected_row;
        let scroll = self.classic_pl_scroll(h).max(0) as usize;
        for (i, t) in tracks
            .iter()
            .enumerate()
            .skip(scroll)
            .take(rows.max(0) as usize)
        {
            let y = ROW_TOP + (i - scroll) as i32 * ROW_HEIGHT;
            if selected == Some(i as i32) {
                p.fill(12, y, w - 32, ROW_HEIGHT, style.selected_bg);
            }
            let color = if i as i32 == current {
                style.current
            } else {
                style.normal
            };
            let time = time_text(t.length_ms / 1000);
            let font = Some(&style.font);
            p.text(
                &format!("{}. {}", i + 1, t.title),
                font,
                (12, y, w - 32, ROW_HEIGHT),
                color,
                Align::Left,
            );
            p.text(
                &time,
                font,
                (12, y, w - 32, ROW_HEIGHT),
                color,
                Align::Right,
            );
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
        let thumb = Self::classic_pl_thumb(h, tracks.len() as i32, scroll as i32);
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
            // the time played, or the time left with a minus before it (the minus drawn as
            // 3 columns of text.bmp's "-" where the blank goes)
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
