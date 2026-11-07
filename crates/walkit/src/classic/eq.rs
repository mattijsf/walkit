// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic equalizer window, pixel for pixel as Winamp 5.666 paints it.

use crate::classic::main::Painter;
use crate::draw::{DrawCmd, Rect};
use crate::gui::ObjId;
use crate::runtime::SkinRuntime;

impl SkinRuntime {
    pub(crate) fn draw_classic_eq(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let mut p = Painter { at, out };
        // the background, the title bar over it; its buttons are the strip's until pressed
        p.blit("eqmain.bmp", 0, 0, 275, 116, 0, 0);
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        p.blit(
            "eqmain.bmp",
            0,
            if active { 134 } else { 149 },
            275,
            14,
            0,
            0,
        );
        // on and auto, the presets button
        let on = self.player.eq_enabled();
        p.blit(
            "eqmain.bmp",
            10 + if on { 59 } else { 0 },
            119,
            25,
            12,
            14,
            18,
        );
        p.blit("eqmain.bmp", 35, 119, 33, 12, 39, 18);
        p.blit("eqmain.bmp", 224, 164, 44, 12, 217, 18);
        // the preamp and the ten bands: a slider frame by the value, the thumb on it
        let preamp = self.player.eq_preamp();
        let bands = self.player.eq_bands();
        self.draw_classic_eq_slider(&mut p, 21, preamp);
        for (i, v) in bands.iter().take(10).enumerate() {
            self.draw_classic_eq_slider(&mut p, (i as i32 + 1) * 18 + 60, *v);
        }
        draw_classic_eq_graph(
            &mut p,
            band_step(preamp),
            &bands.iter().map(|v| band_step(*v)).collect::<Vec<_>>(),
        );
    }

    /// A band slider at `x`: the value `v` (0..255 as the player keeps a band, 128 flat, 255
    /// the top) picked as one of 28 frames of the slider's background, the thumb at its height.
    fn draw_classic_eq_slider(&self, p: &mut Painter, x: i32, v: i32) {
        let value = band_step(v);
        let frame = 27 - value * 28 / 64;
        if frame < 14 {
            p.blit("eqmain.bmp", frame * 15 + 13, 164, 14, 63, x, 38);
        } else {
            p.blit("eqmain.bmp", frame * 15 - 197, 229, 14, 63, x, 38);
        }
        let y = (value - 63) * 52 / 64 + 89;
        p.blit("eqmain.bmp", 0, 164, 11, 11, x + 1, y);
    }
}

/// A band as Winamp's classic slider holds it, 0..63 with 0 the top, from the player's 0..255 (128 flat,
/// 255 the top).
fn band_step(v: i32) -> i32 {
    (63 - v.clamp(0, 255) * 64 / 256).clamp(0, 63)
}

/// The graph: its background, the preamp's zero line, and the curve through the
/// ten bands as one colored column per x, the columns joined where the curve steps.
fn draw_classic_eq_graph(p: &mut Painter, preamp: i32, bands: &[i32]) {
    p.blit("eqmain.bmp", 0, 294, 113, 19, 86, 17);
    let zero = round_half_even(f64::from(preamp) * 19.0 / 64.0);
    p.blit("eqmain.bmp", 0, 314, 113, 1, 86, 35 - zero);
    // the points: the first band again before them, the last again after
    let mut points: Vec<f64> = Vec::with_capacity(12);
    points.push(f64::from(*bands.first().unwrap_or(&31)) * 19.0 / 64.0);
    for i in 0..10 {
        points.push(f64::from(*bands.get(i).unwrap_or(&31)) * 19.0 / 64.0);
    }
    points.push(*points.last().unwrap_or(&0.0));
    // one column per x from 88 to 196: Winamp draws one column past 195 and stops there
    let mut last: Option<i32> = None;
    for x in 88..=196 {
        let u = f64::from(x - 88) / 12.0 + 1.0;
        let y = round_half_even(spline(&points, u)).clamp(0, 18);
        let (top, height) = match last {
            Some(l) if l < y => (l, y - l + 1),
            Some(l) if l > y => (y, l - y + 1),
            _ => (y, 1),
        };
        p.blit("eqmain.bmp", 115, 294 + top, 1, height, x, 17 + top);
        last = Some(y);
    }
}

/// A Kochanek-Bartels spline, which lands on the pixels of Winamp's graph: tension 0,
/// continuity 0, bias 0.1, over `points` at `u` (point index plus the way to the next).
fn spline(points: &[f64], u: f64) -> f64 {
    let n = points.len() as i64;
    let i = u.floor() as i64;
    let t = u - u.floor();
    let at = |k: i64| points[k.rem_euclid(n) as usize];
    let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
    let (tension, continuity, bias) = (0.0f64, 0.0f64, 0.1f64);
    let fa = (1.0 - tension) * (1.0 + continuity) * (1.0 + bias);
    let fb = (1.0 - tension) * (1.0 - continuity) * (1.0 - bias);
    let fc = (1.0 - tension) * (1.0 - continuity) * (1.0 + bias);
    let fd = (1.0 - tension) * (1.0 + continuity) * (1.0 - bias);
    let d1 = 0.5 * (fa * (p1 - p0) + fb * (p2 - p1));
    let d2 = 0.5 * (fc * (p2 - p1) + fd * (p3 - p2));
    let (t2, t3) = (t * t, t * t * t);
    let h1 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h2 = -2.0 * t3 + 3.0 * t2;
    let h3 = t3 - 2.0 * t2 + t;
    let h4 = t3 - t2;
    h1 * p1 + h2 * p2 + h3 * d1 + h4 * d2
}

/// `v` rounded to the nearest, ties to even, as Winamp's graph lands on its pixels.
fn round_half_even(v: f64) -> i32 {
    let r = v.round();
    if (v - v.trunc()).abs() == 0.5 && r as i64 % 2 != 0 {
        (r - v.signum()) as i32
    } else {
        r as i32
    }
}
