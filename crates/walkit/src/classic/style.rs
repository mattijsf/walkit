// SPDX-License-Identifier: GPL-3.0-or-later
//! Which Winamp a classic skin is shown as. The windows follow Winamp 5.666 unless the host
//! picks another version; each style holds what that version of Winamp does differently
//! with the same skin.

/// 2.91's names for the equalizer's sliders in its drag text.
const EQ_NAMES_291: [&str; 11] = [
    "PREAMP", "60Hz", "170Hz", "310Hz", "600Hz", "1KHz", "3KHz", "6KHz", "12KHz", "14KHz", "16KHz",
];
/// 5.666's band frequencies in its drag text.
const EQ_BANDS_5666: [&str; 10] = ["70", "180", "320", "600", "1", "3", "6", "12", "14", "16"];

/// A Winamp version whose classic windows the engine can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassicStyle {
    /// Winamp 2.91, as webamp shows it.
    Winamp291,
    /// Winamp 5.666, the reference VM's.
    #[default]
    Winamp5666,
}

impl ClassicStyle {
    /// The style by its version, "2.91" or "5.666".
    pub fn from_version(v: &str) -> Option<Self> {
        match v.trim() {
            "2.91" => Some(Self::Winamp291),
            "5.666" => Some(Self::Winamp5666),
            _ => None,
        }
    }

    /// The version as Winamp names itself, shown in the title row with nothing loaded.
    pub fn name(self) -> &'static str {
        match self {
            Self::Winamp291 => "Winamp 2.91",
            Self::Winamp5666 => "Winamp 5.666",
        }
    }

    /// The playlist font size the version starts with (`pe_fontsize`, pixels to the em).
    pub fn list_font_size(self) -> i32 {
        match self {
            Self::Winamp291 => 10,
            Self::Winamp5666 => 12,
        }
    }

    /// The balance thumb's places (0..=24) that snap to the middle while dragged: 2.91 five
    /// either side of 12, 5.666 three.
    pub(crate) fn balance_snap(self) -> std::ops::RangeInclusive<i32> {
        match self {
            Self::Winamp291 => 10..=14,
            Self::Winamp5666 => 11..=13,
        }
    }

    /// The equalizer shade bar's balance for column `i` of 42: 2.91 runs -128..127 and snaps
    /// within 20 of the middle, 5.666 caps at 127 and snaps within 8.
    pub(crate) fn shade_balance(self, i: i32) -> i32 {
        match self {
            Self::Winamp291 => {
                let b = i * 255 / 42 - 128;
                if b.abs() < 20 {
                    0
                } else {
                    b
                }
            }
            Self::Winamp5666 => {
                let b = (i * 255 / 42 - 127).min(127);
                if (-8..=8).contains(&b) {
                    0
                } else {
                    b
                }
            }
        }
    }

    /// Rows the playlist's scroll arrows move: 2.91 four, 5.666 its
    /// `plscrollsize`, one.
    pub(crate) fn arrow_rows(self) -> i32 {
        match self {
            Self::Winamp291 => 4,
            Self::Winamp5666 => 1,
        }
    }

    /// Rows the playlist moves for a notch of the mouse wheel: 2.91 eight, 5.666 two
    /// (`plmw2xscroll`).
    pub(crate) fn wheel_rows(self) -> i32 {
        match self {
            Self::Winamp291 => 8,
            Self::Winamp5666 => 2,
        }
    }

    /// Whether gen window titles draw digits, '-' and ':' from `gen.bmp` (where `pledit.txt`
    /// asks); 2.91 knows only the letters and leaves a gap for the rest.
    pub(crate) fn gen_digits(self) -> bool {
        self == Self::Winamp5666
    }

    /// Whether the analyzer, clicked off while playing, leaves its dot grid: 2.91 blits the
    /// grid; 5.666 lets `main.bmp` show.
    pub(crate) fn vis_off_grid(self) -> bool {
        self == Self::Winamp291
    }

    /// The title row's text while equalizer slider `n` (0 the preamp) is dragged to step
    /// `step` 0..=63: dB = (step - 31.5) / 31.5 * -12, within 0.32 of zero shown as zero.
    /// 5.666 writes "EQ: 70Hz: +3.0 dB"; 2.91 "EQ: 60Hz: +3.0 db" with its
    /// own band names and the tenths rounded from the hundredths.
    pub(crate) fn eq_text(self, n: usize, step: i32) -> String {
        let db = (f64::from(step) - 31.5) / 31.5 * -12.0;
        let db = if (-0.32..0.32).contains(&db) { 0.0 } else { db };
        match self {
            Self::Winamp291 => {
                let sign = if db < 0.0 { "-" } else { "+" };
                let whole = db.abs() as i32;
                let tenth = ((db.abs() * 100.0) as i32 % 100 + 5) / 10;
                format!("EQ: {}: {sign}{whole}.{tenth} db", EQ_NAMES_291[n])
            }
            Self::Winamp5666 => {
                let name = if n == 0 {
                    "Preamp".to_string()
                } else {
                    let unit = if n >= 5 { "KHz" } else { "Hz" };
                    format!("{}{unit}", EQ_BANDS_5666[n - 1])
                };
                let sign = if db < 0.0 { "" } else { "+" };
                format!("EQ: {name}: {sign}{db:.1} dB")
            }
        }
    }
}

impl crate::runtime::SkinRuntime {
    /// Shows the classic windows as `style`'s Winamp does, its playlist font size included.
    pub fn set_classic_style(&mut self, style: ClassicStyle) {
        self.classic_style = style;
        self.set_classic_list_font_size(style.list_font_size());
        self.repaint();
    }

    /// The Winamp the classic windows are shown as.
    pub fn classic_style(&self) -> ClassicStyle {
        self.classic_style
    }

    /// Scrolls a classic playlist for `notches` of the mouse wheel, down for more than 0, as
    /// many rows a notch as the style's Winamp does.
    pub fn wheel_classic_playlist(&mut self, notches: i32) {
        self.scroll_classic_playlist(notches * self.classic_style.wheel_rows());
    }
}

#[cfg(test)]
mod tests {
    use super::ClassicStyle::{Winamp291, Winamp5666};

    #[test]
    fn the_eq_text_names_each_versions_bands_and_rounds_its_way() {
        assert_eq!(Winamp5666.eq_text(1, 20), "EQ: 70Hz: +4.4 dB");
        assert_eq!(Winamp291.eq_text(1, 20), "EQ: 60Hz: +4.4 db");
        assert_eq!(Winamp5666.eq_text(0, 63), "EQ: Preamp: -12.0 dB");
        assert_eq!(Winamp291.eq_text(0, 63), "EQ: PREAMP: -12.0 db");
        assert_eq!(Winamp291.eq_text(3, 31), "EQ: 310Hz: +0.0 db");
        // the tenths come from the hundredths cut, then rounded: 9.714 shows 9.7
        assert_eq!(Winamp291.eq_text(10, 6), "EQ: 16KHz: +9.7 db");
    }

    #[test]
    fn balance_snaps_wider_in_2_91() {
        assert!(Winamp291.balance_snap().contains(&10));
        assert!(!Winamp5666.balance_snap().contains(&10));
        assert_eq!(Winamp291.shade_balance(0), -128);
        assert_eq!(Winamp5666.shade_balance(0), -127);
        // column 18: 2.91's -19 snaps to the middle, 5.666's -18 is past its snap
        assert_eq!(Winamp291.shade_balance(18), 0);
        assert_eq!(Winamp5666.shade_balance(18), -18);
        assert_eq!(Winamp291.shade_balance(42), 127);
    }
}
