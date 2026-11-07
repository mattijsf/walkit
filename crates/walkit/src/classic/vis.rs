// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic main window's analyzer and oscilloscope, as Winamp paints them
//! (in 5.666) into a 76 x 16 picture over the viscolor palette: the dot grid on
//! the odd rows and even columns; the analyzer's thick bars three columns wide with the fourth
//! empty, each bar's base two rows under the picture so its bottom row carries color 15 and each
//! row up one color less, and peak marks in color 23, with falloff, peaks and their speeds as
//! Winamp's settings have them; the oscilloscope's 75 samples as a line, each sample's row 17
//! less its 0..15 value (the two lowest rows under the picture), colored 18 to 22 by its distance
//! from the middle, each column a run from the sample before it (measured on Winamp 5.666 in a VM
//! with white noise and sine tones, 7 Nov 2026).

use std::rc::Rc;

use crate::classic::main::Painter;
use crate::draw::{DrawCmd, Rect};

/// The picture's width.
pub const WIDTH: usize = 76;
/// The picture's height.
pub const HEIGHT: usize = 16;
/// The shade bar's strip's width; the oscilloscope fills all 38 columns.
pub const SHADE_WIDTH: usize = 38;
/// The shade bar's strip's height.
pub const SHADE_HEIGHT: usize = 5;
/// The columns the shade bar's analyzer fills.
const SHADE_BANDS: usize = 37;
/// The vis area's color while the vis is off: the palette's background.
pub const OFF: u8 = 0;
/// The bands Winamp hands the analyzer.
const BANDS: usize = 75;
/// Winamp's bar base: this many rows under the picture's bottom row.
const BASE_BELOW: i32 = 2;
/// A frame of the analyzer, in ms; also how often a playing classic window is redrawn.
pub(crate) const FRAME_MS: i64 = 25;
const MAX_FRAMES: i64 = 200;
/// Falloff per frame, by the analyzer falloff setting (`safalloff` 2).
const FALLOFF: i32 = 12;
/// A fresh peak's speed and how it grows each frame (`sa_peak_falloff` 1).
const PEAK_SPEED: f32 = 3.0;
const PEAK_ACCEL: f32 = 1.1;

/// What the main window's vis area shows; a press on it goes to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisMode {
    /// The spectrum analyzer.
    #[default]
    Analyzer,
    /// The oscilloscope.
    Scope,
    /// Nothing.
    Off,
}

impl VisMode {
    /// The mode a press on the vis area switches to.
    #[must_use]
    pub fn next(self) -> VisMode {
        match self {
            VisMode::Analyzer => VisMode::Scope,
            VisMode::Scope => VisMode::Off,
            VisMode::Off => VisMode::Analyzer,
        }
    }
}

/// How the analyzer colors its bars (Winamp's "safire" bits 0 and 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalyzerStyle {
    /// Each row a color down from 17 at the bottom.
    #[default]
    Normal,
    /// The bar's top in color 3, each row below one more: a tall bar burns brighter.
    Fire,
    /// The whole bar in one color by its height, 17 less the height.
    Line,
}

/// How the oscilloscope draws its samples (Winamp's "safire" bits 2 and 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScopeStyle {
    /// A pixel per sample.
    Dots,
    /// Each column a run from the sample before's row.
    #[default]
    Lines,
    /// Each column a run from the middle row.
    Solid,
}

/// Winamp's visualization options as the classic vis area shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisStyle {
    /// The analyzer's coloring.
    pub analyzer: AnalyzerStyle,
    /// Thin bars: a band per column with no gap, instead of bars three wide over four bands.
    pub thin_bars: bool,
    /// Peak marks over the bars.
    pub peaks: bool,
    /// The oscilloscope's drawing.
    pub scope: ScopeStyle,
}

impl Default for VisStyle {
    /// Winamp's defaults: normal thick bars with peaks, the oscilloscope in lines.
    fn default() -> Self {
        VisStyle {
            analyzer: AnalyzerStyle::Normal,
            thin_bars: false,
            peaks: true,
            scope: ScopeStyle::Lines,
        }
    }
}

