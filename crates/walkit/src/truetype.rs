// SPDX-License-Identifier: GPL-3.0-or-later

//! TrueType text as Winamp draws it, for hosts that draw the letters themselves (behind the
//! `truetype` feature; the engine itself never draws). Winamp draws a skin's TrueType text in
//! one of two ways, a [`Style`]:
//!
//! - [`Style::Classic`], a classic skin's text (the playlist), as Windows draws it: the font's
//!   own hinting fits each letter to the pixel grid, letters advance by whole pixels, pairs
//!   the font's `kern` table names move by whole pixels, and the font's `gasp` table says at
//!   which sizes the letters are smoothed. Arial between 9 and 17 pixels is not smoothed.
//! - [`Style::Modern`], a modern skin's text: always smoothed, each letter fitted to the
//!   pixel grid by an autohinter rather than the font's own hinting, at three quarters of the
//!   skin's font size without rounding (Arial at `fontsize="14"` is 10.5 pixels per em). Kerning
//!   shrinks below 25 pixels per em, and bold is the regular letters drawn twice.
//!
//! A host parses each font once ([`Font::parse`]), answers the engine's text measures from
//! [`Font::width`] and [`Font::line`] at [`em`] pixels per em, and draws a
//! [`TextCmd`](crate::draw::TextCmd) from [`Font::run`]: the pen starts on a whole pixel, and
//! a face the engine marks `fake_bold` is drawn twice, one pixel apart (`bold` = 1).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlinePen, SmoothMode, Target,
};
use skrifa::raw::tables::gasp::GaspRangeBehavior;
use skrifa::raw::tables::kern::SubtableKind;
use skrifa::raw::types::Tag;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::gdi::em_size;

/// How far outside the outline a pixel's center may lie and still count as on it. Hinted
/// points sit on a grid of 64ths of a pixel, so a center on the outline is met exactly.
const EDGE: f32 = 1e-5;

/// Smoothed classic letters are sampled this many times per pixel, across and down.
const SAMPLES: i32 = 4;

/// Modern text's kerning is scaled down below this many pixels per em.
const KERN_FULL_PPEM: f32 = 25.0;

/// How far a modern line's ascent reaches above the top of a capital H, in pixels.
const MODERN_HEADROOM: i32 = 3;

/// Which of Winamp's two ways of drawing TrueType text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// A classic skin's text, drawn by Windows: the font's own hinting, whole-pixel widths
    /// from its tables, smoothed only where its `gasp` table asks.
    Classic,
    /// A modern skin's text: smoothed, fitted to the grid by an autohinter, at three quarters
    /// of the font size without rounding.
    Modern,
}

/// A drawn letter: where its bitmap sits from the pen on the baseline, its coverage with one
/// byte per pixel, and how far the pen moves on.
pub struct Glyph {
    /// Pixels from the pen to the bitmap's left edge.
    pub left: i32,
    /// Pixels from the baseline up to the bitmap's top edge.
    pub top: i32,
    /// The bitmap's width in pixels.
    pub w: usize,
    /// The bitmap's height in pixels.
    pub h: usize,
    /// One byte per pixel, rows top first: 0 is clear, 255 is the text's color.
    pub coverage: Vec<u8>,
    /// Whole pixels the pen moves on after the letter.
    pub advance: i32,
}

/// The pixels per em Winamp draws a skin's `fontsize` at in `style`: three quarters of it,
/// rounded to whole pixels for classic text as Windows sizes fonts, unrounded for modern text.
/// A host drawing at a scale multiplies it by the scale.
pub fn em(style: Style, fontsize: i32) -> f32 {
    match style {
        Style::Classic => em_size(fontsize).max(1.0),
        Style::Modern => (fontsize as f32 * 0.75).max(1.0),
    }
}

/// What a font does at one size in one style.
struct Sized {
    /// The hinting at this size, or `None` where the font asks for none or has none.
    hinter: Option<HintingInstance>,
    /// Whether letters are smoothed at this size.
    smooth: bool,
    glyphs: HashMap<char, Rc<Glyph>>,
}

/// A size in one style, as a key: the em's bits.
type SizeKey = (Style, u32);

/// A TrueType font and the letters drawn from it so far.
pub struct Font {
    data: Rc<[u8]>,
    sizes: RefCell<HashMap<SizeKey, Sized>>,
    kerns: RefCell<HashMap<(char, char, SizeKey), i32>>,
    lines: RefCell<HashMap<SizeKey, (f32, f32)>>,
}

impl Font {
    /// The font in `data`, or `None` when it is not a TrueType or OpenType file. Of a
    /// collection (`.ttc`) it is the first font.
    pub fn parse(data: Rc<[u8]>) -> Option<Rc<Font>> {
        FontRef::from_index(&data, 0).ok()?;
        Some(Rc::new(Font {
            data,
            sizes: RefCell::default(),
            kerns: RefCell::default(),
            lines: RefCell::default(),
        }))
    }

