// SPDX-License-Identifier: GPL-3.0-or-later
//! Text as Windows draws a classic skin's: a TrueType font's outlines hinted to the pixel grid
//! (skrifa's autohinter, for one-bit pixels), each pixel the outline covers at its centre filled
//! and the rest left clear, the advances rounded to whole pixels and no kerning, as GDI's
//! `DrawText` lays out a line. A host enlarges the result unfiltered, as it does the skin's
//! pictures.

use std::cell::RefCell;
use std::collections::HashMap;

use jni::errors::{Error, Result, ThrowRuntimeExAndDefault};
use jni::objects::{JByteArray, JClass, JIntArray, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::{jni_mangle, EnvUnowned};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlineGlyphCollection, OutlinePen,
    Target,
};
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::bridge::jints;

/// Straight pieces a curve is cut into: plenty for letters a dozen pixels tall.
const CURVE_STEPS: usize = 8;

/// A font's bytes and its outlines hinted at each size asked for so far.
pub struct HintedFont {
    data: Vec<u8>,
    hinted: RefCell<HashMap<u32, HintingInstance>>,
}

impl HintedFont {
    /// The font in `data`, or `None` when it is not one skrifa reads.
    pub fn new(data: Vec<u8>) -> Option<HintedFont> {
        FontRef::new(&data).ok()?;
        Some(HintedFont {
            data,
            hinted: RefCell::new(HashMap::new()),
        })
    }

    fn font(&self) -> FontRef<'_> {
        FontRef::new(&self.data).expect("checked in new")
    }

    /// Whether the font has a glyph for every character of `text` but spaces and controls.
    pub fn covers(&self, text: &str) -> bool {
        let map = self.font().charmap();
        text.chars()
            .all(|c| c.is_whitespace() || c.is_control() || map.map(c).is_some())
    }

    /// The glyphs of `text` and where each starts across, from 0, at an em of `em` pixels; and
    /// the line's width.
    fn layout(&self, text: &str, em: f32) -> (Vec<(GlyphId, i32)>, i32) {
        let font = self.font();
        let map = font.charmap();
        let metrics = font.glyph_metrics(Size::new(em), LocationRef::default());
        let mut x = 0;
        let mut glyphs = Vec::new();
        for c in text.chars() {
            let gid = map.map(c).unwrap_or_default();
            glyphs.push((gid, x));
            x += metrics.advance_width(gid).unwrap_or(0.0).round() as i32;
        }
        (glyphs, x)
    }

    /// How wide `text` draws at an em of `em` pixels, in whole pixels.
    pub fn width(&self, text: &str, em: f32) -> i32 {
        self.layout(text, em).1
    }

    /// `text` at an em of `em` pixels in a block `w` x `h`, starting at `x` with its baseline at
    /// `baseline`: true where a pixel is ink.
    pub fn draw(
        &self,
        text: &str,
        em: f32,
        (w, h): (i32, i32),
        x: i32,
        baseline: i32,
    ) -> Vec<bool> {
        let mut ink = vec![false; (w.max(0) * h.max(0)) as usize];
        let font = self.font();
        let outlines = font.outline_glyphs();
        let mut hinted = self.hinted.borrow_mut();
        let instance = match hinted.entry(em.to_bits()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(v) => {
                let Ok(i) = hinting(&outlines, em) else {
                    return ink;
                };
                v.insert(i)
            }
        };
        let (glyphs, _) = self.layout(text, em);
        for (gid, gx) in glyphs {
            let Some(glyph) = outlines.get(gid) else {
                continue;
            };
            let mut pen = Edges {
                at: ((x + gx) as f32, baseline as f32),
                ..Edges::default()
            };
            if glyph
                .draw(DrawSettings::hinted(instance, false), &mut pen)
                .is_ok()
            {
                pen.close();
                pen.fill(&mut ink, w, h);
            }
        }
        ink
    }
}

