// SPDX-License-Identifier: GPL-3.0-or-later

//! What the renderer draws, in order, in layout pixels: the draw list and the pieces it is made
//! of (rectangles, parts of skin pictures, colors and tints, fonts, regions).

use std::rc::Rc;

/// A rectangle in layout pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// The left edge.
    pub left: i32,
    /// The top edge.
    pub top: i32,
    /// The width; may be 0 or negative for a rectangle that covers nothing.
    pub width: i32,
    /// The height.
    pub height: i32,
}

impl Rect {
    /// The rectangle with every edge times `scale`, rounded, so that rectangles sharing an
    /// edge still share it.
    #[must_use]
    pub fn scaled(&self, scale: f32) -> Rect {
        let at = |v: i32| (v as f32 * scale).round() as i32;
        let (left, top) = (at(self.left), at(self.top));
        Rect::new(left, top, at(self.right()) - left, at(self.bottom()) - top)
    }

    /// A rectangle covering nothing at the origin.
    pub const EMPTY: Rect = Rect {
        left: 0,
        top: 0,
        width: 0,
        height: 0,
    };

    /// `left`, `top`, `width`, `height`.
    pub const fn new(left: i32, top: i32, width: i32, height: i32) -> Rect {
        Rect {
            left,
            top,
            width,
            height,
        }
    }

    /// The right edge, `left + width`.
    pub fn right(&self) -> i32 {
        self.left.wrapping_add(self.width)
    }

    /// The bottom edge, `top + height`.
    pub fn bottom(&self) -> i32 {
        self.top.wrapping_add(self.height)
    }

    /// Whether the point lies inside.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right() && y >= self.top && y < self.bottom()
    }

    /// Whether this rectangle and `r` share at least one pixel.
    pub fn overlaps(&self, r: &Rect) -> bool {
        r.left < self.right()
            && self.left < r.right()
            && r.top < self.bottom()
            && self.top < r.bottom()
    }

    /// The part of this rectangle inside `c`; the width or height goes negative when they are apart.
    #[must_use]
    pub fn cut_to(&self, c: &Rect) -> Rect {
        let l = self.left.max(c.left);
        let t = self.top.max(c.top);
        Rect::new(
            l,
            t,
            self.right().min(c.right()) - l,
            self.bottom().min(c.bottom()) - t,
        )
    }
}

/// A part of a skin image: the file and the source rectangle inside it, or no size for the
/// whole file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BitmapDef {
    /// The element id, lowercase.
    pub id: Rc<str>,
    /// The picture file in the skin.
    pub file: Rc<str>,
    /// The left of the part inside the file.
    pub x: i32,
    /// The top of the part inside the file.
    pub y: i32,
    /// The part's width, or `None` for the rest of the file.
    pub w: Option<i32>,
    /// The part's height, or `None` for the rest of the file.
    pub h: Option<i32>,
    /// The color theme group it is tinted by, lowercase.
    pub gamma_group: Option<Rc<str>>,
}

impl BitmapDef {
    /// A part of `file` under `id`.
    pub fn new(
        id: &str,
        file: &str,
        x: i32,
        y: i32,
        w: Option<i32>,
        h: Option<i32>,
        gamma_group: Option<&str>,
    ) -> Self {
        BitmapDef {
            id: Rc::from(id),
            file: Rc::from(file),
            x,
            y,
            w,
            h,
            gamma_group: gamma_group.map(Rc::from),
        }
    }

    /// The same part with another source rectangle.
    #[must_use]
    pub fn part(&self, x: i32, y: i32, w: Option<i32>, h: Option<i32>) -> Self {
        BitmapDef {
            id: self.id.clone(),
            file: self.file.clone(),
            x,
            y,
            w,
            h,
            gamma_group: self.gamma_group.clone(),
        }
    }
}

/// A color theme's tint for one gamma group: per-channel factors in 1/4096 steps around 1, an
/// optional grey conversion first (1 = brightest channel, 2 = average). `boost` brightens before
/// the tint (see [`Gamma::apply`]), with or without a tint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gamma {
    /// The red factor.
    pub r: i32,
    /// The green factor.
    pub g: i32,
    /// The blue factor.
    pub b: i32,
    /// How the pixel is brightened before the tint: 0 not at all, 1 into the upper half, 2 fourfold.
    pub boost: i32,
    /// The grey conversion: 0 none, 1 the brightest channel, 2 the average.
    pub gray: i32,
}

