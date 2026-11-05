// SPDX-License-Identifier: GPL-3.0-or-later

//! The draw list and a menu as text, one command per line: the lines of the trace (`trace v1`),
//! and the form a host that takes the draw list as text reads. Rects are
//! `left,top,width,height`, colors `#rrggbb`, fractions have two decimals.

use crate::draw::{DrawCmd, Gamma, Rect, Region, RegionImage, TextCmd};
use crate::host::{MenuItem, SkinMenu};

/// A draw command as one line.
pub fn line(cmd: &DrawCmd) -> String {
    match cmd {
        DrawCmd::Image {
            bitmap,
            dst,
            alpha,
            gamma,
            smooth,
        } => image(&RegionImage {
            bitmap: bitmap.clone(),
            dst: *dst,
            alpha: *alpha,
            gamma: *gamma,
            smooth: *smooth,
        }),
        DrawCmd::Fill {
            rect: r,
            color: c,
            alpha,
        } => format!("fill {} {} a={alpha}", rect(r), color(c.0)),
        DrawCmd::WindowStart(r) => format!("shape {}", region(r)),
        DrawCmd::WindowEnd => "endshape".to_string(),
        DrawCmd::Text(t) => text(t),
        DrawCmd::BitmapText(b) => format!("bitmaptext {} {}", quote(&b.text), rect(&b.rect)),
        DrawCmd::Clip(r) => format!("clip {}", rect(r)),
        DrawCmd::Unclip => "unclip".to_string(),
        DrawCmd::Pixels {
            rect: r,
            w,
            h,
            argb,
            alpha,
            stable,
        } => {
            format!(
                "pixels {} {w}x{h} a={alpha} crc={:08x}{}",
                rect(r),
                crc32_of(argb),
                if *stable { " stable" } else { "" }
            )
        }
    }
}

/// A menu as one line: `[id:text*! - text>[...]]`, `*` checked, `!` disabled, `-` a separator.
pub fn menu(menu: &SkinMenu) -> String {
    let parts: Vec<String> = menu
        .items
        .iter()
        .map(|it| match it {
            MenuItem::Command {
                id,
                text,
                checked,
                disabled,
            } => {
                format!(
                    "{id}:{text}{}{}",
                    if *checked { "*" } else { "" },
                    if *disabled { "!" } else { "" }
                )
            }
            MenuItem::Separator => "-".to_string(),
            MenuItem::Submenu { text, menu: sub } => format!("{text}>{}", self::menu(sub)),
        })
        .collect();
    format!("[{}]", parts.join(" "))
}

/// The CRC-32 (IEEE) of the pixels as bytes, each ARGB big-endian, as `java.util.zip.CRC32` sums them.
pub fn crc32_of(argb: &[u32]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for p in argb {
        for b in p.to_be_bytes() {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
    }
    !crc
}

fn image(cmd: &RegionImage) -> String {
    let b = &cmd.bitmap;
    let mut s = format!(
        "image {} {},{},{},{} -> {} a={}",
        b.file,
        b.x,
        b.y,
        b.w.map(|w| w.to_string()).unwrap_or_else(|| "-".into()),
        b.h.map(|h| h.to_string()).unwrap_or_else(|| "-".into()),
        rect(&cmd.dst),
        cmd.alpha
    );
    if let Some(g) = cmd.gamma.filter(|g| !g.is_identity()) {
        s.push_str(&format!(" g={}", gamma(&g)));
    }
    if cmd.smooth {
        s.push_str(" smooth");
    }
    s
}

fn text(cmd: &TextCmd) -> String {
    let mut s = format!(
        "text {} font={} size={} {} a={} {} x={} base={}",
        quote(&cmd.text),
        cmd.font
            .as_ref()
            .map(|f| f.file.to_string())
            .unwrap_or_else(|| "-".into()),
        cmd.size,
        color(cmd.color.0),
        cmd.alpha,
        rect(&cmd.rect),
        fixed2(cmd.x),
        fixed2(cmd.baseline)
    );
    if cmd.bold {
        s.push_str(" bold");
    }
    if let Some(sh) = cmd.shadow {
        s.push_str(&format!(
            " shadow={} {},{}",
            color(sh.0),
            cmd.shadow_dx,
            cmd.shadow_dy
        ));
    }
    s
}

fn region(r: &Region) -> String {
    match r {
        Region::Pic(img) => format!("pic({})", image(img).trim_start_matches("image ")),
        Region::Box(b) => format!("box({})", rect(b)),
        Region::Op {
            base,
            add,
            keep,
            cut,
        } => {
            let mut parts = Vec::new();
            if let Some(b) = base {
                parts.push(format!("base={}", region(b)));
            }
            let list = |rs: &[Region]| rs.iter().map(region).collect::<Vec<_>>().join(" ");
            if !add.is_empty() {
                parts.push(format!("add=[{}]", list(add)));
            }
            if !keep.is_empty() {
                parts.push(format!("keep=[{}]", list(keep)));
            }
            if !cut.is_empty() {
                parts.push(format!("cut=[{}]", list(cut)));
            }
            format!("op({})", parts.join(" "))
        }
    }
}

fn gamma(g: &Gamma) -> String {
    format!("{},{},{},{},{}", g.r, g.g, g.b, g.boost, g.gray)
}

/// `left,top,width,height`.
pub fn rect(r: &Rect) -> String {
    format!("{},{},{},{}", r.left, r.top, r.width, r.height)
}

fn color(rgb: u32) -> String {
    format!("#{:06x}", rgb & 0xFFFFFF)
}

/// A float with two decimals, rounded half up as Java's `%.2f` rounds, written with integer
/// arithmetic so the engine carries no float formatting for it.
pub fn fixed2(f: f32) -> String {
    let scaled = f as f64 * 100.0;
    let rounded = if scaled.fract().abs() == 0.5 {
        scaled.trunc() + scaled.signum()
    } else {
        scaled.round()
    };
    if !rounded.is_finite() || rounded.abs() >= 1e15 {
        return "?".to_string();
    }
    let cents = rounded as i64;
    let sign = if cents < 0 { "-" } else { "" };
    let a = cents.unsigned_abs();
    format!("{sign}{}.{:02}", a / 100, a % 100)
}

fn quote(s: &str) -> String {
    format!(
        "\"{}\"",
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_decimals_round_half_up() {
        assert_eq!(fixed2(24.2), "24.20");
        assert_eq!(fixed2(0.125), "0.13");
        assert_eq!(fixed2(-0.125), "-0.13");
        assert_eq!(fixed2(12.345), "12.35"); // 12.345f32 is 12.3450003 as a double
        assert_eq!(fixed2(0.0), "0.00");
        assert_eq!(fixed2(-3.0), "-3.00");
        assert_eq!(fixed2(17.5), "17.50");
    }
}