/// The dot grid alone.
fn grid() -> [u8; WIDTH * HEIGHT] {
    let mut px = [0u8; WIDTH * HEIGHT];
    for y in (1..HEIGHT).step_by(2) {
        for x in (0..WIDTH).step_by(2) {
            px[y * WIDTH + x] = 1;
        }
    }
    px
}

/// The oscilloscope over `wave` (samples -1..1, the window Winamp hands a visualizer): 75
/// columns, each the sample at its share of the window scaled to 0..15 (16 per unit, 8 the
/// middle), drawn as a run from the previous column's row to this column's row, in the color of
/// its distance from the middle; digital silence draws no line, as Winamp shows none.
pub fn scope(wave: &[f32], style: ScopeStyle) -> [u8; WIDTH * HEIGHT] {
    let mut px = grid();
    if wave.is_empty() || wave.iter().all(|s| *s == 0.0) {
        return px;
    }
    let mut prev: Option<i32> = None;
    for col in 0..BANDS {
        let sample = wave[col * wave.len() / BANDS];
        let v = ((sample * 32767.0) as i32) >> 11;
        let s = (v + 8).clamp(0, 15);
        let color = ((s / 2) - 4).unsigned_abs() as u8 + 18;
        let from = prev.unwrap_or(s);
        // lines: Winamp runs from the sample's row to the one before's, leaving that one
        // out when the line goes down the picture; solid: to the middle rows 7 and 8
        let (lo, hi) = match style {
            ScopeStyle::Dots => (s, s),
            ScopeStyle::Lines if s < from => (s, from - 1),
            ScopeStyle::Lines => (from, s),
            ScopeStyle::Solid if s < 8 => (s, 7),
            ScopeStyle::Solid => (8, s),
        };
        for value in lo..=hi {
            let y = 17 - value;
            if (0..HEIGHT as i32).contains(&y) {
                px[y as usize * WIDTH + col] = color;
            }
        }
        prev = Some(s);
    }
    px
}

/// The shade bar's oscilloscope: the first 38 samples of the window, each scaled to 0..4 with 2
/// the middle (rows 0..4, 4 the bottom), in color 18, each column a run from the previous
/// column's row to this column's row, both ends included.
pub fn scope_shade(wave: &[f32], style: ScopeStyle) -> [u8; SHADE_WIDTH * SHADE_HEIGHT] {
    let mut px = [OFF; SHADE_WIDTH * SHADE_HEIGHT];
    if wave.is_empty() || wave.iter().all(|s| *s == 0.0) {
        return px;
    }
    let mut prev: Option<i32> = None;
    for col in 0..SHADE_WIDTH {
        let sample = wave[col * wave.len() / BANDS];
        let v = ((sample * 32767.0) as i32) >> 11;
        let s = (((v + 8) * 5) >> 4).clamp(0, 4);
        let from = prev.unwrap_or(s);
        let (lo, hi) = match style {
            ScopeStyle::Dots => (s, s),
            ScopeStyle::Lines => (s.min(from), s.max(from)),
            ScopeStyle::Solid => (s.min(2), s.max(2)),
        };
        for value in lo..=hi {
            px[(4 - value) as usize * SHADE_WIDTH + col] = 18;
        }
        prev = Some(s);
    }
    px
}

/// What the analyzer keeps between frames.
#[derive(Debug, Clone)]
pub struct ClassicVis {
    /// Each band's level times 16, falling off.
    sa: [i32; BANDS],
    /// Each band's peak times 256.
    peak: [i32; BANDS],
    /// How fast each peak falls.
    peak_speed: [f32; BANDS],
    /// When the last frame was stepped; -1 before the first.
    last_frame_at: i64,
}

impl Default for ClassicVis {
    fn default() -> Self {
        ClassicVis {
            sa: [0; BANDS],
            peak: [0; BANDS],
            peak_speed: [PEAK_SPEED; BANDS],
            last_frame_at: -1,
        }
    }
}