impl Gamma {
    /// Whether it changes nothing. A boost brightens even with no tint: MMD3's "Thinger" group
    /// is 0,0,0 with boost 1 in its default theme, and Winamp 5.666 draws the component bucket
    /// lifted there.
    pub fn is_identity(&self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0 && self.gray == 0 && self.boost == 0
    }

    /// `argb` tinted.
    pub fn apply(&self, argb: u32) -> u32 {
        let mut red = ((argb >> 16) & 0xff) as i32;
        let mut green = ((argb >> 8) & 0xff) as i32;
        let mut blue = (argb & 0xff) as i32;
        if self.gray != 0 {
            red = if self.gray == 2 {
                (red + green + blue) / 3
            } else {
                red.max(green).max(blue)
            };
            green = red;
            blue = red;
        }
        // boost brightens before the tint. 1 squeezes the pixel into the upper half (c / 2 + 128:
        // Winamp 5.666's black menu text under MonoDark and MMD3's green labels under "silver1 |
        // xblue" both fit); 2 adds 32 and quadruples, as Webamp Modern does (only seen with
        // near-black tints, where it matches)
        let lifted = |c: i32| -> f64 {
            match self.boost {
                1 => c as f64 / 2.0 + 128.0,
                2 => (c + 32) as f64 * 4.0,
                _ => c as f64,
            }
        };
        let ch = |c: i32, f: i32| -> u32 {
            ((lifted(c) * (1.0 + f as f64 / 4096.0)) as i32).clamp(0, 255) as u32
        };
        (argb & 0xff00_0000) | (ch(red, self.r) << 16) | (ch(green, self.g) << 8) | ch(blue, self.b)
    }
}

/// A bitmap font a skin declares: a sheet of cells, each a character.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BitmapFontDef {
    /// The font id, lowercase.
    pub id: Rc<str>,
    /// The sheet file.
    pub file: Rc<str>,
    /// A cell's width.
    pub char_width: i32,
    /// A cell's height.
    pub char_height: i32,
    /// Pixels between characters across.
    pub h_spacing: i32,
    /// Pixels between rows.
    pub v_spacing: i32,
    /// Where the glyph sheet starts inside the file, when the font names a bitmap element.
    pub ox: i32,
    /// See `ox`.
    pub oy: i32,
    /// The color theme group of the sheet's element.
    pub gamma_group: Option<Rc<str>>,
}

const FONT_ROWS: [&str; 2] = [
    "abcdefghijklmnopqrstuvwxyz\"@  ",
    "0123456789….:()-'!_+\\/[]^&%,=$#\nâöä?*",
];

impl BitmapFontDef {
    /// Which cell a character sits in (column, row): the rows of a Winamp bitmap font. A space
    /// advances but draws nothing; its cell is not blank in every skin.
    pub fn cell(&self, c: char) -> Option<(i32, i32)> {
        if c == ' ' {
            return None;
        }
        let lower = c.to_lowercase().next().unwrap_or(c);
        for (row, chars) in FONT_ROWS.iter().enumerate() {
            if let Some(col) = chars.chars().position(|x| x == lower) {
                return Some((col as i32, row as i32));
            }
        }
        None
    }
}

/// A TrueType font a skin declares: its `id`, and the `file` in the skin, empty for a system font.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontDef {
    /// The font id, lowercase.
    pub id: Rc<str>,
    /// The TrueType file in the skin, or empty for a system font named by its id.
    pub file: Rc<str>,
}

impl FontDef {
    /// A font `id` from `file`.
    pub fn new(id: &str, file: &str) -> Self {
        FontDef {
            id: Rc::from(id),
            file: Rc::from(file),
        }
    }
}

/// RGB color as `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rgb(pub u32);

/// How text sits across its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    /// At the left edge.
    Left,
    /// In the middle.
    Center,
    /// At the right edge.
    Right,
}

