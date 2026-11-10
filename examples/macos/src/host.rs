// SPDX-License-Identifier: GPL-3.0-or-later

//! The host: the skin's pictures, decoded with the `image` crate, and its text, measured and
//! drawn with `fontdue`. The engine reads them through [`Host`]; the renderer reads the same
//! [`Assets`], so each picture and font is decoded once.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fontdue::{Font, FontSettings, Metrics};
use walkit::draw::FontDef;
use walkit::files::SkinFiles;
use walkit::gdi::{em_size, FontFace};
use walkit::host::SkinHost;

/// Where macOS keeps the fonts a skin names most.
const SUPPLEMENTAL: &str = "/System/Library/Fonts/Supplemental";

/// The font for text that names none, and the one a missing font falls back to.
const DEFAULT_FONTS: [&str; 2] = [
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
];

/// Its bold cut.
const DEFAULT_BOLD_FONTS: [&str; 2] = [
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
];

/// A decoded picture: its size and its pixels as ARGB, rows top first.
pub struct Picture {
    pub w: i32,
    pub h: i32,
    pub argb: Vec<u32>,
}

impl Picture {
    /// The pixel at `x`, `y`, or `None` outside the picture.
    pub fn at(&self, x: i32, y: i32) -> Option<u32> {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        Some(self.argb[(y * self.w + x) as usize])
    }
}

/// A drawn glyph: where it sits from the pen, and its coverage, one byte per pixel.
pub type Glyph = Rc<(Metrics, Vec<u8>)>;

/// A font with the glyphs drawn so far, by character and pixel size.
pub struct FontEntry {
    pub font: Font,
    glyphs: RefCell<HashMap<(char, u32), Glyph>>,
}

impl FontEntry {
    fn new(font: Font) -> Rc<FontEntry> {
        Rc::new(FontEntry {
            font,
            glyphs: RefCell::default(),
        })
    }

    /// The coverage bitmap of `c` at `px` pixels, drawn once.
    pub fn glyph(&self, c: char, px: f32) -> Glyph {
        let key = (c, px.to_bits());
        self.glyphs
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| Rc::new(self.font.rasterize(c, px)))
            .clone()
    }

    /// How far `text` advances at `px` pixels, with kerning.
    pub fn advance(&self, text: &str, px: f32) -> f32 {
        let mut width = 0.0;
        let mut last = None;
        for c in text.chars() {
            if let Some(prev) = last {
                width += self.font.horizontal_kern(prev, c, px).unwrap_or(0.0);
            }
            width += self.font.metrics(c, px).advance_width;
            last = Some(c);
        }
        width
    }
}

/// The font a run of text draws in, and whether the renderer thickens it for bold.
pub struct TextFace {
    pub font: Rc<FontEntry>,
    pub fake_bold: bool,
}

/// The skin's files, and the pictures and fonts read from them so far.
pub struct Assets {
    files: Box<dyn SkinFiles>,
    pictures: HashMap<String, Option<Rc<Picture>>>,
    fonts: HashMap<String, Option<Rc<FontEntry>>>,
}

