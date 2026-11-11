// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the glyphs of a text come from, and a TrueType font's line as Windows lays it out.

use crate::draw::FontDef;

/// Where the glyphs of a text come from, as the engine chooses them; every renderer and every
/// text measure goes by the same choice.
#[derive(Debug, Clone, PartialEq)]
pub enum FontFace {
    /// The skin's font; the renderer makes it bold itself when `fake_bold`.
    SkinFile {
        /// The font.
        font: FontDef,
        /// Made bold by the renderer.
        fake_bold: bool,
    },
    /// A free font that stands in for a Windows system font the skin names without a file, in
    /// its bold cut when `bold`, so it is never made bold by the renderer.
    StandIn {
        /// The family.
        family: &'static str,
        /// The bold cut.
        bold: bool,
    },
    /// No font named: the platform's default, made bold by the renderer when `fake_bold`.
    Default {
        /// Made bold by the renderer.
        fake_bold: bool,
    },
}

/// Windows fonts skins name without a file, and the free font that stands in.
const STAND_INS: [(&str, &str); 2] = [
    ("tahoma", "DejaVuSansCondensed"),
    ("arial", "LiberationSans"),
];

impl FontFace {
    /// The face `font` is drawn and measured with, `bold` or not.
    pub fn of(font: Option<&FontDef>, bold: bool) -> FontFace {
        let Some(font) = font else {
            return FontFace::Default { fake_bold: bold };
        };
        if font.file.is_empty() {
            if let Some((_, family)) = STAND_INS.iter().find(|(id, _)| *id == &*font.id) {
                return FontFace::StandIn { family, bold };
            }
        }
        FontFace::SkinFile {
            font: font.clone(),
            fake_bold: bold,
        }
    }

    /// Where the wasabi base keeps a stand-in font.
    pub fn file(&self) -> Option<String> {
        match self {
            FontFace::StandIn { family, bold } => Some(format!(
                "fonts/{family}{}.ttf",
                if *bold { "-Bold" } else { "" }
            )),
            _ => None,
        }
    }
}

/// The em size to draw a TrueType font at for a skin's `fontsize`. Winamp 5.666 draws text at
/// three quarters of it, rounded to whole pixels as Windows sizes fonts (Big Bento's fontsize 14
/// draws at 11 px; measured against its screenshots).
pub fn em_size(fontsize: i32) -> f32 {
    (fontsize as f32 * 0.75).round()
}

/// A TrueType font's line as Windows lays it out: from the OS/2 table's usWinAscent and
/// usWinDescent, each rounded up to whole pixels ([`GdiLineMetrics::at`]). A classic skin's text
/// goes by the line the font says for its size where it says one
/// ([`GdiLineMetrics::measured`]). Renderers read it from the font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GdiLineMetrics {
    units_per_em: i32,
    win_ascent: i32,
    win_descent: i32,
    /// From the `VDMX` table: pixels per em, highest point and lowest point.
    sizes: Vec<(u16, i16, i16)>,
    /// The font has a `VDMX` table, but without a group Windows reads.
    other_vdmx: bool,
}

impl GdiLineMetrics {
    /// Ascent and descent in pixels at an em of `px`, rounded up.
    pub fn at(&self, px: f32) -> (i32, i32) {
        let scale = |units: i32| units as f32 * px / self.units_per_em as f32;
        (
            scale(self.win_ascent).ceil() as i32,
            scale(self.win_descent).ceil() as i32,
        )
    }

    /// Ascent and descent at an em of `px` where the font says them for that size, as a
    /// classic skin's text is laid out (probed with Arial and Tahoma): the `VDMX` table's group
    /// for Windows' character set, or, for a font whose table has only a group for all
    /// characters (Tahoma), usWinAscent and usWinDescent rounded to the nearest pixel. `None`
    /// for a font without the table.
    pub fn measured(&self, px: f32) -> Option<(i32, i32)> {
        let ppem = px.round() as u16;
        if let Some(&(_, high, low)) = self.sizes.iter().find(|s| s.0 == ppem) {
            return Some((i32::from(high), -i32::from(low)));
        }
        self.other_vdmx.then(|| {
            let scale = |units: i32| units as f32 * px / self.units_per_em as f32;
            (
                scale(self.win_ascent).round() as i32,
                scale(self.win_descent).round() as i32,
            )
        })
    }

    /// From a TrueType file's bytes; `None` when it has no head or OS/2 table.
    pub fn read(bytes: &[u8]) -> Option<GdiLineMetrics> {
        let u16_at = |o: usize| -> i32 {
            if o + 2 <= bytes.len() {
                ((bytes[o] as i32) << 8) | bytes[o + 1] as i32
            } else {
                -1
            }
        };
        let u32_at = |o: usize| -> i64 { ((u16_at(o) as i64) << 16) | u16_at(o + 2) as i64 };
        let tables = u16_at(4);
        if tables <= 0 {
            return None;
        }
        let mut head: i64 = -1;
        let mut os2: i64 = -1;
        let mut vdmx: i64 = -1;
        for i in 0..tables as usize {
            let rec = 12 + i * 16;
            if rec + 16 > bytes.len() {
                break;
            }
            let tag = &bytes[rec..rec + 4];
            let offset = u32_at(rec + 8);
            match tag {
                b"head" => head = offset,
                b"OS/2" => os2 = offset,
                b"VDMX" => vdmx = offset,
                _ => {}
            }
        }
        if head < 0 || os2 < 0 {
            return None;
        }
        let upem = u16_at(head as usize + 18);
        let ascent = u16_at(os2 as usize + 74);
        let descent = u16_at(os2 as usize + 76);
        if upem <= 0 || ascent < 0 || descent < 0 {
            None
        } else {
            let (sizes, other_vdmx) = if vdmx < 0 {
                (Vec::new(), false)
            } else {
                read_vdmx(bytes, vdmx as usize)
            };
            Some(GdiLineMetrics {
                units_per_em: upem,
                win_ascent: ascent,
                win_descent: descent,
                sizes,
                other_vdmx,
            })
        }
    }
}