/// The font's outlines hinted at an em of `em` pixels for one-bit pixels.
fn hinting(
    outlines: &OutlineGlyphCollection,
    em: f32,
) -> std::result::Result<HintingInstance, skrifa::outline::DrawError> {
    HintingInstance::new(
        outlines,
        Size::new(em),
        LocationRef::default(),
        HintingOptions {
            engine: Engine::Auto(None),
            target: Target::Mono,
        },
    )
}

/// A glyph's outline as straight edges in the block's pixels, y down from `at`.
#[derive(Default)]
struct Edges {
    at: (f32, f32),
    start: (f32, f32),
    last: (f32, f32),
    lines: Vec<((f32, f32), (f32, f32))>,
}

impl Edges {
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.at.0 + x, self.at.1 - y)
    }

    fn edge(&mut self, to: (f32, f32)) {
        if to != self.last {
            self.lines.push((self.last, to));
        }
        self.last = to;
    }

    /// Fills each pixel of `ink` (`w` x `h`) whose centre the outline winds around.
    fn fill(&self, ink: &mut [bool], w: i32, h: i32) {
        let mut crossings: Vec<(f32, i32)> = Vec::new();
        for row in 0..h {
            let y = row as f32 + 0.5;
            crossings.clear();
            for &((x0, y0), (x1, y1)) in &self.lines {
                if y0 == y1 {
                    continue;
                }
                let (lo, hi) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
                if y < lo || y >= hi {
                    continue;
                }
                let x = x0 + (y - y0) * (x1 - x0) / (y1 - y0);
                crossings.push((x, if y1 > y0 { 1 } else { -1 }));
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                if winding == 0 {
                    continue;
                }
                // the pixels whose centres lie in [left, right)
                let first = (pair[0].0 - 0.5).ceil().max(0.0) as i32;
                let end = ((pair[1].0 - 0.5).ceil() as i32).min(w);
                for col in first..end {
                    ink[(row * w + col) as usize] = true;
                }
            }
        }
    }
}

impl OutlinePen for Edges {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.start = self.point(x, y);
        self.last = self.start;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.edge(p);
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.last, self.point(cx, cy), self.point(x, y));
        for i in 1..=CURVE_STEPS {
            let t = i as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            self.edge((
                u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p1.0,
                u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p1.1,
            ));
        }
    }

    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (p0, c0, c1, p1) = (
            self.last,
            self.point(c0x, c0y),
            self.point(c1x, c1y),
            self.point(x, y),
        );
        for i in 1..=CURVE_STEPS {
            let t = i as f32 / CURVE_STEPS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.edge((
                a * p0.0 + b * c0.0 + c * c1.0 + d * p1.0,
                a * p0.1 + b * c0.1 + c * c1.1 + d * p1.1,
            ));
        }
    }

    fn close(&mut self) {
        let start = self.start;
        self.edge(start);
    }
}

// ---- the Kotlin side's HintedFont ---------------------------------------------------------------

/// The font behind handle `h`.
///
/// # Safety
/// `h` came from `hf_new` and has not been given to `hf_free`: the Kotlin side keeps that.
fn font<'a>(h: jlong) -> Result<&'a HintedFont> {
    if h == 0 {
        return Err(Error::NullPtr("font handle"));
    }
    // SAFETY: the handle is a pointer `hf_new` made with `Box::into_raw`, alive until `hf_free`.
    Ok(unsafe { &*(h as *const HintedFont) })
}