    /// The letter `c` at `em` pixels per em in `style`, drawn once.
    pub fn glyph(&self, c: char, style: Style, em: f32) -> Rc<Glyph> {
        let key = (style, em.to_bits());
        let mut sizes = self.sizes.borrow_mut();
        let font = self.font();
        let sized = sizes.entry(key).or_insert_with(|| size(&font, style, em));
        if let Some(glyph) = sized.glyphs.get(&c) {
            return glyph.clone();
        }
        let glyph = Rc::new(match style {
            Style::Classic => draw(&font, sized, c, em as u32),
            Style::Modern => draw_modern(&font, sized, c, em),
        });
        sized.glyphs.insert(c, glyph.clone());
        glyph
    }

    /// How far `text` advances at `em` pixels per em in `style`, with kerning.
    pub fn advance(&self, text: &str, style: Style, em: f32) -> i32 {
        self.run(text, style, em, 0)
            .last()
            .map_or(0, |(at, glyph)| at + glyph.advance)
    }

    /// How wide `text` draws at `em` pixels per em in `style`. A face without a bold cut is
    /// drawn twice, `bold` pixels apart (0 when it is not bold; 1, or the scale a host draws
    /// at): a classic line is that much wider for it, a modern line that much wider for each
    /// letter.
    pub fn width(&self, text: &str, style: Style, em: f32, bold: i32) -> i32 {
        let run = self.run(text, style, em, bold);
        let Some((at, glyph)) = run.last() else {
            return 0;
        };
        // the second stroke reaches past the last letter
        at + glyph.advance + bold
    }

    /// The letters of `text` at `em` pixels per em in `style`, each with how far the pen has
    /// moved from the start of the line when it is drawn, kerning counted in. A modern line
    /// drawn bold, its two strokes `bold` pixels apart, moves that much more after each letter.
    pub fn run(&self, text: &str, style: Style, em: f32, bold: i32) -> Vec<(i32, Rc<Glyph>)> {
        let extra = if style == Style::Modern { bold } else { 0 };
        let mut x = 0;
        let mut last = None;
        text.chars()
            .map(|c| {
                if let Some(prev) = last {
                    x += self.kerning(prev, c, style, em);
                }
                last = Some(c);
                let glyph = self.glyph(c, style, em);
                let at = x;
                x += glyph.advance + extra;
                (at, glyph)
            })
            .collect()
    }

    /// How many pixels `right` moves toward `left` (negative) or away from it at `em` pixels
    /// per em in `style`, from the font's `kern` table, in whole pixels. Modern text scales
    /// the pair down below 25 pixels per em first.
    pub fn kerning(&self, left: char, right: char, style: Style, em: f32) -> i32 {
        *self
            .kerns
            .borrow_mut()
            .entry((left, right, (style, em.to_bits())))
            .or_insert_with(|| {
                let font = self.font();
                let map = font.charmap();
                let (Some(l), Some(r)) = (map.map(left), map.map(right)) else {
                    return 0;
                };
                let Ok(kern) = font.kern() else {
                    return 0;
                };
                let units: i32 = kern
                    .subtables()
                    .flatten()
                    .filter(|table| table.is_horizontal() && !table.is_cross_stream())
                    .filter_map(|table| match table.kind().ok()? {
                        SubtableKind::Format0(pairs) => pairs.kerning(l, r),
                        _ => None,
                    })
                    .sum();
                let upem = f32::from(font.head().map_or(1000, |head| head.units_per_em()));
                let scaled = units as f32 * em / upem;
                match style {
                    Style::Classic => scaled.round() as i32,
                    Style::Modern => {
                        let ppem = (em + 0.5).floor();
                        let k = if ppem < KERN_FULL_PPEM {
                            scaled * ppem / KERN_FULL_PPEM
                        } else {
                            scaled
                        };
                        // to 64ths of a pixel, then to the nearest pixel, halves up
                        (((k * 64.0).round() + 32.0) / 64.0).floor() as i32
                    }
                }
            })
    }