/// The sizes of the first `VDMX` group for Windows' character set that holds a 1:1 aspect
/// ratio, and whether the table has groups but none Windows reads.
fn read_vdmx(bytes: &[u8], at: usize) -> (Vec<(u16, i16, i16)>, bool) {
    let u8_at = |o: usize| bytes.get(o).copied();
    let u16_at = |o: usize| Some(u16::from_be_bytes([u8_at(o)?, u8_at(o + 1)?]));
    let Some(ratios) = u16_at(at + 4) else {
        return (Vec::new(), false);
    };
    for i in 0..ratios as usize {
        let rec = at + 6 + 4 * i;
        let (Some(charset), Some(x), Some(start), Some(end)) =
            (u8_at(rec), u8_at(rec + 1), u8_at(rec + 2), u8_at(rec + 3))
        else {
            break;
        };
        // a group for all aspect ratios, or one whose range takes 1:1
        let square = x == 0 || (start <= x && x <= end);
        if charset != 1 || !square {
            continue;
        }
        let Some(offset) = u16_at(at + 6 + 4 * ratios as usize + 2 * i) else {
            break;
        };
        let group = at + offset as usize;
        let count = u16_at(group).unwrap_or(0) as usize;
        let sizes = (0..count)
            .map_while(|k| {
                let r = group + 4 + 6 * k;
                Some((u16_at(r)?, u16_at(r + 2)? as i16, u16_at(r + 4)? as i16))
            })
            .collect();
        return (sizes, false);
    }
    (Vec::new(), ratios > 0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A size in a VDMX table: pixels per em, highest point, lowest point.
    pub(crate) type VdmxSize = (u16, i16, i16);

    /// A font file of only the tables the line is read from: head (2048 units to the em), OS/2
    /// (ascent 1854, descent 434) and, when given, a VDMX table of one group.
    pub(crate) fn font_with(vdmx: Option<(u8, &[VdmxSize])>) -> Vec<u8> {
        let mut head = vec![0u8; 54];
        head[18..20].copy_from_slice(&2048u16.to_be_bytes());
        let mut os2 = vec![0u8; 78];
        os2[74..76].copy_from_slice(&1854u16.to_be_bytes());
        os2[76..78].copy_from_slice(&434u16.to_be_bytes());
        let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![(b"head", head), (b"OS/2", os2)];
        if let Some((charset, sizes)) = vdmx {
            let mut t = Vec::new();
            t.extend_from_slice(&1u16.to_be_bytes()); // version
            t.extend_from_slice(&1u16.to_be_bytes()); // groups
            t.extend_from_slice(&1u16.to_be_bytes()); // ratios
            t.extend_from_slice(&[charset, 1, 1, 1]);
            t.extend_from_slice(&12u16.to_be_bytes()); // the group, after this offset
            t.extend_from_slice(&(sizes.len() as u16).to_be_bytes());
            t.extend_from_slice(&[0, 255]);
            for &(ppem, high, low) in sizes {
                t.extend_from_slice(&ppem.to_be_bytes());
                t.extend_from_slice(&high.to_be_bytes());
                t.extend_from_slice(&low.to_be_bytes());
            }
            tables.push((b"VDMX", t));
        }
        let mut out = vec![0u8; 12 + 16 * tables.len()];
        out[4..6].copy_from_slice(&(tables.len() as u16).to_be_bytes());
        for (i, (tag, data)) in tables.iter().enumerate() {
            let rec = 12 + 16 * i;
            let offset = out.len() as u32;
            out[rec..rec + 4].copy_from_slice(*tag);
            out[rec + 8..rec + 12].copy_from_slice(&offset.to_be_bytes());
            out[rec + 12..rec + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    #[test]
    fn the_line_rounds_up_without_a_vdmx_table() {
        let m = GdiLineMetrics::read(&font_with(None)).expect("a line");
        assert_eq!(m.at(18.0), (17, 4));
        assert_eq!(m.at(24.0), (22, 6));
        assert_eq!(m.measured(18.0), None);
    }

    #[test]
    fn a_vdmx_group_for_windows_characters_gives_the_measured_line() {
        let m = GdiLineMetrics::read(&font_with(Some((1, &[(18, 17, -4), (24, 21, -6)]))))
            .expect("a line");
        assert_eq!(m.measured(24.0), Some((21, 6)));
        assert_eq!(m.measured(18.0), Some((17, 4)));
        assert_eq!(m.measured(12.0), None, "a size the table leaves out");
        assert_eq!(m.at(24.0), (22, 6), "the rounded-up line stays as it was");
    }

    #[test]
    fn a_vdmx_table_only_for_all_characters_rounds_the_measured_line() {
        let m = GdiLineMetrics::read(&font_with(Some((0, &[(16, 17, -4)])))).expect("a line");
        // 1854 and 434 of 2048 at 16 pixels: 14.48 and 3.39
        assert_eq!(m.measured(16.0), Some((14, 3)));
    }
}
