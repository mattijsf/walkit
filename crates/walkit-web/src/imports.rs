// SPDX-License-Identifier: GPL-3.0-or-later

//! The functions the host provides (`host_*`), and memory the host and the module share. This
//! is the one module that calls into the host, which Rust can only do with `unsafe`.

#[link(wasm_import_module = "host")]
extern "C" {
    fn host_perf_now() -> f64;
    fn host_decode(
        file: *const u8,
        file_len: usize,
        bytes: *const u8,
        len: usize,
        out: *mut u32,
    ) -> i32;
    fn host_text_width(
        text: *const u8,
        len: usize,
        font: *const u8,
        font_len: usize,
        size: i32,
        bold: i32,
    ) -> i32;
    fn host_line_metrics(
        font: *const u8,
        font_len: usize,
        size: i32,
        bold: i32,
        out: *mut f32,
    ) -> i32;
    fn host_image_size(file: *const u8, len: usize, out: *mut i32) -> i32;
    fn host_pixels(
        file: *const u8,
        len: usize,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        out: *mut u32,
    ) -> i32;
    fn host_font_file(file: *const u8, len: usize, out: *mut u32) -> i32;
    fn host_log(text: *const u8, len: usize);
    fn host_invalidate();
    fn host_window_open(kind: *const u8, len: usize) -> i32;
    fn host_choose_from_menu(text: *const u8, len: usize) -> i32;
    fn host_windows_changed(text: *const u8, len: usize);
    fn host_host_action(action: *const u8, action_len: usize, param: *const u8, param_len: usize);
}

/// Memory for the host: `len` bytes, uninitialized, which the host fills.
pub fn alloc(len: usize) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Gives back memory from [`alloc`] (or from a `Vec` the module handed out) of `len` bytes.
pub fn free(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: the pointer and length come from `alloc` with this length, unchanged by the host.
    unsafe {
        drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
    }
}

/// The `len` bytes at `ptr` of the module's memory, copied; `None` for a null pointer.
pub fn read_bytes(ptr: *const u8, len: usize) -> Option<Vec<u8>> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the host wrote `len` bytes at a pointer it got from `alloc`, or passes a slice
    // the module handed out; both lie inside the module's memory.
    Some(unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec())
}

/// Takes a buffer of `len` 32-bit words the host allocated with `wk_alloc` for `len * 4` bytes.
fn take_words(ptr: u32, len: usize) -> Vec<u32> {
    if ptr == 0 || len == 0 {
        return Vec::new();
    }
    let bytes = read_bytes(ptr as *const u8, len * 4).unwrap_or_default();
    free(ptr as *mut u8, len * 4);
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The host's clock, in ms.
pub fn perf_now() -> f64 {
    // SAFETY: an import with no pointer.
    unsafe { host_perf_now() }
}

/// The host decodes `bytes` (the file `file`): its size and pixels as ARGB, or `None`.
pub fn decode(file: &str, bytes: &[u8]) -> Option<walkit_trace::Picture> {
    let mut out = [0u32; 3];
    // SAFETY: the slices live for the call; `out` holds the three words the host writes.
    let ok = unsafe {
        host_decode(
            file.as_ptr(),
            file.len(),
            bytes.as_ptr(),
            bytes.len(),
            out.as_mut_ptr(),
        )
    };
    if ok == 0 {
        return None;
    }
    let (w, h) = (out[0] as i32, out[1] as i32);
    let argb = take_words(out[2], (w.max(0) as usize) * (h.max(0) as usize));
    if argb.len() != (w.max(0) as usize) * (h.max(0) as usize) {
        return None;
    }
    Some(walkit_trace::Picture { w, h, argb })
}

/// The host measures `text`.
pub fn text_width(text: &str, font: &str, size: i32, bold: bool) -> i32 {
    // SAFETY: the slices live for the call.
    unsafe {
        host_text_width(
            text.as_ptr(),
            text.len(),
            font.as_ptr(),
            font.len(),
            size,
            bold as i32,
        )
    }
}

/// The host's line metrics for a font, or `None`.
pub fn line_metrics(font: &str, size: i32, bold: bool) -> Option<(f32, f32)> {
    let mut out = [0f32; 2];
    // SAFETY: `out` holds the two floats the host writes.
    let ok = unsafe {
        host_line_metrics(
            font.as_ptr(),
            font.len(),
            size,
            bold as i32,
            out.as_mut_ptr(),
        )
    };
    if ok != 0 {
        Some((out[0], out[1]))
    } else {
        None
    }
}

/// The size of a skin picture, or `None`.
pub fn image_size(file: &str) -> Option<(i32, i32)> {
    let mut out = [0i32; 2];
    // SAFETY: `out` holds the two ints the host writes.
    let ok = unsafe { host_image_size(file.as_ptr(), file.len(), out.as_mut_ptr()) };
    if ok != 0 {
        Some((out[0], out[1]))
    } else {
        None
    }
}

/// A block of a skin picture's pixels, or `None` when the host cannot read it.
pub fn pixels(file: &str, x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u32>> {
    if w <= 0 || h <= 0 {
        return Some(Vec::new());
    }
    let n = (w as usize) * (h as usize);
    let mut out = vec![0u32; n];
    // SAFETY: `out` holds `n` words the host writes.
    let ok = unsafe { host_pixels(file.as_ptr(), file.len(), x, y, w, h, out.as_mut_ptr()) };
    if ok != 0 {
        Some(out)
    } else {
        None
    }
}

/// The bytes of a stand-in font, or `None`.
pub fn font_file(file: &str) -> Option<Vec<u8>> {
    let mut out = [0u32; 2];
    // SAFETY: `out` holds the pointer and length the host writes.
    let ok = unsafe { host_font_file(file.as_ptr(), file.len(), out.as_mut_ptr()) };
    if ok == 0 || out[0] == 0 {
        return None;
    }
    let bytes = read_bytes(out[0] as *const u8, out[1] as usize);
    free(out[0] as *mut u8, out[1] as usize);
    bytes
}

/// A line the skin logs.
pub fn log(text: &str) {
    // SAFETY: the slice lives for the call.
    unsafe { host_log(text.as_ptr(), text.len()) }
}

/// The engine asks for a redraw.
pub fn invalidate() {
    // SAFETY: an import with no pointer.
    unsafe { host_invalidate() }
}

/// Whether one of the app's windows is open: `None` when the host leaves it to the skin.
pub fn window_open(kind: &str) -> Option<bool> {
    // SAFETY: the slice lives for the call.
    match unsafe { host_window_open(kind.as_ptr(), kind.len()) } {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// The host shows a menu (as the trace writes it) and answers the command picked, 0 for none, -1 for no answer.
pub fn choose_from_menu(text: &str) -> Option<i32> {
    // SAFETY: the slice lives for the call.
    let r = unsafe { host_choose_from_menu(text.as_ptr(), text.len()) };
    if r < 0 {
        None
    } else {
        Some(r)
    }
}

/// The windows on screen changed, as a comma-separated list of ids.
pub fn windows_changed(text: &str) {
    // SAFETY: the slice lives for the call.
    unsafe { host_windows_changed(text.as_ptr(), text.len()) }
}

/// A button action the host answers.
pub fn host_action(action: &str, param: &str) {
    // SAFETY: the slices live for the call.
    unsafe { host_host_action(action.as_ptr(), action.len(), param.as_ptr(), param.len()) }
}
