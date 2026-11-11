// SPDX-License-Identifier: GPL-3.0-or-later

//! The pictures the engine paints pixel by pixel: the analyzer and oscilloscope, the EQ curve,
//! and gradients.

use std::rc::Rc;

use super::SkinRuntime;
use crate::classic::eq::band_step;
use crate::draw::{DrawCmd, Gamma, Rgb};
use crate::gui::{parse_rgb, to_bool, to_int_lenient, Kind, ObjId, ScopeStyle};
use crate::player::Status;

const WIDTH: usize = 72;
const ROWS: usize = 16;
const COLUMNS: usize = 75;
const MAX_BAR: i32 = 15;
const FRAME_MS: i64 = 25;
const MAX_FRAMES: i64 = 200;
const PEAK_START_SPEED: f32 = 3.0;
const OPAQUE: u32 = 0xff00_0000;
const FALLOFF_STEPS: [i32; 5] = [3, 6, 12, 16, 32];
const PEAK_ACCEL: [f32; 5] = [1.05, 1.1, 1.2, 1.4, 1.6];
/// The oscilloscope's row for silence.
const SCOPE_MIDDLE: i32 = 8;
/// Rows per unit of sample: an 8-bit sample (128 to the unit) over 8.
const SCOPE_ROWS_PER_UNIT: f32 = 16.0;
/// Points across the 576 samples at one per four; the picture shows the first `WIDTH`.
const SCOPE_POINTS: i64 = 144;
const SCOPE_COLORS: usize = 5;
/// The most pixels a made picture may hold.
pub(crate) const MAX_PIXELS: i64 = 16 * 1024 * 1024;
const NO_STOPS: u32 = 0xffff_00ff;

/// `px` (`w` x `h`) stretched to `dw` x `dh` as Winamp stretches the vis picture: target pixel
/// x takes source column floor(x * w / dw).
pub fn stretch(px: &[u32], w: usize, h: usize, dw: usize, dh: usize) -> Vec<u32> {
    if dw == w && dh == h {
        return px.to_vec();
    }
    let mut out = vec![0u32; dw * dh];
    for y in 0..dh {
        let sy = y * h / dh;
        for x in 0..dw {
            out[y * dw + x] = px[sy * w + x * w / dw];
        }
    }
    out
}

/// `px` mirrored left to right and/or top to bottom.
pub fn flip(px: &[u32], w: usize, h: usize, horizontal: bool, vertical: bool) -> Vec<u32> {
    if !horizontal && !vertical {
        return px.to_vec();
    }
    let mut out = vec![0u32; px.len()];
    for y in 0..h {
        for x in 0..w {
            let sx = if horizontal { w - 1 - x } else { x };
            let sy = if vertical { h - 1 - y } else { y };
            out[y * w + x] = px[sy * w + sx];
        }
    }
    out
}

/// `from` moved on by the whole 25 ms frames up to `now`; `now` itself for a first frame, or
/// when the clock went back.
fn on_grid(from: i64, now: i64) -> i64 {
    if from < 0 || now < from {
        now
    } else {
        from + (now - from) / FRAME_MS * FRAME_MS
    }
}

impl SkinRuntime {
    fn vis(&self, o: ObjId) -> &crate::gui::VisData {
        match &self.objects[o].kind {
            Kind::Vis(v) => v,
            _ => panic!("not a vis"),
        }
    }

    fn vis_mut(&mut self, o: ObjId) -> &mut crate::gui::VisData {
        match &mut self.objects[o].kind {
            Kind::Vis(v) => v,
            _ => panic!("not a vis"),
        }
    }

