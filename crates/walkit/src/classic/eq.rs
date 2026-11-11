// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic equalizer window, pixel for pixel as Winamp 5.666 paints it.

use crate::classic::main::Painter;
use crate::classic::touch::Control;
use crate::draw::{DrawCmd, Rect};
use crate::gui::ObjId;
use crate::runtime::SkinRuntime;

impl SkinRuntime {
    pub(crate) fn draw_classic_eq(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let mut p = Painter {
            at,
            out,
            sheets: self.classic_sheets.clone(),
        };
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
        // on and auto, the presets button, each from its pressed look while pressed
        let press = self.classic_press_on(o).map(|p| (p.control, p.inside));
        let pressed = |c: Control| press.is_some_and(|(pc, inside)| pc == c && inside);
        let on = self.player.eq_enabled();
        let on_px = if pressed(Control::EqOn) { 118 } else { 0 };
        let auto_px = if pressed(Control::EqAuto) { 118 } else { 0 };
        let on_sx = 10 + on_px + if on { 59 } else { 0 };
        p.blit("eqmain.bmp", on_sx, 119, 25, 12, 14, 18);
        let auto_sx = 35 + auto_px + if self.classic_eq_auto { 59 } else { 0 };
        p.blit("eqmain.bmp", auto_sx, 119, 33, 12, 39, 18);
        let presets_y = if pressed(Control::EqPresets) {
            176
        } else {
            164
        };
        p.blit("eqmain.bmp", 224, presets_y, 44, 12, 217, 18);
        if pressed(Control::Close) {
            p.blit("eqmain.bmp", 0, 125, 9, 9, 264, 3);
        }
        // the preamp and the ten bands: a slider frame by the value, the thumb on it
        let preamp = self.player.eq_preamp();
        let bands = self.player.eq_bands();
        Self::draw_classic_eq_slider(&mut p, 21, preamp, pressed(Control::EqSlider(0)));
        for (i, v) in bands.iter().take(10).enumerate() {
            let held = pressed(Control::EqSlider(i + 1));
            Self::draw_classic_eq_slider(&mut p, (i as i32 + 1) * 18 + 60, *v, held);
        }
        draw_classic_eq_graph(
            &mut p,
            band_step(preamp),
            &bands.iter().map(|v| band_step(*v)).collect::<Vec<_>>(),
        );
    }

    /// The equalizer's shade bar (drawn from `eq_ex.bmp`): the strip, with the
    /// volume mark 3x7 at 61 + v*94/255 from (1 + (v*3>>8)*3, 30) and the balance mark at
    /// 164 + (b+128)*39/255 from (11 + ((b+128)*3>>8)*3, 30).
    pub(crate) fn draw_classic_eq_shade(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let volume = self.player.volume().clamp(0, 255);
        let balance = self.player.pan().clamp(-127, 127) + 128;
        let mut p = Painter {
            at,
            out,
            sheets: self.classic_sheets.clone(),
        };
        p.blit("eq_ex.bmp", 0, if active { 0 } else { 15 }, 275, 14, 0, 0);
        p.blit(
            "eq_ex.bmp",
            1 + ((volume * 3) >> 8) * 3,
            30,
            3,
            7,
            61 + volume * 94 / 255,
            4,
        );
        p.blit(
            "eq_ex.bmp",
            11 + ((balance * 3) >> 8) * 3,
            30,
            3,
            7,
            164 + balance * 39 / 255,
            4,
        );
    }

    /// A band slider at `x`: the value `v` (0..255 as the player keeps a band, 128 flat, 255
    /// the top) picked as one of 28 frames of the slider's background, the thumb at its height.
    fn draw_classic_eq_slider(p: &mut Painter, x: i32, v: i32, held: bool) {
        let value = band_step(v);
        let frame = 27 - value * 28 / 64;
        if frame < 14 {
            p.blit("eqmain.bmp", frame * 15 + 13, 164, 14, 63, x, 38);
        } else {
            p.blit("eqmain.bmp", frame * 15 - 197, 229, 14, 63, x, 38);
        }
        let y = (value - 63) * 52 / 64 + 89;
        p.blit(
            "eqmain.bmp",
            0,
            if held { 176 } else { 164 },
            11,
            11,
            x + 1,
            y,
        );
    }
}

/// A band as Winamp's classic slider holds it, 0..63 with 0 the top, from the player's 0..255
/// (128 flat, 255 the top).
pub(crate) fn band_step(v: i32) -> i32 {
    (63 - v.clamp(0, 255) * 64 / 256).clamp(0, 63)
}

/// The graph: its background, the preamp's zero line, and the curve through the
/// ten bands as one colored column per x, the columns joined where the curve steps.
fn draw_classic_eq_graph(p: &mut Painter, preamp: i32, bands: &[i32]) {
    p.blit("eqmain.bmp", 0, 294, 113, 19, 86, 17);
    p.blit(
        "eqmain.bmp",
        0,
        314,
        113,
        1,
        86,
        17 + graph_preamp_row(preamp),
    );
    // one column per x from 88 to 196: Winamp draws one column past 195 and stops there
    let mut last: Option<i32> = None;
    for (x, y) in (88..).zip(graph_curve(bands)) {
        let (top, height) = match last {
            Some(l) if l < y => (l, y - l + 1),
            Some(l) if l > y => (y, l - y + 1),
            _ => (y, 1),
        };
        p.blit("eqmain.bmp", 115, 294 + top, 1, height, x, 17 + top);
        last = Some(y);
    }
}