    /// The line at `em` pixels per em in `style`, as ascent and descent, both positive. A
    /// classic line is the font's own; the engine lays classic text out by Windows' line
    /// itself. A modern line reaches three pixels above a capital H and down to one pixel
    /// above the foot of a g, both as fitted to the grid: the engine centers modern text by it.
    pub fn line(&self, style: Style, em: f32) -> (f32, f32) {
        let key = (style, em.to_bits());
        if let Some(line) = self.lines.borrow().get(&key) {
            return *line;
        }
        let font = self.font();
        let m = font.metrics(Size::new(em), LocationRef::default());
        let line = match style {
            Style::Classic => (m.ascent, -m.descent),
            Style::Modern => {
                let hinter = modern_hinter(&font, em);
                let reach = |c: char| {
                    let id = font.charmap().map(c)?;
                    let outline = font.outline_glyphs().get(id)?;
                    let mut ys = Extent::default();
                    match &hinter {
                        Some(h) => outline.draw(DrawSettings::hinted(h, false), &mut ys).ok()?,
                        None => outline
                            .draw(
                                DrawSettings::unhinted(Size::new(em), LocationRef::default()),
                                &mut ys,
                            )
                            .ok()?,
                    };
                    ys.found.then_some((ys.top, ys.bottom))
                };
                let cap = reach('H')
                    .map_or(m.ascent.ceil() as i32 - MODERN_HEADROOM, |(top, _)| {
                        top.ceil() as i32
                    });
                let foot = reach('g').map_or((-m.descent).ceil() as i32, |(_, bottom)| {
                    (-bottom).ceil() as i32
                });
                let ascent = cap + MODERN_HEADROOM;
                let height = ascent + foot - 1;
                (ascent as f32, (height - ascent) as f32)
            }
        };
        self.lines.borrow_mut().insert(key, line);
        line
    }

    fn font(&self) -> FontRef<'_> {
        // checked in `parse`
        FontRef::from_index(&self.data, 0).expect("a font that parsed before")
    }
}

/// The top and bottom of an outline.
#[derive(Default)]
struct Extent {
    found: bool,
    top: f32,
    bottom: f32,
}

impl Extent {
    fn add(&mut self, y: f32) {
        if self.found {
            self.top = self.top.max(y);
            self.bottom = self.bottom.min(y);
        } else {
            (self.found, self.top, self.bottom) = (true, y, y);
        }
    }
}

impl OutlinePen for Extent {
    fn move_to(&mut self, _x: f32, y: f32) {
        self.add(y);
    }
    fn line_to(&mut self, _x: f32, y: f32) {
        self.add(y);
    }
    fn quad_to(&mut self, _cx: f32, cy: f32, _x: f32, y: f32) {
        self.add(cy);
        self.add(y);
    }
    fn curve_to(&mut self, _ax: f32, ay: f32, _bx: f32, by: f32, _x: f32, y: f32) {
        self.add(ay);
        self.add(by);
        self.add(y);
    }
    fn close(&mut self) {}
}

/// The autohinter modern text is fitted with at `em`.
fn modern_hinter(font: &FontRef, em: f32) -> Option<HintingInstance> {
    let options = HintingOptions {
        engine: Engine::Auto(None),
        target: Target::Smooth {
            mode: SmoothMode::Normal,
            symmetric_rendering: true,
            preserve_linear_metrics: false,
        },
    };
    HintingInstance::new(
        &font.outline_glyphs(),
        Size::new(em),
        LocationRef::default(),
        options,
    )
    .ok()
}

/// What a font does at `em` in `style`.
fn size(font: &FontRef, style: Style, em: f32) -> Sized {
    match style {
        Style::Classic => classic_size(font, em as u32),
        Style::Modern => Sized {
            hinter: modern_hinter(font, em),
            smooth: true,
            glyphs: HashMap::new(),
        },
    }
}

/// Reads from the font's `gasp` table what Windows does at `px`: fit to the grid, smooth,
/// both or neither. A font without the table gets both.
fn classic_size(font: &FontRef, px: u32) -> Sized {
    let (mut fit, mut smooth) = (true, true);
    if let Ok(gasp) = font.gasp() {
        let range = gasp
            .gasp_ranges()
            .iter()
            .find(|r| u32::from(r.range_max_ppem()) >= px);
        if let Some(range) = range {
            let behavior = range.range_gasp_behavior();
            fit = behavior.contains(GaspRangeBehavior::GASP_GRIDFIT);
            smooth = behavior.contains(GaspRangeBehavior::GASP_DOGRAY);
        }
    }
    let hinter = fit
        .then(|| {
            let options = HintingOptions {
                engine: Engine::Interpreter,
                target: Target::Mono,
            };
            HintingInstance::new(
                &font.outline_glyphs(),
                Size::new(px as f32),
                LocationRef::default(),
                options,
            )
            .ok()
        })
        .flatten();
    Sized {
        hinter,
        smooth,
        glyphs: HashMap::new(),
    }
}