/// A font from its file's bytes; 0 when they are not a font.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn hf_new<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, data: JByteArray<'l>) -> jlong {
    env.with_env(|env| -> Result<jlong> {
        let bytes = env.convert_byte_array(&data)?;
        Ok(HintedFont::new(bytes).map_or(0, |f| Box::into_raw(Box::new(f)) as jlong))
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

/// Drops the font; the handle is dead after.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn hf_free<'l>(mut env: EnvUnowned<'l>, _class: JClass<'l>, h: jlong) {
    env.with_env(|_env| -> Result<()> {
        if h != 0 {
            // SAFETY: the handle is the pointer `hf_new` made, given back once by the Kotlin side.
            drop(unsafe { Box::from_raw(h as *mut HintedFont) });
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}

/// Whether the font has a glyph for every character of `text` but spaces.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn hf_covers<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    text: JString<'l>,
) -> jboolean {
    env.with_env(|_env| -> Result<jboolean> { Ok(font(h)?.covers(&text.to_string())) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// How wide `text` draws at an em of `em` pixels.
#[jni_mangle("nl.mattix.walkit.Native")]
pub fn hf_width<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    text: JString<'l>,
    em: f32,
) -> jint {
    env.with_env(|_env| -> Result<jint> { Ok(font(h)?.width(&text.to_string(), em)) })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `text` at an em of `em` pixels in `argb` on clear pixels, a block `w` x `hgt` with the line
/// starting at `x` and its baseline at `baseline`, rows top first.
#[jni_mangle("nl.mattix.walkit.Native")]
#[allow(clippy::too_many_arguments)]
pub fn hf_draw<'l>(
    mut env: EnvUnowned<'l>,
    _class: JClass<'l>,
    h: jlong,
    text: JString<'l>,
    em: f32,
    argb: jint,
    w: jint,
    hgt: jint,
    x: jint,
    baseline: jint,
) -> JIntArray<'l> {
    env.with_env(|env| -> Result<JIntArray<'l>> {
        let ink = font(h)?.draw(&text.to_string(), em, (w, hgt), x, baseline);
        let px: Vec<i32> = ink.iter().map(|&on| if on { argb } else { 0 }).collect();
        jints(env, &px)
    })
    .resolve::<ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn liberation() -> Option<HintedFont> {
        let path = std::env::var("WALKIT_TEST_FONT").ok()?;
        HintedFont::new(std::fs::read(path).ok()?)
    }

    #[test]
    fn a_square_outline_fills_the_pixels_whose_centres_it_holds() {
        let mut e = Edges::default();
        e.move_to(1.0, 0.0);
        e.line_to(3.0, 0.0);
        e.line_to(3.0, -2.0);
        e.line_to(1.0, -2.0);
        e.close();
        let mut ink = vec![false; 4 * 3];
        e.fill(&mut ink, 4, 3);
        let rows: Vec<String> = ink
            .chunks(4)
            .map(|r| r.iter().map(|&b| if b { '#' } else { '.' }).collect())
            .collect();
        assert_eq!(rows, vec![".##.", ".##.", "...."]);
    }

    #[test]
    fn a_hole_inside_a_letter_stays_clear() {
        // an outer square wound one way and an inner one the other
        let mut e = Edges::default();
        e.move_to(0.0, 0.0);
        e.line_to(5.0, 0.0);
        e.line_to(5.0, -5.0);
        e.line_to(0.0, -5.0);
        e.close();
        e.move_to(2.0, -2.0);
        e.line_to(2.0, -3.0);
        e.line_to(3.0, -3.0);
        e.line_to(3.0, -2.0);
        e.close();
        let mut ink = vec![false; 25];
        e.fill(&mut ink, 5, 5);
        assert!(ink[0] && ink[24]);
        assert!(!ink[2 * 5 + 2], "the hole");
    }

    #[test]
    fn arial_s_stand_in_draws_a_one_pixel_stem_for_an_l_at_twelve_pixels() {
        let Some(f) = liberation() else {
            return;
        };
        assert!(f.covers("Andamp 10:00"));
        assert!(!f.covers("坂本"));
        let ink = f.draw("l", 12.0, (6, 14), 1, 11);
        let stem: usize = (0..6).filter(|&c| ink[5 * 6 + c]).count();
        assert_eq!(stem, 1, "a crisp stem one pixel wide");
        assert_eq!(f.width("10:00", 12.0), 4 * 7 + 3, "digits 7, the colon 3");
    }
}