/// How text sits up and down in its box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VAlign {
    /// At the top.
    Top,
    /// In the middle.
    Center,
    /// At the bottom.
    Bottom,
}

/// A run of text as the renderer draws it; see [`DrawCmd::Text`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextCmd {
    /// The characters.
    pub text: String,
    /// The font, or `None` for the platform's default.
    pub font: Option<FontDef>,
    /// The skin's font size.
    pub size: i32,
    /// The color.
    pub color: Rgb,
    /// The box it is clipped to.
    pub rect: Rect,
    /// How it sits across the box.
    pub align: Align,
    /// Pixels to shift the text left by, for a ticker.
    pub scroll: i32,
    /// Opacity 0..255.
    pub alpha: i32,
    /// How it sits up and down in the box.
    pub valign: VAlign,
    /// Drawn bold.
    pub bold: bool,
    /// A shadow color, when there is a shadow.
    pub shadow: Option<Rgb>,
    /// Where the shadow sits from the text, across.
    pub shadow_dx: i32,
    /// Where the shadow sits from the text, down.
    pub shadow_dy: i32,
    /// Drawn smoothed, as Winamp draws a modern skin's TrueType text unless the skin sets
    /// `antialias="0"`; unsmoothed, as Windows draws it, otherwise and in a classic skin. The
    /// engine measured and placed it the same way (see
    /// [`SkinHost::smooth_text_advance`](crate::host::SkinHost::smooth_text_advance)). Not
    /// carried in the packed draw list.
    pub smooth: bool,
    /// Where the text starts across, in layout pixels: aligned in `rect` by its measured width,
    /// less `scroll`. The engine fills it in when the frame is drawn.
    pub x: f32,
    /// The baseline, in layout pixels: placed in `rect` by `valign` and the font's line.
    pub baseline: f32,
}

/// Text in a skin's bitmap font. The engine spells it out as clips and images before a host
/// sees the list, so a renderer draws nothing for it.
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapTextCmd {
    /// The characters.
    pub text: String,
    /// The font.
    pub font: Rc<BitmapFontDef>,
    /// The box.
    pub rect: Rect,
    /// How it sits across the box.
    pub align: Align,
    /// Opacity 0..255.
    pub alpha: i32,
    /// How it sits up and down.
    pub valign: VAlign,
    /// The advance of a colon in a time display, when the skin narrows it.
    pub colon_width: Option<i32>,
    /// Pixels to shift the text left by, for a ticker.
    pub scroll: i32,
}

/// What the renderer draws, in order, in layout pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCmd {
    /// A bitmap, or the part of it named, stretched to `dst`.
    Image {
        /// The part of the picture.
        bitmap: BitmapDef,
        /// Where it goes.
        dst: Rect,
        /// Opacity 0..255.
        alpha: i32,
        /// The color theme's tint for this bitmap, filled in when the frame is drawn.
        gamma: Option<Gamma>,
        /// Scale with filtering instead of nearest pixels: Winamp shrinks a list's thumb picture that way.
        smooth: bool,
    },
    /// `rect` filled with `color` at `alpha`, 0..255.
    Fill {
        /// The box.
        rect: Rect,
        /// The color.
        color: Rgb,
        /// Opacity.
        alpha: i32,
    },
    /// Start of a shaped window: what follows until [`DrawCmd::WindowEnd`] shows only inside the region.
    WindowStart(Region),
    /// End of the shaped window [`DrawCmd::WindowStart`] began.
    WindowEnd,
    /// A run of text.
    Text(Box<TextCmd>),
    /// Text in a bitmap font, spelled out before a host sees it.
    BitmapText(Box<BitmapTextCmd>),
    /// Clip the commands that follow, until the matching [`DrawCmd::Unclip`].
    Clip(Rect),
    /// The end of the last [`DrawCmd::Clip`].
    Unclip,
    /// A small ARGB picture (`w` x `h`, rows top first) stretched over `rect` with nearest
    /// pixels: the vis, the EQ graph and gradients.
    Pixels {
        /// Where it goes.
        rect: Rect,
        /// The picture's width.
        w: i32,
        /// The picture's height.
        h: i32,
        /// The pixels, ARGB.
        argb: Rc<[u32]>,
        /// Opacity.
        alpha: i32,
        /// Whether the same pixels come back unchanged in later frames until the picture changes.
        stable: bool,
    },
}

