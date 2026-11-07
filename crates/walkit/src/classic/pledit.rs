// SPDX-License-Identifier: GPL-3.0-or-later
//! `pledit.txt`: the playlist's colors and font (`[Text]`: `Normal`, `Current`, `NormalBG`,
//! `SelectedBG`, `Font`), as Winamp reads them.

use crate::draw::Rgb;

/// The playlist's look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PleditStyle {
    /// A row's text.
    pub normal: Rgb,
    /// The current song's text.
    pub current: Rgb,
    /// The list's background.
    pub normal_bg: Rgb,
    /// A selected row's background.
    pub selected_bg: Rgb,
    /// The font's face name.
    pub font: String,
    /// The video window's status text (`mbFG`).
    pub mb_fg: Rgb,
    /// The video window's status background (`mbBG`).
    pub mb_bg: Rgb,
    /// Whether `gen.bmp`'s digits, '-' and ':' show in window titles (`[Misc]` `UseGenNums`).
    pub use_gen_nums: bool,
}

impl Default for PleditStyle {
    /// Winamp's base skin: green on black, Arial.
    fn default() -> Self {
        PleditStyle {
            normal: Rgb(0x00ff00),
            current: Rgb(0xffffff),
            normal_bg: Rgb(0x000000),
            selected_bg: Rgb(0x0000c6),
            font: "Arial".to_string(),
            mb_fg: Rgb(0x00ff00),
            mb_bg: Rgb(0x000000),
            use_gen_nums: false,
        }
    }
}

/// `text`, a `pledit.txt`, over the defaults.
pub fn parse(text: &str) -> PleditStyle {
    let mut s = PleditStyle::default();
    let mut section = String::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if section == "misc" && key.trim().eq_ignore_ascii_case("usegennums") {
            s.use_gen_nums = value.parse::<i32>().is_ok_and(|v| v != 0);
            continue;
        }
        if section != "text" {
            continue;
        }
        match key.trim().to_lowercase().as_str() {
            "normal" => s.normal = color(value).unwrap_or(s.normal),
            "current" => s.current = color(value).unwrap_or(s.current),
            "normalbg" => s.normal_bg = color(value).unwrap_or(s.normal_bg),
            "selectedbg" => s.selected_bg = color(value).unwrap_or(s.selected_bg),
            "font" => s.font = value.to_string(),
            "mbfg" => s.mb_fg = color(value).unwrap_or(s.mb_fg),
            "mbbg" => s.mb_bg = color(value).unwrap_or(s.mb_bg),
            _ => {}
        }
    }
    s
}

/// `#rrggbb`, as the file writes a color.
fn color(value: &str) -> Option<Rgb> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok().map(Rgb)
}

#[cfg(test)]
mod tests {
    use super::{parse, PleditStyle};
    use crate::draw::Rgb;

    #[test]
    fn the_text_section_sets_the_colors_and_the_font() {
        let s = parse("[Text]\nNormal=#1a2b3c\nCurrent=#ffffff\nNormalBG=#101010\nSelectedBG=#202020\nFont=Tahoma\n");
        assert_eq!(s.normal, Rgb(0x1a2b3c));
        assert_eq!(s.normal_bg, Rgb(0x101010));
        assert_eq!(s.selected_bg, Rgb(0x202020));
        assert_eq!(s.font, "Tahoma");
        assert_eq!(parse(""), PleditStyle::default());
    }

    #[test]
    fn use_gen_nums_is_read_from_misc() {
        assert!(!parse("[Text]\nNormal=#00ff00\n").use_gen_nums);
        assert!(parse("[Misc]\nUseGenNums=1\n").use_gen_nums);
        assert!(
            !parse("[Text]\nUseGenNums=1\n").use_gen_nums,
            "only under [Misc]"
        );
    }
}
