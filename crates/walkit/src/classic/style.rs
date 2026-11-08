// SPDX-License-Identifier: GPL-3.0-or-later
//! Which Winamp a classic skin is shown as. The windows follow Winamp 5.666 unless the host
//! picks another version; each style holds what that version of Winamp does differently
//! with the same skin.

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
}
