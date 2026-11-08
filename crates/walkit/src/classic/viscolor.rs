// SPDX-License-Identifier: GPL-3.0-or-later
//! `viscolor.txt`: the vis palette, 24 colors as `r,g,b` lines (a `//` comment after each), as
//! Winamp reads them: 0 the background, 1 the dot grid, 2..17 the analyzer's colors from the
//! top down, 18..22 the oscilloscope's, 23 the peak marks.

use crate::draw::Rgb;

/// The palette: 24 colors.
pub type Palette = [Rgb; 24];

/// Winamp's own palette, for a skin without `viscolor.txt`: the colors Winamp shows then
/// (the same 24 colors in 5.666 and 2.91).
pub const BASE: Palette = [
    Rgb(0x000000),
    Rgb(0x181829),
    Rgb(0xef3110),
    Rgb(0xce2910),
    Rgb(0xd65a00),
    Rgb(0xd66600),
    Rgb(0xd67300),
    Rgb(0xc67b08),
    Rgb(0xdea518),
    Rgb(0xd6b521),
    Rgb(0xbdde29),
    Rgb(0x94de21),
    Rgb(0x29ce10),
    Rgb(0x32be10),
    Rgb(0x39b510),
    Rgb(0x319c08),
    Rgb(0x299400),
    Rgb(0x188408),
    Rgb(0xffffff),
    Rgb(0xd6d6de),
    Rgb(0xb5bdbd),
    Rgb(0xa0aaaf),
    Rgb(0x949ca5),
    Rgb(0x969696),
];

/// `text`, a `viscolor.txt`, over the base palette: each line's first three numbers.
pub fn parse(text: &str) -> Palette {
    let mut p = BASE;
    let mut i = 0;
    for line in text.lines() {
        if i >= p.len() {
            break;
        }
        let head = line.split("//").next().unwrap_or("");
        let nums: Vec<u32> = head
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|n| n.trim().parse::<u32>().ok())
            .take(3)
            .collect();
        if nums.len() == 3 {
            p[i] = Rgb((nums[0].min(255) << 16) | (nums[1].min(255) << 8) | nums[2].min(255));
            i += 1;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::{parse, BASE};
    use crate::draw::Rgb;

    #[test]
    fn lines_set_the_colors_in_order_and_comments_are_skipped() {
        let p =
            parse("43,41,48, // background\n73,69,79, // dot grid\n239,184,200 // spectrum 0\n");
        assert_eq!(p[0], Rgb(0x2b2930));
        assert_eq!(p[1], Rgb(0x49454f));
        assert_eq!(p[2], Rgb(0xefb8c8));
        assert_eq!(p[3], BASE[3]);
    }
}
