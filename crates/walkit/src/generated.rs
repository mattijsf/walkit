// SPDX-License-Identifier: GPL-3.0-or-later

//! Pictures Winamp makes instead of reading a file. `<bitmap file="$solid" w h color alpha>` is
//! a w x h picture of one color (DeClassified's playlist middle and frame texture). The bitmap
//! element is given a made-up file name that holds everything needed, so a host's files can
//! answer it with the picture.

use crate::runtime::layout::MAX_SIDE;

const SOLID: &str = "$solid/";

/// The file name for a `$solid` element: `$solid/<rrggbb><aa>/<w>x<h>.png`.
pub fn solid_name(
    w: Option<i32>,
    h: Option<i32>,
    color: Option<&str>,
    alpha: Option<i32>,
) -> String {
    let c = crate::gui::parse_rgb(color.unwrap_or(""))
        .map(|c| c.0)
        .unwrap_or(0);
    let a = alpha.unwrap_or(255).clamp(0, 255);
    format!(
        "{SOLID}{:06x}{:02x}/{}x{}.png",
        c & 0xffffff,
        a,
        w.unwrap_or(1).clamp(1, MAX_SIDE),
        h.unwrap_or(1).clamp(1, MAX_SIDE)
    )
}

/// What a made-up name asks for: the width, the height and the ARGB color; `None` for a real file.
pub fn solid_spec(path: &str) -> Option<(i32, i32, u32)> {
    let rest = path.strip_prefix(SOLID)?.strip_suffix(".png")?;
    let (spec, size) = rest.split_once('/')?;
    if size.contains('/') {
        return None;
    }
    let argb = u32::from_str_radix(spec, 16).ok()?;
    let (w, h) = size.split_once('x')?;
    let w = w.parse::<i32>().unwrap_or(1).clamp(1, MAX_SIDE);
    let h = h.parse::<i32>().unwrap_or(1).clamp(1, MAX_SIDE);
    let rgb = argb >> 8;
    let a = argb & 0xff;
    Some((w, h, (a << 24) | rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solid_name_round_trips() {
        let name = solid_name(Some(4), Some(2), Some("255,0,0"), Some(128));
        assert_eq!(name, "$solid/ff000080/4x2.png");
        assert_eq!(solid_spec(&name), Some((4, 2, 0x80ff_0000)));
    }
}