/// Draws `c`: its outline, hinted where the size asks for it, filled into a bitmap.
fn draw(font: &FontRef, sized: &Sized, c: char, px: u32) -> Glyph {
    let size = Size::new(px as f32);
    let id = font.charmap().map(c).unwrap_or(GlyphId::NOTDEF);
    let linear = font
        .glyph_metrics(size, LocationRef::default())
        .advance_width(id)
        .unwrap_or(0.0);
    let mut advance = linear.round() as i32;
    // Windows takes a letter's width from the font's tables before it asks the hinting
    let device = device_width(font, id, px);
    let mut path = Path::new(px);
    if let Some(outline) = font.outline_glyphs().get(id) {
        let hinted = sized.hinter.as_ref().and_then(|hinter| {
            let mut hinted = Path::new(px);
            let metrics = outline
                .draw(DrawSettings::hinted(hinter, false), &mut hinted)
                .ok()?;
            Some((hinted, metrics.advance_width))
        });
        if let Some((hinted, width)) = hinted {
            path = hinted;
            if let Some(width) = width {
                advance = width.round() as i32;
            }
        } else {
            let settings = DrawSettings::unhinted(size, LocationRef::default());
            if outline.draw(settings, &mut path).is_err() {
                path = Path::new(px);
            }
        }
    }
    let mut glyph = path.fill(sized.smooth);
    glyph.advance = device.unwrap_or(advance);
    glyph
}

/// Draws `c` as modern text: its outline fitted by the autohinter, each pixel as dark as the
/// share of it the outline covers, and the fitted advance in whole pixels.
fn draw_modern(font: &FontRef, sized: &Sized, c: char, em: f32) -> Glyph {
    let size = Size::new(em);
    let id = font.charmap().map(c).unwrap_or(GlyphId::NOTDEF);
    let linear = font
        .glyph_metrics(size, LocationRef::default())
        .advance_width(id)
        .unwrap_or(0.0);
    let mut advance = linear.round() as i32;
    let mut path = Path::fine(em);
    if let Some(outline) = font.outline_glyphs().get(id) {
        let drawn = match &sized.hinter {
            Some(hinter) => outline.draw(DrawSettings::hinted(hinter, false), &mut path),
            None => outline.draw(
                DrawSettings::unhinted(size, LocationRef::default()),
                &mut path,
            ),
        };
        match drawn {
            Ok(metrics) => {
                if let Some(width) = metrics.advance_width {
                    advance = width.round() as i32;
                }
            }
            Err(_) => path = Path::fine(em),
        }
    }
    let mut glyph = path.fill_area();
    glyph.advance = advance;
    glyph
}

/// The width the font's tables give glyph `id` at `px` pixels per em, where they give one:
/// the `hdmx` table's device width for that size, or, from the size the `LTSH` table says
/// the glyph scales evenly, its scaled width rounded. `None` leaves the width to the
/// hinting.
fn device_width(font: &FontRef, id: GlyphId, px: u32) -> Option<i32> {
    let index = id.to_u32() as usize;
    if let Some(record) = u8::try_from(px)
        .ok()
        .and_then(|size| font.hdmx().ok()?.record_for_size(size))
    {
        if let Some(width) = record.widths().get(index) {
            return Some(i32::from(*width));
        }
    }
    // LTSH: a version, the glyph count, then one size per glyph
    let ltsh = font.table_data(Tag::new(b"LTSH"))?;
    let from = u32::from(*ltsh.as_bytes().get(4 + index)?);
    if from == 0 || px < from {
        return None;
    }
    let linear = font
        .glyph_metrics(Size::new(px as f32), LocationRef::default())
        .advance_width(id)?;
    Some(linear.round() as i32)
}

/// An outline as closed runs of straight edges, in pixels with y up.
struct Path {
    contours: Vec<Vec<(f32, f32)>>,
    /// How many straight edges stand in for a curve.
    steps: u32,
}

impl Path {
    fn new(px: u32) -> Path {
        Path {
            contours: Vec::new(),
            steps: (px / 3).clamp(6, 32),
        }
    }

    /// A path with curves cut finely enough for smoothed letters, whose edges show.
    fn fine(em: f32) -> Path {
        Path {
            contours: Vec::new(),
            steps: ((em / 2.0) as u32).clamp(16, 48),
        }
    }

    fn last(&self) -> (f32, f32) {
        self.contours
            .last()
            .and_then(|c| c.last())
            .copied()
            .unwrap_or_default()
    }

