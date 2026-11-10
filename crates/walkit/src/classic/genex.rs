// SPDX-License-Identifier: GPL-3.0-or-later
//! `genex.bmp`'s colors: what a classic skin asks of the media library and the windows plug-ins
//! draw inside a gen frame. Winamp hands the bitmap to them; color `i` is the pixel at x 48 + 2i
//! of row 0, in the order the Winamp SDK's `wa_dlg.h` names them.

use crate::draw::Rgb;
use crate::runtime::SkinRuntime;

/// The sheet the colors come from.
const SHEET: &str = "genex.bmp";
/// How many colors the sheet holds.
pub const GENEX_COLORS: usize = 24;

/// The colors `genex.bmp` names, in its order (the Winamp SDK's `wa_dlg.h` names them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum GenexColor {
    /// A list's background.
    ItemBg,
    /// A list's text.
    ItemFg,
    /// The window's background.
    WindowBg,
    /// A button's text.
    ButtonFg,
    /// The window's text.
    WindowFg,
    /// A highlight.
    Hilite,
    /// A selected item's background.
    SelColor,
    /// A list header's background.
    ListHeaderBg,
    /// A list header's text.
    ListHeaderFont,
    /// A list header frame's top line.
    ListHeaderFrameTop,
    /// A list header frame's middle.
    ListHeaderFrameMiddle,
    /// A list header frame's bottom line.
    ListHeaderFrameBottom,
    /// The header's background past the last column.
    ListHeaderEmptyBg,
    /// A scroll bar's thumb.
    ScrollbarFg,
    /// A scroll bar's track.
    ScrollbarBg,
    /// An inactive scroll bar's thumb.
    ScrollbarInvFg,
    /// An inactive scroll bar's track.
    ScrollbarInvBg,
    /// The corner between two scroll bars.
    ScrollbarDeadArea,
    /// The selection bar's text.
    SelbarFg,
    /// The selection bar's background.
    SelbarBg,
    /// An inactive selection bar's text.
    InactSelbarFg,
    /// An inactive selection bar's background.
    InactSelbarBg,
    /// Every second list row's background.
    ItemBg2,
    /// Every second list row's text.
    ItemFg2,
}

/// The media library's colors for a pixel it does not take.
const DEFAULTS: [u32; GENEX_COLORS] = [
    0x000000, 0x00ff00, 0x24243c, 0x393842, 0xffffff, 0x8494a5, 0x0000c6, 0x484878, 0xffffff,
    0x6c6cb4, 0x24243c, 0x12121e, 0x24243c, 0x24243c, 0x24243c, 0x798296, 0x4e586e, 0x24243c,
    0xffffff, 0x0000b4, 0x00ff00, 0x000080, 0x000000, 0x00ff00,
];

/// The colors from row 0 of the sheet, `None` for a sheet that cannot be read. A pixel past
/// the sheet's edge, the cyan 00c6ff or the color of pixel 111 counts as unset: the selection
/// bars then follow the window's text and the selection color, the second row colors follow
/// the first, and the rest take the media library's colors.
pub(crate) fn colors(row: Option<&[u32]>) -> [Rgb; GENEX_COLORS] {
    let at = |x: usize| row.and_then(|r| r.get(x)).map(|p| p & 0x00ff_ffff);
    let marker = at(111);
    let mut out = [Rgb(0); GENEX_COLORS];
    for i in 0..GENEX_COLORS {
        let px = at(48 + 2 * i);
        let unset = px.is_none() || px == Some(0x00c6ff) || px == marker;
        out[i] = match (unset, i) {
            (false, _) => Rgb(px.unwrap_or(0)),
            (true, 18 | 20) => out[GenexColor::WindowFg as usize],
            (true, 19) => out[GenexColor::SelColor as usize],
            (true, 21) => Rgb(half(out[GenexColor::WindowBg as usize].0)
                + half(out[GenexColor::SelColor as usize].0)),
            (true, 22) => out[GenexColor::ItemBg as usize],
            (true, 23) => out[GenexColor::ItemFg as usize],
            (true, _) => Rgb(DEFAULTS[i]),
        };
    }
    out
}

/// Each channel of `c` halved, as the media library halves them.
fn half(c: u32) -> u32 {
    (c / 2) & 0x007f_7f7f
}

impl SkinRuntime {
    /// The colors a classic skin's `genex.bmp` gives what the host draws in a gen window (the
    /// media library's lists, a plug-in's controls), indexed by [`GenexColor`]. A skin without
    /// the sheet takes the base skin's; without either it gets the media library's colors.
    pub fn classic_genex_colors(&mut self) -> [Rgb; GENEX_COLORS] {
        let sheet = self.classic_sheet(SHEET);
        let width = self.image_size(&sheet).map_or(0, |s| s.0);
        let row = (width > 0)
            .then(|| self.host.pixels(&sheet, 0, 0, width, 1))
            .flatten();
        colors(row.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::{colors, GenexColor, DEFAULTS, GENEX_COLORS};
    use crate::draw::Rgb;

    #[test]
    fn colors_come_from_every_second_pixel_from_48() {
        let mut row = vec![0xff00_0000u32; 130];
        for i in 0..GENEX_COLORS {
            row[48 + 2 * i] = 0xff00_0000 | (0x10 + i as u32);
        }
        row[111] = 0xff12_3456;
        let c = colors(Some(&row));
        assert_eq!(c[GenexColor::ItemBg as usize], Rgb(0x10));
        assert_eq!(c[GenexColor::ItemFg2 as usize], Rgb(0x10 + 23));
    }

    #[test]
    fn unset_pixels_follow_the_media_librarys_rules() {
        // a 60-pixel sheet: colors 6 and on lie past its edge
        let mut row = vec![0xff00_c6ffu32; 60];
        row[48] = 0xff11_1111; // item bg
        row[50] = 0xff22_2222; // item fg
        row[52] = 0xff44_4444; // window bg
        row[56] = 0xff66_6666; // window fg
        let c = colors(Some(&row));
        assert_eq!(c[3], Rgb(DEFAULTS[3]), "button fg: cyan is unset");
        assert_eq!(c[6], Rgb(DEFAULTS[6]), "past the edge");
        assert_eq!(c[18], Rgb(0x666666), "selbar fg = window fg");
        assert_eq!(c[19], c[6], "selbar bg = sel color");
        assert_eq!(
            c[21],
            Rgb(0x222222 + 0x000063),
            "half window bg + half sel color"
        );
        assert_eq!(c[22], Rgb(0x111111));
        assert_eq!(c[23], Rgb(0x222222));
    }

    #[test]
    fn no_sheet_gives_the_media_librarys_colors_and_its_derived_ones() {
        let c = colors(None);
        assert_eq!(c[0], Rgb(DEFAULTS[0]));
        assert_eq!(c[18], Rgb(DEFAULTS[4]));
    }
}