    /// A vis's attributes beyond the base ones.
    pub(crate) fn vis_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        let v = self.vis_mut(o);
        if k == "colorallbands" {
            if let Some(c) = parse_rgb(value) {
                v.rows = [c; ROWS];
            }
        } else if let Some(n) = k.strip_prefix("colorband").filter(|_| k != "colorbandpeak") {
            if let (Ok(n), Some(c)) = (n.parse::<usize>(), parse_rgb(value)) {
                if (1..=ROWS).contains(&n) {
                    v.rows[ROWS - n] = c;
                }
            }
        } else if k == "colorbandpeak" {
            if let Some(c) = parse_rgb(value) {
                v.peak_color = c;
            }
        } else if k == "colorallosc" {
            if let Some(c) = parse_rgb(value) {
                v.scope_colors = [c; SCOPE_COLORS];
            }
        } else if let Some(n) = k.strip_prefix("colorosc") {
            if let (Ok(n), Some(c)) = (n.parse::<usize>(), parse_rgb(value)) {
                if (1..=SCOPE_COLORS).contains(&n) {
                    v.scope_colors[n - 1] = c;
                }
            }
        } else if k == "oscstyle" {
            match value.trim().to_lowercase().as_str() {
                "dots" => v.scope_style = ScopeStyle::Dots,
                "solid" => v.scope_style = ScopeStyle::Line,
                "lines" => v.scope_style = ScopeStyle::Filled,
                _ => {}
            }
        } else if k == "gammagroup" {
            v.gamma_group = Some(value.to_string());
        } else if k == "mode" {
            v.mode = to_int_lenient(value)
                .filter(|m| (0..=2).contains(m))
                .unwrap_or(0);
        } else if k == "peaks" {
            v.peaks = to_bool(value);
        } else if k == "bandwidth" {
            v.thin = value.eq_ignore_ascii_case("thin");
        } else if k == "flipv" {
            v.flip_v = to_bool(value);
        } else if k == "fliph" {
            v.flip_h = to_bool(value);
        } else if k == "falloff" {
            v.falloff = to_int_lenient(value).map(|f| f.clamp(0, 4)).unwrap_or(2);
        } else if k == "peakfalloff" {
            v.peak_falloff = to_int_lenient(value).map(|f| f.clamp(0, 4)).unwrap_or(1);
        } else if k.starts_with("color") || k == "coloring" || k == "fps" {
        } else {
            return false;
        }
        true
    }

    /// Whether a vis's picture still moves with the player in `status`: while the music plays,
    /// and after it stops or pauses until the bars and peaks have fallen to the bottom, or the
    /// oscilloscope has drawn its flat line once. A mode that shows nothing never moves.
    pub(crate) fn vis_moving(&self, o: ObjId, status: Status) -> bool {
        let v = self.vis(o);
        if !(1..=2).contains(&v.mode) {
            false
        } else if status == Status::Playing {
            true
        } else if v.mode == 2 {
            !v.scope_flat
        } else {
            v.bars.iter().any(|b| *b > 0.0) || v.peak.iter().any(|p| *p > 0.0)
        }
    }

    fn vis_moved_at(&self, o: ObjId) -> i64 {
        let v = self.vis(o);
        if v.mode == 2 {
            v.scope_at
        } else {
            v.last_frame_at
        }
    }

    /// Whether a 25 ms step has passed since the picture last moved on, at `now` (or the clock went back).
    pub(crate) fn vis_step_due(&self, o: ObjId, now: i64) -> bool {
        let moved = self.vis_moved_at(o);
        moved < 0 || now < moved || now - moved >= FRAME_MS
    }

    /// When the next 25 ms step is due; `now` when one is due already.
    pub(crate) fn vis_next_step_at(&self, o: ObjId, now: i64) -> i64 {
        if self.vis_step_due(o, now) {
            now
        } else {
            self.vis_moved_at(o) + FRAME_MS
        }
    }

    fn vis_tint(&mut self, group: Option<&str>, c: Rgb) -> Rgb {
        match self.gamma(group) {
            Some(g) => Rgb(g.apply(c.0 | OPAQUE) & 0xffffff),
            None => c,
        }
    }

    /// Advances bars and peaks by the 25 ms frames since the last one, which stay on a 25 ms grid.
    fn vis_step(&mut self, o: ObjId, levels: &[f32]) {
        let now = self.now();
        let v = self.vis_mut(o);
        let frames = if v.last_frame_at < 0 || now < v.last_frame_at {
            1
        } else {
            ((now - v.last_frame_at) / FRAME_MS).min(MAX_FRAMES)
        };
        if frames == 0 {
            return;
        }
        v.last_frame_at = if frames == MAX_FRAMES {
            now
        } else {
            on_grid(v.last_frame_at, now)
        };
        let fall = FALLOFF_STEPS[v.falloff as usize] as f32 / 16.0;
        let accel = PEAK_ACCEL[v.peak_falloff as usize];
        for _ in 0..frames {
            for i in 0..COLUMNS {
                let target = if levels.is_empty() {
                    0.0
                } else {
                    (levels[i * levels.len() / COLUMNS] * ROWS as f32).min(MAX_BAR as f32)
                };
                v.bars[i] = if target >= v.bars[i] {
                    target
                } else {
                    target.max(v.bars[i] - fall)
                };
                let bar_peak = v.bars[i] * 256.0;
                if v.peak[i] <= bar_peak {
                    v.peak[i] = bar_peak;
                    v.speed[i] = PEAK_START_SPEED;
                } else {
                    v.peak[i] = bar_peak.max(v.peak[i] - v.speed[i] as i32 as f32);
                    v.speed[i] *= accel;
                }
            }
        }
    }

    /// A vis's drawing: a 72 x 16 picture stretched over the object.
    pub(crate) fn draw_vis(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if rect.width <= 0 || rect.height <= 0 || !(1..=2).contains(&self.vis(o).mode) {
            return;
        }
        let mut px = vec![0u32; WIDTH * ROWS];
        if self.vis(o).mode == 2 {
            self.paint_scope(o, &mut px);
        } else {
            self.paint_spectrum(o, &mut px);
        }
        if rect.width as i64 * rect.height as i64 > MAX_PIXELS {
            return;
        }
        let v = self.vis(o);
        let flipped = flip(&px, WIDTH, ROWS, v.flip_h, v.flip_v);
        let scaled = stretch(
            &flipped,
            WIDTH,
            ROWS,
            rect.width as usize,
            rect.height as usize,
        );
        out.push(DrawCmd::Pixels {
            rect,
            w: rect.width,
            h: rect.height,
            argb: Rc::from(scaled),
            alpha,
            stable: false,
        });
    }

    /// Steps the bars and peaks on and paints them into `px` (72 x 16).
    fn paint_spectrum(&mut self, o: ObjId, px: &mut [u32]) {
        let levels = self.player.spectrum();
        self.vis_step(o, &levels);
        let group = self.vis(o).gamma_group.clone();
        let rows = self.vis(o).rows;
        let mut colors = [0u32; ROWS];
        for (i, c) in rows.iter().enumerate() {
            colors[i] = self.vis_tint(group.as_deref(), *c).0 | OPAQUE;
        }
        let peak_color = self.vis(o).peak_color;
        let dot = self.vis_tint(group.as_deref(), peak_color).0 | OPAQUE;
        let v = self.vis(o);
        let column = |px: &mut [u32], x: usize, h: i32, p: i32| {
            for r in 0..h.max(0) as usize {
                if r < ROWS {
                    px[(ROWS - 1 - r) * WIDTH + x] = colors[ROWS - 1 - r];
                }
            }
            if v.peaks {
                px[(MAX_BAR - p.clamp(0, MAX_BAR)) as usize * WIDTH + x] = dot;
            }
        };
        if v.thin {
            for x in 0..WIDTH {
                column(px, x, v.bars[x] as i32, (v.peak[x] / 256.0) as i32);
            }
        } else {
            for b in 0..WIDTH / 4 {
                // a bar is the mean of its four data columns; the fourth column stays empty
                let h = ((0..4).map(|i| v.bars[b * 4 + i] as f64).sum::<f64>() / 4.0) as i32;
                let p =
                    ((0..4).map(|i| v.peak[b * 4 + i] as f64).sum::<f64>() / 4.0 / 256.0) as i32;
                for x in b * 4..b * 4 + 3 {
                    column(px, x, h, p);
                }
            }
        }
    }

    /// Paints the oscilloscope into `px` (72 x 16): a point per column from every fourth of the
    /// 576 samples Winamp hands a visualizer.
    fn paint_scope(&mut self, o: ObjId, px: &mut [u32]) {
        let wave = self.player.waveform();
        let now = self.now();
        let group = self.vis(o).gamma_group.clone();
        let scope_colors = self.vis(o).scope_colors;
        let mut colors = [0u32; SCOPE_COLORS];
        for (i, c) in scope_colors.iter().enumerate() {
            colors[i] = self.vis_tint(group.as_deref(), *c).0 | OPAQUE;
        }
        let v = self.vis_mut(o);
        v.scope_at = on_grid(v.scope_at, now);
        v.scope_flat = wave.is_empty();
        let style = v.scope_style;
        let mut last: i32 = -1;
        for x in 0..WIDTH {
            let sample = if wave.is_empty() {
                0.0
            } else {
                wave[(x as i64 * wave.len() as i64 / SCOPE_POINTS) as usize]
            };
            let row = ((sample.clamp(-1.0, 1.0) * SCOPE_ROWS_PER_UNIT).floor() as i32
                + SCOPE_MIDDLE)
                .clamp(0, ROWS as i32 - 1);
            let mut from = row;
            let mut to = row;
            match style {
                ScopeStyle::Dots => {}
                // from the last point down to this one; going up, from this one down to the row above the last
                ScopeStyle::Line => {
                    if last >= 0 && last < row {
                        from = last;
                    } else if row < last {
                        to = last - 1;
                    }
                }
                ScopeStyle::Filled => {
                    if row < SCOPE_MIDDLE {
                        to = SCOPE_MIDDLE - 1;
                    } else {
                        from = SCOPE_MIDDLE;
                    }
                }
            }
            let c = colors[(row / 2 - SCOPE_MIDDLE / 2).unsigned_abs() as usize];
            for r in from..=to {
                px[r as usize * WIDTH + x] = c;
            }
            last = row;
        }
    }

    /// A vis's script methods.
    pub(crate) fn vis_maki_call(
        &mut self,
        o: ObjId,
        name: &str,
        args: &[crate::maki::Value],
    ) -> Option<crate::maki::Value> {
        use crate::maki::Value;
        Some(match name {
            "setmode" => {
                let m = args.first().map(|a| a.as_int()).unwrap_or(0);
                self.vis_mut(o).mode = if (0..=2).contains(&m) { m } else { 0 };
                self.repaint();
                Value::Null
            }
            "getmode" => Value::Int(self.vis(o).mode),
            "nextmode" => {
                let v = self.vis_mut(o);
                v.mode = if v.mode >= 2 { 0 } else { v.mode + 1 };
                self.repaint();
                Value::Null
            }
            _ => return None,
        })
    }

    // ---- the EQ curve ------------------------------------------------------------------------

    /// An eqvis's attributes beyond the base ones.
    pub(crate) fn eqvis_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        let Kind::EqVis(e) = &mut self.objects[o].kind else {
            return false;
        };
        match k {
            "colortop" => {
                if let Some(c) = parse_rgb(value) {
                    e.top = c;
                }
            }
            "colormiddle" => {
                if let Some(c) = parse_rgb(value) {
                    e.middle = c;
                }
            }
            "colorbottom" => {
                if let Some(c) = parse_rgb(value) {
                    e.bottom = c;
                }
            }
            "colorpreamp" => {
                if let Some(c) = parse_rgb(value) {
                    e.preamp = c;
                }
            }
            // on an eqvis, "gamma" is the curve's opacity (Winamp3: gamma="192")
            "gamma" => self.objects[o].base.set_alpha(
                to_int_lenient(value)
                    .map(|a| a.clamp(0, 255))
                    .unwrap_or(255),
            ),
            "gammagroup" => e.gamma_group = Some(value.to_string()),
            _ => return k.starts_with("color"),
        }
        true
    }

    /// The EQ curve: a spline through the ten bands (see `eq_curve_rows`), each row in a color
    /// blended from top to middle to bottom, the preamp's line first as a full row.
    pub(crate) fn draw_eqvis(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if rect.width <= 0
            || rect.height <= 0
            || rect.width as i64 * rect.height as i64 > MAX_PIXELS
        {
            return;
        }
        let Kind::EqVis(e) = self.objects[o].kind.clone() else {
            return;
        };
        let (w, h) = (rect.width as usize, rect.height as usize);
        let half = (h / 2).max(1);
        let t0 = self.vis_tint(e.gamma_group.as_deref(), e.top);
        let m0 = self.vis_tint(e.gamma_group.as_deref(), e.middle);
        let b0 = self.vis_tint(e.gamma_group.as_deref(), e.bottom);
        let mix = |a: Rgb, b: Rgb, t: f32| -> u32 {
            let ch = |v: u32, s: u32| ((v >> s) & 0xff) as i32;
            let lerp = |s: u32| {
                ((ch(a.0, s) as f32 + (ch(b.0, s) - ch(a.0, s)) as f32 * t) as i32).clamp(0, 255)
                    as u32
            };
            OPAQUE | (lerp(16) << 16) | (lerp(8) << 8) | lerp(0)
        };
        let row_color: Vec<u32> = (0..h)
            .map(|r| {
                if r < half {
                    mix(t0, m0, r as f32 / half as f32)
                } else {
                    mix(m0, b0, (r - half) as f32 / (h - half).max(1) as f32)
                }
            })
            .collect();
        let mut px = vec![0u32; w * h];
        let pre_row = eq_preamp_row(band_step(self.player.eq_preamp()), h);
        let pc = self.vis_tint(e.gamma_group.as_deref(), e.preamp).0 | OPAQUE;
        px[pre_row * w..(pre_row + 1) * w].fill(pc);
        let steps: Vec<i32> = self.player.eq_bands().into_iter().map(band_step).collect();
        let mut prev: Option<usize> = None;
        for (x, y) in eq_curve_rows(&steps, w, h).into_iter().enumerate() {
            let from = prev.unwrap_or(y);
            for r in from.min(y)..=from.max(y) {
                px[r * w + x] = row_color[r];
            }
            prev = Some(y);
        }
        out.push(DrawCmd::Pixels {
            rect,
            w: rect.width,
            h: rect.height,
            argb: Rc::from(px),
            alpha,
            stable: false,
        });
    }

    // ---- gradients ---------------------------------------------------------------------------

    /// A gradient's attributes beyond the base ones.
    pub(crate) fn gradient_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        let Kind::Gradient(g) = &mut self.objects[o].kind else {
            return false;
        };
        g.made = None;
        match k {
            "gradient_x1" => g.x1 = value.parse().unwrap_or(0.0),
            "gradient_y1" => g.y1 = value.parse().unwrap_or(0.0),
            "gradient_x2" => g.x2 = value.parse().unwrap_or(0.0),
            "gradient_y2" => g.y2 = value.parse().unwrap_or(0.0),
            "points" => g.stops = parse_stops(value),
            "gammagroup" => g.gamma_group = Some(value.to_string()),
            _ => return false,
        }
        true
    }

    /// A gradient's drawing: the stops blended along the line, made once per size and tint.
    pub(crate) fn draw_gradient(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        let (w, h) = (rect.width, rect.height);
        if w <= 0 || h <= 0 || w as i64 * h as i64 > MAX_PIXELS {
            return;
        }
        let Kind::Gradient(g) = self.objects[o].kind.clone() else {
            return;
        };
        let gamma = self
            .gamma(g.gamma_group.as_deref())
            .filter(|g| !g.is_identity());
        let key = (w, h, gamma);
        if let (Some(made), Some(made_for)) = (&g.made, g.made_for) {
            if made_for == key {
                out.push(DrawCmd::Pixels {
                    rect,
                    w,
                    h,
                    argb: made.clone(),
                    alpha,
                    stable: true,
                });
                return;
            }
        }
        // no stops at all: Winamp's magenta
        let mut argb = vec![NO_STOPS; (w * h) as usize];
        if !g.stops.is_empty() {
            paint_gradient(&g, &mut argb, w, h, gamma);
        }
        let made: Rc<[u32]> = Rc::from(argb);
        if let Kind::Gradient(gd) = &mut self.objects[o].kind {
            gd.made = Some(made.clone());
            gd.made_for = Some(key);
        }
        out.push(DrawCmd::Pixels {
            rect,
            w,
            h,
            argb: made,
            alpha,
            stable: true,
        });
    }
}