    fn push(&mut self, point: (f32, f32)) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push(point);
        }
    }

    /// The edges that cross scan lines, as `(x0, y0, x1, y1)` after `turn`, which maps a
    /// point to the sweep's own axes.
    fn edges(&self, turn: impl Fn((f32, f32)) -> (f32, f32)) -> Vec<[f32; 4]> {
        let mut edges = Vec::new();
        for contour in &self.contours {
            for (i, &from) in contour.iter().enumerate() {
                let (x0, y0) = turn(from);
                let (x1, y1) = turn(contour[(i + 1) % contour.len()]);
                if y0 != y1 {
                    edges.push([x0, y0, x1, y1]);
                }
            }
        }
        edges
    }

    /// Fills the outline. Unsmoothed, a pixel is on when its center lies inside, and a stem
    /// thinner than a pixel that passes between two centers still turns one pixel on, as
    /// Windows keeps thin strokes from dropping out. Smoothed, each pixel is sampled
    /// [`SAMPLES`] times across and down.
    fn fill(&self, smooth: bool) -> Glyph {
        let points = || self.contours.iter().flatten();
        let fold = |f: fn(f32, f32) -> f32, pick: fn(&(f32, f32)) -> f32, start: f32| {
            points().map(pick).fold(start, f)
        };
        let empty = Glyph {
            left: 0,
            top: 0,
            w: 0,
            h: 0,
            coverage: Vec::new(),
            advance: 0,
        };
        if points().next().is_none() {
            return empty;
        }
        // a pixel of margin, for a dropped-out stem on the box's edge
        let left = fold(f32::min, |p| p.0, f32::MAX).floor() as i32 - 1;
        let right = fold(f32::max, |p| p.0, f32::MIN).ceil() as i32 + 1;
        let bottom = fold(f32::min, |p| p.1, f32::MAX).floor() as i32 - 1;
        let top = fold(f32::max, |p| p.1, f32::MIN).ceil() as i32 + 1;
        let (w, h) = ((right - left) as usize, (top - bottom) as usize);
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            return empty;
        }
        let n = if smooth { SAMPLES } else { 1 };
        let scale = n as f32;
        // sample space: x right from the box's left edge, y down from its top edge
        let across = self.edges(|(x, y)| ((x - left as f32) * scale, (top as f32 - y) * scale));
        let (sw, sh) = (w * n as usize, h * n as usize);
        let mut samples = vec![false; sw * sh];
        let mut drops = Vec::new();
        let rows = runs(&across, sh);
        for (row, line) in rows.iter().enumerate() {
            for &(from, to) in line {
                match centers(from, to, sw) {
                    Some(Centers::Covered(first, last)) => {
                        samples[row * sw + first..=row * sw + last].fill(true);
                    }
                    Some(Centers::Between(before)) if continues(&rows, row, from, to) => {
                        // the left one of the two
                        drops.push((before, row, before, row, before + 1, row));
                    }
                    _ => {}
                }
            }
        }
        if !smooth {
            // the same sweep along the columns finds thin flat strokes
            let down = self.edges(|(x, y)| (top as f32 - y, x - left as f32));
            let columns = runs(&down, sw);
            for (col, line) in columns.iter().enumerate() {
                for &(from, to) in line {
                    if let Some(Centers::Between(before)) = centers(from, to, sh) {
                        if continues(&columns, col, from, to) {
                            // the lower one of the two
                            drops.push((col, before + 1, col, before, col, before + 1));
                        }
                    }
                }
            }
            // a dropped-out stroke lights its pixel only when neither neighbor is lit
            for (x, y, ax, ay, bx, by) in drops {
                if !samples[ay * sw + ax] && !samples[by * sw + bx] {
                    samples[y * sw + x] = true;
                }
            }
        }
        let mut coverage = vec![0u8; w * h];
        let per_pixel = (n * n) as u32;
        for (i, cover) in coverage.iter_mut().enumerate() {
            let (px, py) = (i % w * n as usize, i / w * n as usize);
            let mut lit = 0;
            for sy in 0..n as usize {
                for sx in 0..n as usize {
                    lit += u32::from(samples[(py + sy) * sw + px + sx]);
                }
            }
            *cover = (lit * 255 / per_pixel) as u8;
        }
        Glyph {
            left,
            top,
            w,
            h,
            coverage,
            advance: 0,
        }
    }
}