impl DrawCmd {
    /// The command drawn `scale` times as large, from the window's corner.
    #[must_use]
    pub fn scaled(&self, scale: f32) -> DrawCmd {
        let mut cmd = self.clone();
        match &mut cmd {
            DrawCmd::Image { dst, .. } => *dst = dst.scaled(scale),
            DrawCmd::Fill { rect, .. } | DrawCmd::Clip(rect) | DrawCmd::Pixels { rect, .. } => {
                *rect = rect.scaled(scale);
            }
            DrawCmd::WindowStart(region) => *region = region.scaled(scale),
            DrawCmd::Text(t) => {
                t.rect = t.rect.scaled(scale);
                t.size = (t.size as f32 * scale).round() as i32;
                t.x *= scale;
                t.baseline *= scale;
                t.scroll = (t.scroll as f32 * scale).round() as i32;
                t.shadow_dx = (t.shadow_dx as f32 * scale).round() as i32;
                t.shadow_dy = (t.shadow_dy as f32 * scale).round() as i32;
            }
            DrawCmd::BitmapText(t) => t.rect = t.rect.scaled(scale),
            DrawCmd::WindowEnd | DrawCmd::Unclip => {}
        }
        cmd
    }

    /// Whether `other` draws exactly what this draws: the same kind of command with the same
    /// values, the same part of the same picture in the same tint, the same font, and pixels
    /// with the same content. A renderer may then keep what it drew for one and show it for the
    /// other. A window's shape is the same when it is made of the same pictures, boxes and
    /// operations, whether or not the engine worked it out again; bitmap text, which a renderer
    /// never draws, is the same text in the same box.
    pub fn draws_same_as(&self, other: &DrawCmd) -> bool {
        match (self, other) {
            (DrawCmd::BitmapText(a), DrawCmd::BitmapText(b)) => {
                a.text == b.text && a.rect == b.rect
            }
            (
                DrawCmd::Pixels {
                    rect,
                    w,
                    h,
                    argb,
                    alpha,
                    stable,
                },
                DrawCmd::Pixels {
                    rect: r2,
                    w: w2,
                    h: h2,
                    argb: a2,
                    alpha: al2,
                    stable: s2,
                },
            ) => {
                rect == r2
                    && w == w2
                    && h == h2
                    && alpha == al2
                    && stable == s2
                    && (Rc::ptr_eq(argb, a2) || argb == a2)
            }
            _ => self == other,
        }
    }

    /// An image command with no tint yet.
    pub fn image(bitmap: BitmapDef, dst: Rect, alpha: i32) -> DrawCmd {
        DrawCmd::Image {
            bitmap,
            dst,
            alpha,
            gamma: None,
            smooth: false,
        }
    }

    /// Whether a pixel of a region picture marks a cut: opaque, whatever its color. Winamp
    /// Modern's main window marks its corners in black, its shaded playlist in white.
    pub fn is_cut_pixel(argb: u32) -> bool {
        (argb >> 24) >= 128
    }
}

/// A picture in a region: the same fields as [`DrawCmd::Image`].
#[derive(Debug, Clone, PartialEq)]
pub struct RegionImage {
    /// The part of the picture.
    pub bitmap: BitmapDef,
    /// Where it goes.
    pub dst: Rect,
    /// Opacity.
    pub alpha: i32,
    /// The tint.
    pub gamma: Option<Gamma>,
    /// See [`DrawCmd::Image`].
    pub smooth: bool,
}

/// A window's shape as Winamp composes it: a picture's opaque pixels, a rectangle, or an
/// object's region with its children's added, intersected and cut away.
#[derive(Debug, Clone, PartialEq)]
pub enum Region {
    /// The opaque pixels of a picture.
    Pic(RegionImage),
    /// The whole of a rectangle.
    Box(Rect),
    /// `base` (nothing when `None`), kept only where `keep` is when that is not empty, plus `add`, less `cut`.
    Op {
        /// The region it starts from.
        base: Option<Box<Region>>,
        /// Regions added.
        add: Vec<Region>,
        /// Regions it is kept to.
        keep: Vec<Region>,
        /// Regions cut away.
        cut: Vec<Region>,
    },
}

