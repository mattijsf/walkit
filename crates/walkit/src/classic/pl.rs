// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic playlist editor's frame, pixel for pixel as Winamp 5.666 paints it:
//! the title bar of corners, tiles and the title, the sides
//! tiled down, and the bottom bar with its buttons, the list's background between them.

use crate::classic::font;
use crate::classic::main::Painter;
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

/// The playlist font's size, as Winamp's "playlist font size" default has it.
pub(super) const ROW_FONT: i32 = 12;
/// A row's height in that font.
const ROW_HEIGHT: i32 = 14;

impl SkinRuntime {
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
        let rows = (h - 58) / ROW_HEIGHT;
        for (i, t) in tracks.iter().take(rows.max(0) as usize).enumerate() {
            let y = 20 + i as i32 * ROW_HEIGHT;
            let color = if i as i32 == current {
                style.current
            } else {
                style.normal
            };
            let time = time_text(t.length_ms / 1000);
            p.text(
                &format!("{}. {}", i + 1, t.title),
                12,
                y,
                w - 32,
                ROW_HEIGHT,
                color,
                Align::Left,
            );
            p.text(&time, 12, y, w - 32, ROW_HEIGHT, color, Align::Right);
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
        // the scrollbar's thumb, at the top of a list that does not scroll
        p.blit("pledit.bmp", 52, 53, 8, 18, w - 15, 20);
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
        // the bottom bar's two lines in text.bmp cells: the
        // selected songs' time over the list's, 18 cells from (w-143, h-28); and the song's
        // time at (w-86, h-15), blank while nothing plays
        let total: i32 = tracks.iter().map(|t| t.length_ms / 1000).sum();
        let line = format!("0:00/{}", time_text(total));
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
            let seconds = (self.player.position_ms() / 1000).max(0);
            let (minutes, seconds) = (seconds / 60, seconds % 60);
            let digit =
                |p: &mut Painter, n: i32, x: i32| p.blit("text.bmp", (n % 10) * 5, 6, 5, 6, x, y);
            if minutes / 100 == 0 {
                p.blit("text.bmp", 142, 0, 4, 6, w - 86, y);
                p.blit("text.bmp", 142, 0, 3, 6, w - 82, y);
            } else {
                p.blit("text.bmp", 142, 0, 3, 6, w - 86, y);
                digit(&mut p, minutes / 100, w - 82);
            }
            digit(&mut p, minutes / 10, w - 77);
            digit(&mut p, minutes, w - 72);
            digit(&mut p, seconds / 10, w - 64);
            digit(&mut p, seconds, w - 59);
        }
    }
}