/// The columns of the graph's curve.
const GRAPH_COLUMNS: usize = 109;

/// The row of the graph (0 the top of its 19) the preamp's line lies on: 19 rows over the
/// slider's 64 steps counted up from the bottom, the fraction dropped.
fn graph_preamp_row(preamp: i32) -> i32 {
    18 - (preamp * 19 / 64).clamp(0, 18)
}

/// The curve's row (0 the top) for each of its columns: a cardinal spline with tension 0.1
/// through the ten bands, twelve columns apart, the first and the last band standing in for
/// the neighbor they lack, each height cut down to a whole row. Probed in the VM over some
/// 500 equalizer states, every column of which this lands on.
fn graph_curve(bands: &[i32]) -> [i32; GRAPH_COLUMNS] {
    let band = |i: usize| f64::from(*bands.get(i.min(9)).unwrap_or(&31)) * 19.0 / 64.0;
    let mut rows = [0; GRAPH_COLUMNS];
    for (x, row) in rows.iter_mut().enumerate() {
        let i = (x / 12).min(8);
        let t = (x - i * 12) as f64 / 12.0;
        let (p0, p1, p2, p3) = (band(i.saturating_sub(1)), band(i), band(i + 1), band(i + 2));
        let d1 = 0.9 * (p2 - p0) / 2.0;
        let d2 = 0.9 * (p3 - p1) / 2.0;
        let (t2, t3) = (t * t, t * t * t);
        let h1 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h2 = -2.0 * t3 + 3.0 * t2;
        let h3 = t3 - 2.0 * t2 + t;
        let h4 = t3 - t2;
        let y = h1 * p1 + h2 * p2 + h3 * d1 + h4 * d2;
        *row = (y.floor() as i32).clamp(0, 18);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{graph_curve, graph_preamp_row};

    // every expectation here was read off Winamp 5.666's own graph, probed in the VM

    #[test]
    fn flat_bands_draw_a_level_line_a_row_per_64_19ths() {
        for step in 0..64 {
            let rows = graph_curve(&[step; 10]);
            assert!(rows.iter().all(|y| *y == step * 19 / 64), "{step}");
        }
        assert_eq!(graph_curve(&[0; 10])[0], 0);
        assert_eq!(graph_curve(&[3; 10])[0], 0);
        assert_eq!(graph_curve(&[4; 10])[0], 1);
        assert_eq!(graph_curve(&[31; 10])[0], 9);
        assert_eq!(graph_curve(&[63; 10])[0], 18);
    }

    #[test]
    fn the_curve_of_a_mixed_setting() {
        let rows = graph_curve(&[30, 16, 11, 6, 13, 29, 49, 51, 42, 16]);
        #[rustfmt::skip]
        let want = [
            8, 8, 8, 8, 7, 7, 6, 6, 6, 5, 5, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3,
            3, 3, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3,
            3, 4, 4, 4, 5, 5, 6, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14,
            14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 14, 14, 14, 14, 14, 14, 13, 13, 13, 12,
            12, 12, 11, 10, 10, 9, 8, 7, 6, 6, 5, 5, 4,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn one_band_down_dips_the_next_stretch_below_the_level() {
        let mut bands = [31; 10];
        bands[0] = 63;
        let rows = graph_curve(&bands);
        #[rustfmt::skip]
        let want = [18, 18, 17, 16, 15, 14, 13, 12, 11, 11, 10, 9, 9, 8, 8, 8, 8, 8, 8, 8, 8];
        assert_eq!(rows[..21], want);
        assert!(rows[21..].iter().all(|y| *y == 9));
    }

    #[test]
    fn alternating_extremes_swing_the_full_height_and_ease_at_both_ends() {
        let rows = graph_curve(&[0, 63, 0, 63, 0, 63, 0, 63, 0, 63]);
        #[rustfmt::skip]
        let want = [
            0, 0, 2, 4, 6, 8, 10, 12, 14, 16, 17, 18, 18, 18, 17, 15, 13, 11, 9, 7, 4, 2, 1, 0,
            0, 0, 1, 2, 4, 7, 9, 11, 13, 15, 17, 18, 18, 18, 17, 15, 13, 11, 9, 7, 4, 2, 1, 0,
            0, 0, 1, 2, 4, 7, 9, 11, 13, 15, 17, 18, 18, 18, 17, 15, 13, 11, 9, 7, 4, 2, 1, 0,
            0, 0, 1, 2, 4, 7, 9, 11, 13, 15, 17, 18, 18, 18, 17, 15, 13, 11, 9, 7, 4, 2, 1, 0,
            0, 0, 1, 2, 4, 6, 8, 10, 12, 14, 16, 17, 18,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn alternating_quarters() {
        let rows = graph_curve(&[16, 48, 16, 48, 16, 48, 16, 48, 16, 48]);
        #[rustfmt::skip]
        let want = [
            4, 5, 5, 6, 7, 8, 10, 11, 12, 12, 13, 14, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4,
            4, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4,
            4, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4,
            4, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4,
            4, 4, 5, 6, 6, 7, 8, 10, 11, 12, 13, 13, 14,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn the_preamp_line_drops_the_fraction() {
        let rows: Vec<i32> = [0, 3, 4, 28, 31, 32, 60, 61, 63]
            .iter()
            .map(|p| graph_preamp_row(*p))
            .collect();
        assert_eq!(rows, [18, 18, 17, 10, 9, 9, 1, 0, 0]);
    }
}