impl Path {
    /// Fills the outline with each pixel as dark as the share of its square the outline
    /// covers, by the nonzero rule.
    fn fill_area(&self) -> Glyph {
        let empty = Glyph {
            left: 0,
            top: 0,
            w: 0,
            h: 0,
            coverage: Vec::new(),
            advance: 0,
        };
        let mut points = self.contours.iter().flatten();
        let Some(&(x0, y0)) = points.next() else {
            return empty;
        };
        let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (x0, x0, y0, y0);
        for &(x, y) in points {
            (lo_x, hi_x, lo_y, hi_y) = (lo_x.min(x), hi_x.max(x), lo_y.min(y), hi_y.max(y));
        }
        let left = lo_x.floor() as i32;
        let top = hi_y.ceil() as i32;
        let (w, h) = (
            (hi_x.ceil() as i32 - left).max(0) as usize + 1,
            (top - lo_y.floor() as i32).max(0) as usize,
        );
        if h == 0 || w > 4096 || h > 4096 {
            return empty;
        }
        // signed area left in each cell by the edges crossing it, summed along each row
        let mut cells = vec![0.0f32; w * h + 1];
        for contour in &self.contours {
            for (i, &from) in contour.iter().enumerate() {
                let to = contour[(i + 1) % contour.len()];
                let a = (from.0 - left as f32, top as f32 - from.1);
                let b = (to.0 - left as f32, top as f32 - to.1);
                accumulate(&mut cells, w, h, a, b);
            }
        }
        let mut coverage = vec![0u8; w * h];
        for y in 0..h {
            let mut sum = 0.0;
            for x in 0..w {
                sum += cells[y * w + x];
                coverage[y * w + x] = (sum.abs().min(1.0) * 255.0).round() as u8;
            }
        }
        Glyph {
            left,
            top,
            w,
            h,
            coverage,
            advance: 0,
        }
    }
}

/// Adds the edge from `a` to `b` (pixels, y down) to the row-wise sums of covered area.
fn accumulate(cells: &mut [f32], w: usize, h: usize, a: (f32, f32), b: (f32, f32)) {
    if (a.1 - b.1).abs() <= f32::EPSILON {
        return;
    }
    let (dir, p0, p1) = if a.1 < b.1 { (1.0, a, b) } else { (-1.0, b, a) };
    let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
    let mut x = p0.0;
    if p0.1 < 0.0 {
        x -= p0.1 * dxdy;
    }
    let first = p0.1.max(0.0) as usize;
    let last = (p1.1.ceil() as usize).min(h);
    for y in first..last {
        let row = y * w;
        let dy = ((y + 1) as f32).min(p1.1) - (y as f32).max(p0.1);
        let next = x + dxdy * dy;
        let d = dy * dir;
        let (x0, x1) = if x < next { (x, next) } else { (next, x) };
        let cell = |i: i32| -> Option<usize> {
            if i < 0 {
                Some(row)
            } else if (i as usize) < w {
                Some(row + i as usize)
            } else {
                None
            }
        };
        let mut put = |i: i32, v: f32| {
            if let Some(i) = cell(i) {
                cells[i] += v;
            }
        };
        let x0f = x0.floor();
        let (x0i, x1i) = (x0f as i32, x1.ceil() as i32);
        if x1i <= x0i + 1 {
            // within one cell: the part right of the edge's middle goes on to the next cell
            let mid = f32::midpoint(x, next) - x0f;
            put(x0i, d - d * mid);
            put(x0i + 1, d * mid);
        } else {
            let s = (x1 - x0).recip();
            let f0 = x0 - x0f;
            let a0 = 0.5 * s * (1.0 - f0) * (1.0 - f0);
            let f1 = x1 - x1.ceil() + 1.0;
            let am = 0.5 * s * f1 * f1;
            put(x0i, d * a0);
            if x1i == x0i + 2 {
                put(x0i + 1, d * (1.0 - a0 - am));
            } else {
                let a1 = s * (1.5 - f0);
                put(x0i + 1, d * (a1 - a0));
                for i in x0i + 2..x1i - 1 {
                    put(i, d * s);
                }
                let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                put(x1i - 1, d * (1.0 - a2 - am));
            }
            put(x1i, d * am);
        }
        x = next;
    }
}

