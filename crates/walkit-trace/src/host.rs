// SPDX-License-Identifier: GPL-3.0-or-later

//! The host a trace is recorded with: image sizes and pixels from the skin's files, text
//! measured by a fixed rule so no platform's fonts enter the trace, and every call the engine
//! makes written into the trace. The playlist window counts as open and the library as closed,
//! as the reference Winamp had them; a menu is dismissed at once.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use walkit::draw::FontDef;
use walkit::files::SkinFiles;
use walkit::host::{SkinHost, SkinMenu, WindowId};

/// A decoded picture: its size and its pixels as ARGB, row by row.
#[derive(Debug, Clone)]
pub struct Picture {
    /// The width.
    pub w: i32,
    /// The height.
    pub h: i32,
    /// The pixels, ARGB, rows top first.
    pub argb: Vec<u32>,
}

/// Decodes a skin picture from its file name and bytes; `None` when it cannot be read. The
/// platform supplies it: a native run decodes with a library, a browser with its own decoder.
pub type Decoder = Box<dyn FnMut(&str, &[u8]) -> Option<Picture>>;

/// The trace host; see the module.
pub struct TraceHost {
    files: Rc<dyn SkinFiles>,
    out: Rc<RefCell<String>>,
    decoder: Decoder,
    /// The stand-in fonts by path (`fonts/LiberationSans.ttf`), for Windows' line metrics.
    fonts: Option<Rc<dyn SkinFiles>>,
    /// How often the engine asked for a redraw.
    pub invalidates: Rc<RefCell<i64>>,
    images: HashMap<String, Option<Rc<Picture>>>,
}

impl TraceHost {
    /// A host reading `files`, writing into `out`, decoding pictures with `decoder`, and reading
    /// the stand-in fonts from `fonts`.
    pub fn new(
        files: Rc<dyn SkinFiles>,
        out: Rc<RefCell<String>>,
        decoder: Decoder,
        fonts: Option<Rc<dyn SkinFiles>>,
    ) -> Self {
        TraceHost {
            files,
            out,
            decoder,
            fonts,
            invalidates: Rc::new(RefCell::new(0)),
            images: HashMap::new(),
        }
    }

    fn image(&mut self, file: &str) -> Option<Rc<Picture>> {
        let key = file.to_lowercase();
        if let Some(p) = self.images.get(&key) {
            return p.clone();
        }
        let decoded = self
            .files
            .read(file)
            .and_then(|bytes| (self.decoder)(file, &bytes))
            .map(Rc::new);
        self.images.insert(key, decoded.clone());
        decoded
    }

    fn write(&self, line: &str) {
        let mut out = self.out.borrow_mut();
        out.push_str(line);
        out.push('\n');
    }
}

/// A menu as the trace writes it; see `walkit::listing::menu`.
pub fn menu_items(menu: &SkinMenu) -> String {
    walkit::listing::menu(menu)
}

/// The fixed text rule: six tenths of the size per character, a tenth more in bold; no font.
pub fn rule_text_width(text: &str, size: i32, bold: bool) -> i32 {
    let len = text.encode_utf16().count() as i32;
    len.wrapping_mul(size)
        .wrapping_mul(if bold { 7 } else { 6 })
        / 10
}

/// The fixed line rule: eight tenths of the size above the baseline, two below.
pub fn rule_line_metrics(size: i32) -> (f32, f32) {
    (size as f32 * 0.8, size as f32 * 0.2)
}

impl SkinHost for TraceHost {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        self.image(file).map(|p| (p.w, p.h))
    }

    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        let p = self.image(file)?;
        if x < 0 || y < 0 || x >= p.w || y >= p.h {
            return None;
        }
        Some(p.argb[(y * p.w + x) as usize])
    }

    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        rule_text_width(text, size, bold)
    }

    fn line_metrics(
        &mut self,
        _font: Option<&FontDef>,
        size: i32,
        _bold: bool,
    ) -> Option<(f32, f32)> {
        Some(rule_line_metrics(size))
    }

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        self.fonts.as_ref()?.read(file)
    }

    fn log(&mut self, message: &str) {
        self.write(&format!("log {}", message.replace('\n', "\\n")));
    }

    fn invalidate(&mut self) {
        *self.invalidates.borrow_mut() += 1;
    }

    /// Not in the trace: the engine asks every frame, and the answer is the host's, fixed.
    fn window_open(&mut self, kind: &str) -> Option<bool> {
        match kind {
            "pl" => Some(true),
            "ml" => Some(false),
            _ => None,
        }
    }

    fn choose_from_menu(&mut self, menu: &SkinMenu) -> Option<i32> {
        self.write(&format!("menu {}", menu_items(menu)));
        Some(0)
    }

    fn windows_changed(&mut self, windows: &[WindowId]) {
        let ids: Vec<String> = windows.iter().map(|w| w.0.to_string()).collect();
        self.write(&format!("windows {}", ids.join(",")));
    }

    fn host_action(
        &mut self,
        action: &str,
        param: Option<&str>,
        _from: Option<walkit::host::ActionFrom>,
    ) {
        self.write(&format!("hostaction {action} {}", param.unwrap_or("-")));
    }
}