impl ClassicVis {
    /// The frames due at `now`, and notes them stepped.
    fn step(&mut self, now: i64) -> i64 {
        // a clock that went back starts the frames afresh from now
        let fresh = self.last_frame_at < 0 || now < self.last_frame_at;
        let frames = if fresh {
            1
        } else {
            ((now - self.last_frame_at) / FRAME_MS).min(MAX_FRAMES)
        };
        if frames > 0 {
            self.last_frame_at = if fresh || frames == MAX_FRAMES {
                now
            } else {
                self.last_frame_at + frames * FRAME_MS
            };
        }
        frames
    }

    /// Steps band `col`'s level and peak `frames` times toward `v` (0..15).
    fn fall(&mut self, col: usize, v: i32, frames: i64) {
        for _ in 0..frames {
            if v * 16 < self.sa[col] {
                self.sa[col] -= FALLOFF;
            } else {
                self.sa[col] = v * 16;
            }
            if self.sa[col] < 0 {
                self.sa[col] = 0;
            }
            if self.peak[col] <= v * 256 {
                self.peak[col] = v * 256;
                self.peak_speed[col] = PEAK_SPEED;
            }
        }
    }

    /// Lets band `col`'s peak fall `frames` times.
    fn drop_peak(&mut self, col: usize, frames: i64) {
        for _ in 0..frames {
            self.peak[col] -= self.peak_speed[col].round() as i32;
            self.peak_speed[col] *= PEAK_ACCEL;
            if self.peak[col] < 0 {
                self.peak[col] = 0;
            }
        }
    }

    /// The shade bar's analyzer: 37 columns, thick bars of three with the fourth empty over
    /// bands two per column, each up to five rows from the bottom in colors 17, 14, 11, 8 and
    /// 5, the peak in color 23 (as Winamp paints the strip; confirmed on the VM).
    pub fn frame_shade(
        &mut self,
        now: i64,
        levels: &[f32],
        style: &VisStyle,
    ) -> [u8; SHADE_WIDTH * SHADE_HEIGHT] {
        let frames = self.step(now);
        let mut px = [OFF; SHADE_WIDTH * SHADE_HEIGHT];
        let level = |b: usize| -> i32 {
            levels
                .get(b)
                .map_or(0, |v| (v.clamp(0.0, 1.0) * 15.0).round() as i32)
        };
        // a group of four columns starting at g shows bands 2g..2g+3, skipping
        // the bands between groups, column c falling as band 2c does; the main window's
        // analyzer falls with it, as the two do in the VM
        for col in 0..SHADE_BANDS {
            let v = if style.thin_bars {
                ((level(col * 2) + level(col * 2 + 1)) / 2).min(15)
            } else {
                let g = (col & !3) * 2;
                ((level(g) + level(g + 1) + level(g + 2) + level(g + 3)) >> 2).min(15)
            };
            let band = col * 2;
            self.fall(band, v, frames);
            let shown = ((self.sa[band] >> 4).clamp(0, 15) * 5 / 15).min(5);
            if style.thin_bars || col % 4 != 3 {
                let height = (self.sa[band] >> 4).clamp(0, 15);
                let base = match style.analyzer {
                    AnalyzerStyle::Normal => 17,
                    AnalyzerStyle::Fire => height + 2,
                    AnalyzerStyle::Line => 17 - height,
                };
                for i in 0..shown {
                    let color = match style.analyzer {
                        AnalyzerStyle::Line => base,
                        _ => base - 3 * i,
                    };
                    px[(4 - i) as usize * SHADE_WIDTH + col] = color.clamp(0, 23) as u8;
                }
                let p = (self.peak[band] * 5 / 15) >> 8;
                if style.peaks && (0..5).contains(&p) {
                    px[(4 - p) as usize * SHADE_WIDTH + col] = 23;
                }
            }
            self.drop_peak(band, frames);
        }
        px
    }