impl Region {
    /// The region `scale` times as large, from the window's corner.
    #[must_use]
    pub fn scaled(&self, scale: f32) -> Region {
        match self {
            Region::Pic(img) => Region::Pic(RegionImage {
                dst: img.dst.scaled(scale),
                ..img.clone()
            }),
            Region::Box(r) => Region::Box(r.scaled(scale)),
            Region::Op {
                base,
                add,
                keep,
                cut,
            } => {
                let all = |v: &[Region]| v.iter().map(|r| r.scaled(scale)).collect();
                Region::Op {
                    base: base.as_ref().map(|b| Box::new(b.scaled(scale))),
                    add: all(add),
                    keep: all(keep),
                    cut: all(cut),
                }
            }
        }
    }

    /// The same region with every picture passed through `f` (dropped when it gives `None`).
    pub fn map_pics(
        &self,
        f: &mut dyn FnMut(&RegionImage) -> Option<RegionImage>,
    ) -> Option<Region> {
        match self {
            Region::Pic(img) => f(img).map(Region::Pic),
            Region::Box(_) => Some(self.clone()),
            Region::Op {
                base,
                add,
                keep,
                cut,
            } => Some(Region::Op {
                base: base.as_ref().and_then(|b| b.map_pics(f)).map(Box::new),
                add: add.iter().filter_map(|r| r.map_pics(f)).collect(),
                keep: keep.iter().filter_map(|r| r.map_pics(f)).collect(),
                cut: cut.iter().filter_map(|r| r.map_pics(f)).collect(),
            }),
        }
    }

    /// Every picture in it, for a tint to be filled in.
    pub fn pics_mut(&mut self, f: &mut dyn FnMut(&mut RegionImage)) {
        match self {
            Region::Pic(img) => f(img),
            Region::Box(_) => {}
            Region::Op {
                base,
                add,
                keep,
                cut,
            } => {
                if let Some(b) = base {
                    b.pics_mut(f);
                }
                for r in add.iter_mut().chain(keep.iter_mut()).chain(cut.iter_mut()) {
                    r.pics_mut(f);
                }
            }
        }
    }

    /// The box around it, or `None` when it holds nothing.
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Region::Pic(img) => Some(img.dst),
            Region::Box(r) => Some(*r),
            Region::Op { base, add, .. } => {
                let mut all = base
                    .as_ref()
                    .and_then(|b| b.bounds())
                    .into_iter()
                    .collect::<Vec<_>>();
                all.extend(add.iter().filter_map(|r| r.bounds()));
                all.into_iter().reduce(|a, b| {
                    let l = a.left.min(b.left);
                    let t = a.top.min(b.top);
                    Rect::new(
                        l,
                        t,
                        a.right().max(b.right()) - l,
                        a.bottom().max(b.bottom()) - t,
                    )
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tint_lifts_and_scales() {
        let g = Gamma {
            r: 0,
            g: 0,
            b: 0,
            boost: 1,
            gray: 0,
        };
        assert_eq!(g.apply(0xff00_0000), 0xff80_8080);
        let g = Gamma {
            r: 4096,
            g: 0,
            b: -4096,
            boost: 0,
            gray: 0,
        };
        assert_eq!(g.apply(0xff40_4040), 0xff80_4000);
    }

    #[test]
    fn bitmap_font_cells() {
        let f = BitmapFontDef {
            id: Rc::from("f"),
            file: Rc::from("f.png"),
            char_width: 5,
            char_height: 6,
            h_spacing: 0,
            v_spacing: 0,
            ox: 0,
            oy: 0,
            gamma_group: None,
        };
        assert_eq!(f.cell('A'), Some((0, 0)));
        assert_eq!(f.cell(':'), Some((12, 1)));
        assert_eq!(f.cell(' '), None);
    }
}
