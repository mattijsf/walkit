// SPDX-License-Identifier: GPL-3.0-or-later
//! `viscolor.txt`: the vis palette, 24 colors as `r,g,b` lines (a `//` comment after each), as
//! Winamp reads them: 0 the background, 1 the dot grid, 2..17 the analyzer's colors from the
//! top down, 18..22 the oscilloscope's, 23 the peak marks.

use crate::draw::Rgb;

/// The palette: 24 colors.
pub type Palette = [Rgb; 24];

/// Winamp's base skin palette, for a skin without `viscolor.txt`.
pub const BASE: Palette = [
    Rgb(0x000000),
    Rgb(0x181818),
    Rgb(0xc00000),
    Rgb(0xc00000),
    Rgb(0xd50000),
    Rgb(0xe30000),
    Rgb(0xe60500),
    Rgb(0xea0b00),
    Rgb(0xf21300),
    Rgb(0xf71a00),
    Rgb(0xf53000),
    Rgb(0xf53d00),
    Rgb(0xf65000),
    Rgb(0xf66400),
    Rgb(0xf77800),
    Rgb(0xf78b00),
    Rgb(0xf79e00),
    Rgb(0xf7b100),
    Rgb(0xf7c400),
    Rgb(0xf7d700),
    Rgb(0xf7ea00),
    Rgb(0xf7fd00),
    Rgb(0xf7ff20),
    Rgb(0xffffff),
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