fn parse_stops(value: &str) -> Vec<(f32, u32)> {
    let mut stops: Vec<(f32, u32)> = value
        .split(';')
        .filter_map(|stop| {
            let (pos, color) = stop.split_once('=')?;
            if color.contains('=') {
                return None;
            }
            let c: Vec<i32> = color
                .split(',')
                .map(|p| p.trim().parse().unwrap_or(0))
                .collect();
            let ch = |i: usize, d: i32| c.get(i).copied().unwrap_or(d).clamp(0, 255) as u32;
            let argb = (ch(3, 255) << 24) | (ch(0, 0) << 16) | (ch(1, 0) << 8) | ch(2, 0);
            let p: f32 = pos.trim().parse().ok()?;
            Some((p, argb))
        })
        .collect();
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    stops
}

/// The color at `t` along the line: the stops' colors blended channel by channel.
fn color_at(stops: &[(f32, u32)], t: f32) -> u32 {
    let first = stops[0];
    if t <= first.0 || stops.len() == 1 {
        return first.1;
    }
    let last = stops[stops.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    let i = stops.iter().rposition(|s| s.0 <= t).unwrap_or(0);
    let (p0, c0) = stops[i];
    let (p1, c1) = stops[i + 1];
    let f = if p1 > p0 { (t - p0) / (p1 - p0) } else { 0.0 };
    let ch = |s: u32| -> u32 {
        let a = ((c0 >> s) & 0xff) as i32;
        let b = ((c1 >> s) & 0xff) as i32;
        ((a as f32 + (b - a) as f32 * f) as i32).clamp(0, 255) as u32
    };
    (ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Fills `argb` (`w` x `h`) with the stops blended along the line, tinted by `gamma`.
fn paint_gradient(
    g: &crate::gui::GradientData,
    argb: &mut [u32],
    w: i32,
    h: i32,
    gamma: Option<Gamma>,
) {
    let ax = g.x1 * w as f32;
    let ay = g.y1 * h as f32;
    let dx = g.x2 * w as f32 - ax;
    let dy = g.y2 * h as f32 - ay;
    let len2 = dx * dx + dy * dy;
    for y in 0..h {
        for x in 0..w {
            // where the pixel falls along the line, 0 at its start and 1 at its end
            let t = if len2 > 0.0 {
                ((x as f32 - ax) * dx + (y as f32 - ay) * dy) / len2
            } else {
                0.0
            };
            let c = color_at(&g.stops, t);
            argb[(y * w + x) as usize] = gamma.map(|g| g.apply(c)).unwrap_or(c);
        }
    }
}

/// The row (0 the top) of an EQ curve `h` rows high that the preamp's line lies on, from the
/// preamp's step (0..=63, 0 the top of its slider): the steps counted up from the bottom, over
/// 64 to the height, the fraction dropped. A boost lies low, as on the classic graph.
fn eq_preamp_row(step: i32, h: usize) -> usize {
    let up = (63 - step.clamp(0, 63)) as usize;
    (up * h / 64).min(h.saturating_sub(1))
}

/// The EQ curve's row (0 the top) in each of its `w` columns, `h` rows high, from the bands'
/// steps (0..=63, 0 the top): the classic graph's cardinal spline with tension 0.1 (a knot's
/// tangent 0.9 * (next - previous) / 2) through the ten bands at height (step + 0.5) * h / 64,
/// knots 0.11 * w columns apart from the first column, the first and the last band standing in
/// for the neighbor they lack, past the last band still on the spline toward its repeat; each
/// height cut down to a whole row. Probed in the VM over 138 equalizer states on curves 230x64,
/// 115x15, 113x19 and 67x13: all but three pixels of the 230 wide one's last column land.
fn eq_curve_rows(steps: &[i32], w: usize, h: usize) -> Vec<usize> {
    let height = |band: usize| {
        let step = steps.get(band.min(9)).copied().unwrap_or(31).clamp(0, 63);
        (f64::from(step) + 0.5) * h as f64 / 64.0
    };
    // knot k is band k - 1: band 0 again before the first, band 9 again after the last
    let knots: [f64; 12] = std::array::from_fn(|k| height(k.saturating_sub(1)));
    let tangent = |k: usize| 0.9 * (knots[(k + 1).min(11)] - knots[k.saturating_sub(1)]) / 2.0;
    let span = 0.11 * w as f64;
    let bottom = h.saturating_sub(1);
    (0..w)
        .map(|x| {
            let t = x as f64 / span + 1.0;
            let i = (t.floor() as usize).min(10);
            let u = t - i as f64;
            let (u2, u3) = (u * u, u * u * u);
            let y = (2.0 * u3 - 3.0 * u2 + 1.0) * knots[i]
                + (u3 - 2.0 * u2 + u) * tangent(i)
                + (-2.0 * u3 + 3.0 * u2) * knots[i + 1]
                + (u3 - u2) * tangent(i + 1);
            (y.floor().max(0.0) as usize).min(bottom)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{eq_curve_rows, eq_preamp_row};

    // every expectation here was read off Winamp 5.666's own EQ curves, probed in the VM on
    // a modern skin with curves 230x64, 115x15, 113x19 and 67x13

    #[test]
    fn flat_bands_draw_a_level_line_at_a_step_and_a_half_over_64() {
        for step in 0..64 {
            for (w, h) in [(230, 64), (115, 15), (113, 19), (67, 13)] {
                let rows = eq_curve_rows(&[step; 10], w, h);
                let want = ((2 * step + 1) as usize * h / 128).min(h - 1);
                assert!(rows.iter().all(|y| *y == want), "{step} on {w}x{h}");
            }
        }
        // the steps 21 and 42 on 15 rows: 4.9 and 9.8 down without the half step
        assert_eq!(eq_curve_rows(&[21; 10], 115, 15)[0], 5);
        assert_eq!(eq_curve_rows(&[42; 10], 115, 15)[0], 9);
        assert_eq!(eq_curve_rows(&[63; 10], 67, 13)[0], 12);
    }

    #[test]
    fn the_preamp_line_counts_up_from_the_bottom_so_a_boost_lies_low() {
        assert_eq!(eq_preamp_row(0, 64), 63);
        assert_eq!(eq_preamp_row(31, 64), 32);
        assert_eq!(eq_preamp_row(63, 64), 0);
        let fifteen: Vec<usize> = [0, 4, 8, 16, 20, 24, 28, 31, 36, 56, 60]
            .iter()
            .map(|s| eq_preamp_row(*s, 15))
            .collect();
        assert_eq!(fifteen, [14, 13, 12, 11, 10, 9, 8, 7, 6, 1, 0]);
        let nineteen: Vec<usize> = [0, 16, 44, 60]
            .iter()
            .map(|s| eq_preamp_row(*s, 19))
            .collect();
        assert_eq!(nineteen, [18, 13, 5, 0]);
    }

    #[test]
    fn one_band_up_peaks_on_its_knot_every_0_11_of_the_width() {
        for (w, h) in [(230, 64), (115, 15), (67, 13)] {
            for band in 1..9 {
                let mut steps = [31; 10];
                steps[band] = 0;
                let rows = eq_curve_rows(&steps, w, h);
                let top = *rows.iter().min().unwrap();
                let peak: Vec<usize> = (0..w).filter(|x| rows[*x] == top).collect();
                let knot = (band as f64 * 0.11 * w as f64).round() as usize;
                assert!(peak.contains(&knot), "band {band} on {w}: {peak:?}");
            }
        }
    }

    #[test]
    fn alternating_extremes_on_115x15() {
        let rows = eq_curve_rows(&[0, 63, 0, 63, 0, 63, 0, 63, 0, 63], 115, 15);
        #[rustfmt::skip]
        let want = [
            0, 0, 1, 3, 4, 6, 7, 9, 10, 12, 13, 14, 14, 14, 14, 13, 12, 10, 9, 7, 5, 4, 2, 1,
            0, 0, 0, 0, 1, 3, 4, 6, 8, 9, 11, 12, 13, 14, 14, 14, 13, 12, 11, 9, 7, 6, 4, 3,
            1, 0, 0, 0, 0, 1, 2, 4, 5, 7, 9, 11, 12, 13, 14, 14, 14, 14, 13, 11, 10, 8, 6, 5,
            3, 2, 1, 0, 0, 0, 1, 2, 3, 5, 7, 8, 10, 12, 13, 14, 14, 14, 14, 13, 12, 10, 9, 7,
            5, 3, 2, 1, 0, 0, 0, 0, 1, 2, 4, 5, 7, 9, 10, 12, 13, 14, 14,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn a_mixed_setting_on_67x13() {
        let rows = eq_curve_rows(&[27, 3, 32, 27, 37, 30, 41, 33, 53, 16], 67, 13);
        #[rustfmt::skip]
        let want = [
            5, 5, 4, 3, 2, 1, 1, 0, 0, 1, 2, 3, 4, 5, 6, 6, 6, 6, 6, 6, 5, 5, 5, 5,
            5, 6, 6, 7, 7, 7, 7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 8, 7,
            7, 7, 6, 6, 6, 7, 8, 8, 9, 10, 10, 10, 10, 9, 8, 6, 5, 4, 3,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn a_mixed_setting_on_113x19() {
        let rows = eq_curve_rows(&[32, 47, 8, 50, 49, 9, 46, 54, 35, 6], 113, 19);
        #[rustfmt::skip]
        let want = [
            9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 14, 13, 13, 12, 11, 10, 8, 7, 6, 4, 3, 3,
            2, 2, 2, 3, 4, 5, 6, 8, 9, 10, 12, 13, 14, 14, 15, 15, 15, 16, 16, 16, 16, 16, 15, 15,
            15, 14, 14, 13, 12, 11, 10, 9, 8, 6, 5, 4, 3, 3, 2, 2, 3, 3, 4, 5, 7, 8, 9, 10,
            11, 12, 13, 14, 14, 14, 15, 15, 15, 15, 16, 16, 16, 16, 16, 16, 16, 15, 15, 15, 14, 14, 13, 13,
            12, 11, 11, 10, 10, 9, 8, 8, 7, 6, 5, 4, 4, 3, 2, 2, 1,
        ];
        assert_eq!(rows, want);
    }

    #[test]
    fn past_the_last_band_the_curve_goes_on_toward_its_repeat() {
        // band 9 sits at column 227.7 of 230; the last two columns carry on the slope
        let mut steps = [31; 10];
        steps[9] = 0;
        let rows = eq_curve_rows(&steps, 230, 64);
        assert_eq!(rows[227..], [0, 0, 0]);
        steps[8] = 0;
        steps[9] = 31;
        let rows = eq_curve_rows(&steps, 230, 64);
        assert_eq!(rows[228..], [31, 32]);
    }
}
