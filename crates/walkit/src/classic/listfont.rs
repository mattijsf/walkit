// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic playlist's rows in the skin's `text.bmp` letters, for a host that shows them so
//! (`Presentation::skin_list_font`): each letter's ink, read against the blank cell, laid into
//! a block of pixels in the color `pledit.txt` gives the row.

use crate::classic::font;

/// A cell's size in `text.bmp`.
pub(crate) const CELL_W: i32 = 5;
pub(crate) const CELL_H: i32 = 6;

/// The cell that ends a title cut short: the font's ellipsis.
const ELLIPSIS: char = '\u{1}';

/// How much of each pixel of `text.bmp` is letter: 0 for the cell's background, 255 for the
/// letter at its strongest. The background is the blank cell's, place for place, so a sheet
/// shaded from top to bottom still reads as empty.
pub(crate) struct ListInk {
    w: i32,
    h: i32,
    ink: Vec<u8>,
}

impl ListInk {
    /// The ink of a sheet of `w` x `h` ARGB `pixels`, or `None` for one with no letters on it.
    pub(crate) fn of(pixels: &[u32], w: i32, h: i32) -> Option<ListInk> {
        let (bx, by) = font::BLANK;
        if w < bx + CELL_W || h < by + CELL_H || pixels.len() < (w * h) as usize {
            return None;
        }
        let at = |x: i32, y: i32| pixels[(y * w + x) as usize];
        let paper: Vec<u32> = (0..CELL_W * CELL_H)
            .map(|i| at(bx + i % CELL_W, by + i / CELL_W))
            .collect();
        let far = |a: u32, b: u32| {
            [16, 8, 0]
                .iter()
                .map(|s| ((a >> s & 0xff) as i32 - (b >> s & 0xff) as i32).abs())
                .max()
                .unwrap_or(0)
        };
        let rows = (h / CELL_H).min(3) * CELL_H;
        let mut raw = vec![0i32; (w * h) as usize];
        let mut strongest = 0;
        for y in 0..rows {
            for x in 0..w / CELL_W * CELL_W {
                let d = far(
                    at(x, y),
                    paper[((y % CELL_H) * CELL_W + x % CELL_W) as usize],
                );
                raw[(y * w + x) as usize] = d;
                strongest = strongest.max(d);
            }
        }
        if strongest == 0 {
            return None;
        }
        let ink = raw.iter().map(|d| (d * 255 / strongest) as u8).collect();
        Some(ListInk { w, h, ink })
    }

    /// Writes `text` into `block`, `stride` pixels wide, its first cell's top-left corner at
    /// `x`, `y`, in `rgb`; nothing at or past `right`.
    pub(crate) fn write(
        &self,
        text: &str,
        (x, y): (i32, i32),
        rgb: u32,
        block: &mut [u32],
        stride: i32,
        right: i32,
    ) {
        let rows = block.len() as i32 / stride.max(1);
        let right = right.min(stride);
        let mut cx = x;
        for c in text.chars() {
            if cx >= right {
                return;
            }
            if c != ' ' {
                let (sx, sy) = font::cell(c);
                for gy in 0..CELL_H {
                    let (src_y, dst_y) = (sy + gy, y + gy);
                    if src_y >= self.h || !(0..rows).contains(&dst_y) {
                        continue;
                    }
                    for gx in 0..CELL_W {
                        let (src_x, dst_x) = (sx + gx, cx + gx);
                        if src_x >= self.w || !(0..right).contains(&dst_x) {
                            continue;
                        }
                        let a = self.ink[(src_y * self.w + src_x) as usize] as u32;
                        if a != 0 {
                            block[(dst_y * stride + dst_x) as usize] = a << 24 | (rgb & 0xff_ffff);
                        }
                    }
                }
            }
            cx += CELL_W;
        }
    }
}

/// Whether the font spells `text`: every letter and digit in it has a cell. Other signs it
/// lacks are drawn blank, and do not count.
pub(crate) fn covers(text: &str) -> bool {
    text.chars()
        .all(|c| !c.is_alphanumeric() || font::cell(c) != font::BLANK)
}

/// `text` in at most `room` cells: one too long ends on a whole letter and the ellipsis.
pub(crate) fn fitted(text: &str, room: i32) -> String {
    let room = room.max(1) as usize;
    if text.chars().count() <= room {
        return text.to_string();
    }
    let mut out: String = text.chars().take(room - 1).collect();
    out.push(ELLIPSIS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_the_font_cannot_spell_is_not_covered() {
        assert!(covers("Andamp Whippin' Intro (1997)"));
        assert!(covers("Björk — Jóga"), "accents fold, a dash is a sign");
        assert!(!covers("Кино"));
        assert!(!covers("坂本龍一"));
    }

    #[test]
    fn a_long_title_ends_on_a_whole_letter_and_the_ellipsis() {
        assert_eq!(fitted("ABCDEF", 6), "ABCDEF");
        assert_eq!(fitted("ABCDEFG", 6), "ABCDE\u{1}");
    }

    #[test]
    fn ink_is_read_against_the_blank_cell() {
        // a sheet 155 x 18: paper 0x101010 everywhere, the A cell's top-left pixel lit
        let (w, h) = (155, 18);
        let mut px = vec![0xff10_1010u32; (w * h) as usize];
        px[0] = 0xff30_f030;
        let ink = ListInk::of(&px, w, h).expect("letters on it");
        let mut block = vec![0u32; 10 * 6];
        ink.write("AA", (0, 0), 0x00ff_0000, &mut block, 10, 10);
        assert_eq!(block[0], 0xffff_0000);
        assert_eq!(block[5], 0xffff_0000, "the second A one cell on");
        assert_eq!(block[1], 0, "paper stays clear");
        let mut cut = vec![0u32; 10 * 6];
        ink.write("AA", (0, 0), 0x00ff_0000, &mut cut, 10, 5);
        assert_eq!(cut[5], 0, "nothing past the right edge");
    }
}