impl Assets {
    pub fn new(files: Box<dyn SkinFiles>) -> Self {
        Assets {
            files,
            pictures: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    /// The picture at `file`, decoded the first time it is asked for. Paths are looked up
    /// without regard to case.
    pub fn picture(&mut self, file: &str) -> Option<Rc<Picture>> {
        let key = file.to_lowercase();
        if let Some(p) = self.pictures.get(&key) {
            return p.clone();
        }
        let decoded = self.files.read(file).and_then(|b| decode(&b)).map(Rc::new);
        self.pictures.insert(key, decoded.clone());
        decoded
    }

    /// The font `font` draws in, `bold` or not, as the engine names its face; `None` only
    /// when not even the default font loads.
    pub fn face(&mut self, font: Option<&FontDef>, bold: bool) -> Option<TextFace> {
        let face = FontFace::of(font, bold);
        let (found, fake_bold) = match &face {
            // a Windows font the skin names without a file: the macOS font of that name
            FontFace::StandIn { .. } => {
                let file = face.file().unwrap_or_default();
                let found = stand_in_path(&file).and_then(|p| self.system_font(&p));
                (found, false)
            }
            FontFace::SkinFile { font, fake_bold } => {
                let found = self.skin_font(&font.file);
                (found, *fake_bold)
            }
            FontFace::Default { fake_bold } => (None, *fake_bold),
        };
        if let Some(font) = found {
            return Some(TextFace { font, fake_bold });
        }
        // the default font, in its own bold cut
        let list = if fake_bold {
            DEFAULT_BOLD_FONTS
        } else {
            DEFAULT_FONTS
        };
        let font = list.iter().find_map(|p| self.system_font(p))?;
        Some(TextFace {
            font,
            fake_bold: false,
        })
    }

    /// A TrueType file inside the skin.
    fn skin_font(&mut self, file: &str) -> Option<Rc<FontEntry>> {
        if file.is_empty() {
            return None;
        }
        let key = format!("skin:{}", file.to_lowercase());
        if let Some(f) = self.fonts.get(&key) {
            return f.clone();
        }
        let font = self.files.read(file).and_then(|b| parse_font(&b));
        self.fonts.insert(key, font.clone());
        font
    }

    /// A font file of the system.
    fn system_font(&mut self, path: &str) -> Option<Rc<FontEntry>> {
        if let Some(f) = self.fonts.get(path) {
            return f.clone();
        }
        let font = std::fs::read(path).ok().and_then(|b| parse_font(&b));
        self.fonts.insert(path.to_string(), font.clone());
        font
    }
}

/// Decodes a PNG, BMP, GIF or JPEG into ARGB.
fn decode(bytes: &[u8]) -> Option<Picture> {
    let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let argb = rgba
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            (u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
        })
        .collect();
    Some(Picture {
        w: w as i32,
        h: h as i32,
        argb,
    })
}

fn parse_font(bytes: &[u8]) -> Option<Rc<FontEntry>> {
    Font::from_bytes(bytes, FontSettings::default())
        .ok()
        .map(FontEntry::new)
}

/// The engine names the free fonts that stand in for Arial and Tahoma; macOS has the fonts
/// themselves.
fn stand_in_path(file: &str) -> Option<String> {
    let name = file.to_lowercase();
    let path = match name.as_str() {
        "fonts/liberationsans.ttf" => "Arial.ttf",
        "fonts/liberationsans-bold.ttf" => "Arial Bold.ttf",
        "fonts/dejavusanscondensed.ttf" => "Tahoma.ttf",
        "fonts/dejavusanscondensed-bold.ttf" => "Tahoma Bold.ttf",
        _ => return None,
    };
    Some(format!("{SUPPLEMENTAL}/{path}"))
}

/// What the engine asks of the app. It shares [`Assets`] with the renderer, and raises
/// `redraw` when the engine asks for a frame.
pub struct Host {
    pub assets: Rc<RefCell<Assets>>,
    pub redraw: Rc<Cell<bool>>,
}

impl Host {
    /// How wide `text` draws in layout pixels: Winamp draws a font at three quarters of the
    /// skin's size ([`em_size`]).
    fn measure(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        let face = self.assets.borrow_mut().face(font, bold);
        match face {
            Some(face) => face.font.advance(text, em_size(size)),
            None => text.chars().count() as f32 * size as f32 * 0.6,
        }
    }
}

impl SkinHost for Host {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        let p = self.assets.borrow_mut().picture(file)?;
        Some((p.w, p.h))
    }

    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        self.assets.borrow_mut().picture(file)?.at(x, y)
    }

    fn pixels(&mut self, file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
        let p = self.assets.borrow_mut().picture(file)?;
        let mut out = Vec::with_capacity((w.max(0) * h.max(0)) as usize);
        for row in y..y + h {
            for col in x..x + w {
                out.push(p.at(col, row)?);
            }
        }
        Some(out)
    }

    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        self.measure(text, font, size, bold).ceil() as i32
    }

    fn text_advance(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> f32 {
        self.measure(text, font, size, bold)
    }

    fn line_metrics(
        &mut self,
        font: Option<&FontDef>,
        size: i32,
        bold: bool,
    ) -> Option<(f32, f32)> {
        let face = self.assets.borrow_mut().face(font, bold)?;
        let line = face.font.font.horizontal_line_metrics(em_size(size))?;
        Some((line.ascent, -line.descent))
    }

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        let path = stand_in_path(file)?;
        std::fs::read(path).ok().map(Rc::from)
    }

    fn log(&mut self, message: &str) {
        eprintln!("skin: {message}");
    }

    fn invalidate(&mut self) {
        self.redraw.set(true);
    }
}
