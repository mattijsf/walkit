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

    /// Where the default set keeps a stand-in font.
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

/// A TrueType font's line as Windows lays it out: GDI places text from the OS/2 table's
/// usWinAscent and usWinDescent, each rounded up to whole pixels. Renderers read it from the font file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GdiLineMetrics {
    units_per_em: i32,
    win_ascent: i32,
    win_descent: i32,
}

impl GdiLineMetrics {
    /// Ascent and descent in pixels at an em of `px`.
    pub fn at(&self, px: f32) -> (i32, i32) {
        (
            (self.win_ascent as f32 * px / self.units_per_em as f32).ceil() as i32,
            (self.win_descent as f32 * px / self.units_per_em as f32).ceil() as i32,
        )
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
            Some(GdiLineMetrics {
                units_per_em: upem,
                win_ascent: ascent,
                win_descent: descent,
            })
        }
    }
}
