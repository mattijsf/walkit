// SPDX-License-Identifier: GPL-3.0-or-later

//! The host of a runtime the page drives: every question goes to the JS side through the imports.

use std::rc::Rc;

use walkit::draw::FontDef;
use walkit::host::{SkinHost, SkinMenu, WindowId};

use crate::imports;

/// A host made of the `host_*` imports.
pub struct JsHost;

fn font_name(font: Option<&FontDef>) -> String {
    font.map(|f| format!("{}|{}", f.id, f.file))
        .unwrap_or_default()
}

/// The host decodes a picture; see `host_decode`.
#[cfg(feature = "trace")]
pub fn decode(file: &str, bytes: &[u8]) -> Option<walkit_trace::Picture> {
    imports::decode(file, bytes)
}

impl SkinHost for JsHost {
    fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        imports::image_size(file)
    }

    fn pixel(&mut self, file: &str, x: i32, y: i32) -> Option<u32> {
        imports::pixels(file, x, y, 1, 1).and_then(|v| v.first().copied())
    }

    fn pixels(&mut self, file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
        imports::pixels(file, x, y, w, h)
    }

    fn text_width(&mut self, text: &str, font: Option<&FontDef>, size: i32, bold: bool) -> i32 {
        imports::text_width(text, &font_name(font), size, bold)
    }

    fn line_metrics(
        &mut self,
        font: Option<&FontDef>,
        size: i32,
        bold: bool,
    ) -> Option<(f32, f32)> {
        imports::line_metrics(&font_name(font), size, bold)
    }

    fn font_file(&mut self, file: &str) -> Option<Rc<[u8]>> {
        imports::font_file(file).map(Rc::from)
    }

    fn log(&mut self, message: &str) {
        imports::log(message);
    }

    fn invalidate(&mut self) {
        imports::invalidate();
    }

    fn window_open(&mut self, kind: &str) -> Option<bool> {
        imports::window_open(kind)
    }

    fn choose_from_menu(&mut self, menu: &SkinMenu) -> Option<i32> {
        imports::choose_from_menu(&walkit::listing::menu(menu))
    }

    fn windows_changed(&mut self, windows: &[WindowId]) {
        let ids: Vec<String> = windows.iter().map(|w| w.0.to_string()).collect();
        imports::windows_changed(&ids.join(","));
    }

    // the page's import takes the action and its param; it places its menus itself
    fn host_action(
        &mut self,
        action: &str,
        param: Option<&str>,
        _from: Option<walkit::host::ActionFrom>,
    ) {
        imports::host_action(action, param.unwrap_or(""));
    }
}