/// The runs inside the outline along each of `lines` scan lines through the sample centers,
/// by the nonzero rule, as `(from, to)` along the line.
fn runs(edges: &[[f32; 4]], lines: usize) -> Vec<Vec<(f32, f32)>> {
    let mut crossings: Vec<(f32, i32)> = Vec::new();
    (0..lines)
        .map(|line| {
            let y = line as f32 + 0.5;
            crossings.clear();
            for &[x0, y0, x1, y1] in edges {
                let (low, high, dir) = if y0 < y1 { (y0, y1, 1) } else { (y1, y0, -1) };
                if y >= low && y < high {
                    crossings.push((x0 + (y - y0) * (x1 - x0) / (y1 - y0), dir));
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut found = Vec::new();
            let mut winding = 0;
            let mut start = 0.0;
            for &(x, dir) in &crossings {
                let was = winding;
                winding += dir;
                if was == 0 && winding != 0 {
                    start = x;
                } else if was != 0 && winding == 0 {
                    found.push((start, x));
                }
            }
            found
        })
        .collect()
}

/// The samples a run lights along a line of `len` samples.
enum Centers {
    /// The first and last sample whose centers the run covers.
    Covered(usize, usize),
    /// The run passes between the centers of this sample and the next.
    Between(usize),
}

fn centers(from: f32, to: f32, len: usize) -> Option<Centers> {
    // centers at i + 0.5; a center on the outline counts as inside
    let first = (from - 0.5 - EDGE).ceil().max(0.0) as usize;
    let last = (to - 0.5 + EDGE).floor();
    if last >= first as f32 {
        return (first < len).then(|| Centers::Covered(first, (last as usize).min(len - 1)));
    }
    if to <= from || first == 0 || first >= len {
        return None;
    }
    Some(Centers::Between(first - 1))
}

/// Whether a run between two centers lights a pixel: the stroke it cuts through goes on
/// across both neighboring lines, or the run is at least half a pixel long. The thin tip
/// of a stroke that ends between two lines is left dark.
fn continues(runs: &[Vec<(f32, f32)>], line: usize, from: f32, to: f32) -> bool {
    if to - from >= 0.5 {
        return true;
    }
    let near = |line: &Vec<(f32, f32)>| line.iter().any(|&(a, b)| b >= from - 1.0 && a <= to + 1.0);
    line > 0 && line + 1 < runs.len() && near(&runs[line - 1]) && near(&runs[line + 1])
}

impl OutlinePen for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![(x, y)]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push((x, y));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (x0, y0) = self.last();
        for i in 1..=self.steps {
            let t = i as f32 / self.steps as f32;
            let u = 1.0 - t;
            self.push((
                u * u * x0 + 2.0 * u * t * cx + t * t * x,
                u * u * y0 + 2.0 * u * t * cy + t * t * y,
            ));
        }
    }

    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        let (x0, y0) = self.last();
        for i in 1..=self.steps {
            let t = i as f32 / self.steps as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push((
                a * x0 + b * ax + c * bx + d * x,
                a * y0 + b * ay + c * by + d * y,
            ));
        }
    }

    fn close(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Rc<Font> {
        let bytes: &[u8] = include_bytes!("../../../examples/skins/fonts/LiberationSans.ttf");
        Font::parse(Rc::from(bytes)).expect("the stand-in font parses")
    }

    #[test]
    fn small_text_is_not_smoothed() {
        let font = font();
        for c in "Winamp 0123".chars() {
            let glyph = font.glyph(c, Style::Classic, 10.0);
            assert!(
                glyph
                    .coverage
                    .iter()
                    .all(|&cover| cover == 0 || cover == 255),
                "{c:?} has smoothed pixels at 10 pixels"
            );
        }
    }

    #[test]
    fn large_text_is_smoothed() {
        let glyph = font().glyph('o', Style::Classic, 24.0);
        assert!(glyph.coverage.iter().any(|&cover| cover > 0 && cover < 255));
    }

    #[test]
    fn letters_have_ink_and_spaces_have_none() {
        let font = font();
        let letter = font.glyph('W', Style::Classic, 10.0);
        assert!(letter.coverage.contains(&255));
        assert!(letter.advance > 0);
        let space = font.glyph(' ', Style::Classic, 10.0);
        assert!(space.coverage.iter().all(|&cover| cover == 0));
        assert!(space.advance > 0);
    }

    #[test]
    fn a_line_is_as_wide_as_its_letters_and_kerning() {
        let font = font();
        let letters: i32 = "AV"
            .chars()
            .map(|c| font.glyph(c, Style::Classic, 13.0).advance)
            .sum();
        assert_eq!(
            font.advance("AV", Style::Classic, 13.0),
            letters + font.kerning('A', 'V', Style::Classic, 13.0)
        );
        assert_eq!(font.advance("", Style::Classic, 13.0), 0);
    }

    #[test]
    fn a_run_places_each_letter_where_the_pen_stands() {
        let font = font();
        let run = font.run("AVA", Style::Classic, 13.0, 0);
        assert_eq!(run.len(), 3);
        assert_eq!(run[0].0, 0);
        assert_eq!(
            run[1].0,
            run[0].1.advance + font.kerning('A', 'V', Style::Classic, 13.0),
            "the second letter starts after the first and their kerning"
        );
        let end = run[2].0 + run[2].1.advance;
        assert_eq!(end, font.advance("AVA", Style::Classic, 13.0));
    }

    #[test]
    fn a_face_drawn_twice_for_bold_is_a_pixel_wider() {
        let font = font();
        assert_eq!(
            font.width("Song", Style::Classic, 10.0, 1),
            font.width("Song", Style::Classic, 10.0, 0) + 1
        );
        assert_eq!(font.width("", Style::Classic, 10.0, 1), 0);
    }

    #[test]
    fn a_skin_size_draws_at_three_quarters_rounded_for_classic_text_only() {
        assert_eq!(em(Style::Classic, 14), 11.0);
        assert_eq!(em(Style::Classic, 13), 10.0);
        assert_eq!(em(Style::Classic, 0), 1.0);
        assert_eq!(em(Style::Modern, 14), 10.5);
        assert_eq!(em(Style::Modern, 16), 12.0);
    }

    #[test]
    fn modern_text_is_smoothed_at_every_size() {
        let font = font();
        for em in [7.5, 10.5, 12.0, 18.0] {
            let glyph = font.glyph('o', Style::Modern, em);
            assert!(
                glyph.coverage.iter().any(|&cover| cover > 0 && cover < 255),
                "an o at {em} pixels has grey pixels"
            );
        }
    }

    #[test]
    fn a_modern_stem_is_as_dark_as_the_share_of_the_pixel_it_covers() {
        // a bar 0.75 of a pixel wide from a pixel's left edge, and one across two pixels
        let mut path = Path::fine(12.0);
        path.move_to(1.0, 0.0);
        path.line_to(1.0, 3.0);
        path.line_to(1.75, 3.0);
        path.line_to(1.75, 0.0);
        path.move_to(3.5, 0.0);
        path.line_to(3.5, 3.0);
        path.line_to(4.5, 3.0);
        path.line_to(4.5, 0.0);
        let glyph = path.fill_area();
        let row: Vec<u8> = glyph.coverage[glyph.w..2 * glyph.w].to_vec();
        let at = |x: i32| row[(x - glyph.left) as usize];
        assert_eq!(at(1), 191);
        assert_eq!((at(3), at(4)), (128, 128));
        assert_eq!(at(2), 0);
    }

    #[test]
    fn modern_kerning_shrinks_below_25_pixels_per_em() {
        let font = font();
        // A and V kern by about 1.8 pixels at 24 pixels per em; at 10.5 the pair is scaled by
        // 11/25 and rounds away
        assert_eq!(font.kerning('A', 'V', Style::Modern, 24.0), -2);
        assert_eq!(font.kerning('A', 'V', Style::Classic, 10.0), -1);
        assert_eq!(font.kerning('A', 'V', Style::Modern, 10.5), 0);
        assert_eq!(font.kerning('T', 'o', Style::Modern, 10.5), -1);
    }

    #[test]
    fn modern_bold_moves_a_pixel_further_after_each_letter() {
        let font = font();
        let plain = font.run("Song", Style::Modern, 10.5, 0);
        let bold = font.run("Song", Style::Modern, 10.5, 1);
        for (i, ((a, _), (b, _))) in plain.iter().zip(&bold).enumerate() {
            assert_eq!(*b, a + i as i32);
        }
        assert_eq!(
            font.width("Song", Style::Modern, 10.5, 1),
            font.width("Song", Style::Modern, 10.5, 0) + 4
        );
    }

    #[test]
    fn a_modern_line_reaches_three_pixels_above_a_capital() {
        let font = font();
        let (ascent, descent) = font.line(Style::Modern, 10.5);
        let cap = font.glyph('H', Style::Modern, 10.5);
        assert_eq!(
            ascent as i32,
            cap.top + 3,
            "the H's bitmap top is its fitted height"
        );
        assert!((1.0..4.0).contains(&descent), "{descent}");
    }

    #[test]
    fn letters_advance_by_whole_pixels_the_font_tables_name() {
        // Liberation Sans carries device widths for 10 pixels: the line is their sum
        let font = font();
        let line = "1. Artist - Title";
        let widths: i32 = line
            .chars()
            .map(|c| font.glyph(c, Style::Classic, 10.0).advance)
            .sum();
        let kerns: i32 = line
            .chars()
            .zip(line.chars().skip(1))
            .map(|(a, b)| font.kerning(a, b, Style::Classic, 10.0))
            .sum();
        assert_eq!(font.advance(line, Style::Classic, 10.0), widths + kerns);
        assert!(
            font.glyph('m', Style::Classic, 10.0).advance
                > font.glyph('i', Style::Classic, 10.0).advance
        );
    }

    #[test]
    fn a_thin_stroke_between_two_rows_lights_one_of_them() {
        // a bar a quarter of a pixel thick, between the centers of two rows
        let mut path = Path::new(10);
        path.move_to(0.0, 0.875);
        path.line_to(4.0, 0.875);
        path.line_to(4.0, 1.125);
        path.line_to(0.0, 1.125);
        let glyph = path.fill(false);
        let rows = glyph
            .coverage
            .chunks(glyph.w)
            .filter(|row| row.contains(&255))
            .count();
        assert_eq!(rows, 1);
    }

    #[test]
    fn what_is_not_a_font_does_not_parse() {
        assert!(Font::parse(Rc::from(&b"not a font"[..])).is_none());
    }
}