    /// Steps the frames due at `now` with `levels` (0..1 per band) and paints the picture as
    /// palette indices, row 0 at the top.
    pub fn frame(&mut self, now: i64, levels: &[f32], style: &VisStyle) -> [u8; WIDTH * HEIGHT] {
        let frames = self.step(now);
        let mut px = grid();
        let level = |b: usize| -> i32 {
            levels
                .get(b)
                .map_or(0, |v| (v.clamp(0.0, 1.0) * 15.0).round() as i32)
        };
        for col in 0..BANDS {
            // thick bars: a group of four columns takes their mean, the fourth stays empty;
            // thin bars: a band per column
            let v = if style.thin_bars {
                level(col).min(15)
            } else {
                let g = col & !3;
                ((level(g) + level(g + 1) + level(g + 2) + level(g + 3)) >> 2).min(15)
            };
            self.fall(col, v, frames);
            let shown = (self.sa[col] >> 4).clamp(0, 15);
            let x = col;
            if style.thin_bars || col % 4 != 3 {
                let base = match style.analyzer {
                    AnalyzerStyle::Normal => 17,
                    AnalyzerStyle::Fire => shown + 2,
                    AnalyzerStyle::Line => 17 - shown,
                };
                for i in 0..shown {
                    let y = HEIGHT as i32 - 1 + BASE_BELOW - i;
                    let color = match style.analyzer {
                        AnalyzerStyle::Line => base,
                        _ => base - i,
                    };
                    if (0..HEIGHT as i32).contains(&y) {
                        px[y as usize * WIDTH + x] = color.clamp(0, 23) as u8;
                    }
                }
                let p = self.peak[col] >> 8;
                if style.peaks && (0..16).contains(&p) {
                    let y = HEIGHT as i32 - 1 + BASE_BELOW - p;
                    if (0..HEIGHT as i32).contains(&y) {
                        px[y as usize * WIDTH + x] = 23;
                    }
                }
            }
            self.drop_peak(col, frames);
        }
        px
    }
}

impl Painter<'_> {
    /// A vis picture `w` wide (palette indices, row 0 at the top) at `dx`, `dy`, colored by
    /// `palette`.
    pub(super) fn vis(
        &mut self,
        px: &[u8],
        w: usize,
        palette: &[crate::draw::Rgb; 24],
        dx: i32,
        dy: i32,
    ) {
        let h = px.len() / w;
        let argb: Vec<u32> = px
            .iter()
            .map(|i| 0xff00_0000 | palette[(*i as usize).min(23)].0)
            .collect();
        self.out.push(DrawCmd::Pixels {
            rect: Rect::new(self.at.left + dx, self.at.top + dy, w as i32, h as i32),
            w: w as i32,
            h: h as i32,
            argb: Rc::from(argb.into_boxed_slice()),
            alpha: 255,
            stable: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ClassicVis, VisStyle, HEIGHT, WIDTH};

    #[test]
    fn silence_shows_the_dot_grid_alone() {
        let mut v = ClassicVis::default();
        let px = v.frame(0, &[0.0; 75], &VisStyle::default());
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let want = u8::from(y % 2 == 1 && x % 2 == 0);
                assert_eq!(px[y * WIDTH + x], want, "{x},{y}");
            }
        }
    }

    #[test]
    fn a_full_band_fills_its_three_columns_from_color_15_up_and_marks_its_peak() {
        let mut v = ClassicVis::default();
        let mut levels = [0.0f32; 75];
        levels[..4].copy_from_slice(&[1.0; 4]);
        let px = v.frame(0, &levels, &VisStyle::default());
        // level 15: thirteen rows show, the bottom row color 15, up to row 3 color 3
        for (i, y) in (3..16).rev().enumerate() {
            for x in 0..3 {
                assert_eq!(px[y * WIDTH + x], 15 - i as u8, "{x},{y}");
            }
            assert_eq!(
                px[y * WIDTH + 3],
                u8::from(y % 2 == 1 && 3 % 2 == 0),
                "the fourth column stays"
            );
        }
        // the peak at 15 sits two rows under the picture: not shown; after frames it falls into view
        for _ in 0..40 {
            v.frame(v.last_frame_at + 25, &[0.0; 75], &VisStyle::default());
        }
        let px = v.frame(v.last_frame_at + 25, &[0.0; 75], &VisStyle::default());
        assert!(px.contains(&23), "a peak mark on its way down");
    }
}
