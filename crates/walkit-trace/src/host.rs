// SPDX-License-Identifier: GPL-3.0-or-later

//! The host a trace is recorded with: image sizes and pixels from the skin's files, text
//! measured by a fixed rule so no platform's fonts enter the trace, and every call the engine
//! makes written into the trace. The playlist window counts as open and the library as closed,
//! as the reference Winamp had them; a menu is dismissed at once.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use walkit::draw::FontDef;
use walkit::files::SkinFiles;
use walkit::host::{MenuItem, SkinHost, SkinMenu, WindowId};

/// A decoded picture: its size and its pixels as ARGB, row by row.
struct Picture {
    w: i32,
    h: i32,
    argb: Vec<u32>,
}

/// The trace host; see the module.
pub struct TraceHost {
    files: Rc<dyn SkinFiles>,
    out: Rc<RefCell<String>>,
    fonts: Option<PathBuf>,
    /// How often the engine asked for a redraw.
    pub invalidates: Rc<RefCell<i64>>,
    images: HashMap<String, Option<Rc<Picture>>>,
    font_files: HashMap<String, Option<Rc<[u8]>>>,
}

impl TraceHost {
    /// A host reading `files`, writing into `out`, with stand-in fonts in `fonts`.
    pub fn new(files: Rc<dyn SkinFiles>, out: Rc<RefCell<String>>, fonts: Option<PathBuf>) -> Self {
        TraceHost {
            files,
            out,
            fonts,
            invalidates: Rc::new(RefCell::new(0)),
            images: HashMap::new(),
            font_files: HashMap::new(),
        }
    }

    fn image(&mut self, file: &str) -> Option<Rc<Picture>> {
        let key = file.to_lowercase();
        if let Some(p) = self.images.get(&key) {
            return p.clone();
        }
        let decoded = self.files.read(file).and_then(|bytes| {
            let img = image::load_from_memory(&bytes).ok()?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let argb = rgba
                .pixels()
                .map(|p| {
                    ((p[3] as u32) << 24)
                        | ((p[0] as u32) << 16)
                        | ((p[1] as u32) << 8)
                        | p[2] as u32
                })
                .collect();
            Some(Rc::new(Picture {
                w: w as i32,
                h: h as i32,
                argb,
            }))
        });
        self.images.insert(key, decoded.clone());
        decoded
    }

    fn write(&self, line: &str) {
        let mut out = self.out.borrow_mut();
        out.push_str(line);
        out.push('\n');
    }
}

fn items(menu: &SkinMenu) -> String {
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
            MenuItem::Submenu { text, menu } => format!("{text}>{}", items(menu)),
        })
        .collect();
    format!("[{}]", parts.join(" "))
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

    /// Six tenths of the size per character, a tenth more in bold; a fixed rule, no font.
    fn text_width(&mut self, text: &str, _font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        let len = text.encode_utf16().count() as i32;
        len.wrapping_mul(size)
            .wrapping_mul(if bold { 7 } else { 6 })
            / 10
    }

    /// Eight tenths of the size above the baseline, two below.
    fn line_metrics(
        &mut self,
        _font: Option<&FontDef>,
        size: i32,
        _bold: bool,
    ) -> Option<(f32, f32)> {
        Some((size as f32 * 0.8, size as f32 * 0.2))
    }

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        if let Some(f) = self.font_files.get(file) {
            return f.clone();
        }
        let bytes: Option<Rc<[u8]>> = self
            .fonts
            .as_ref()
            .and_then(|dir| {
                std::fs::read(dir.join(file.strip_prefix("fonts/").unwrap_or(file))).ok()
            })
            .map(Rc::from);
        self.font_files.insert(file.to_string(), bytes.clone());
        bytes
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
        self.write(&format!("menu {}", items(menu)));
        Some(0)
    }

    fn windows_changed(&mut self, windows: &[WindowId]) {
        let ids: Vec<String> = windows.iter().map(|w| w.0.to_string()).collect();
        self.write(&format!("windows {}", ids.join(",")));
    }

    fn host_action(&mut self, action: &str, param: Option<&str>) {
        self.write(&format!("hostaction {action} {}", param.unwrap_or("-")));
    }
}
