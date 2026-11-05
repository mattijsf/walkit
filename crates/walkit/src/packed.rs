// SPDX-License-Identifier: GPL-3.0-or-later

//! The draw list of a frame as one packed buffer, for a host across a language boundary that
//! takes a frame in one call and decodes it into draw commands on its side (the Android
//! binding). Little-endian throughout; strings are interned in a table at the front, so a file
//! name drawn forty times crosses once. A window's commands cross only where they changed:
//! the writer keeps what it sent of each window, and the host keeps what it decoded, so a
//! frame says for each window which stretches of the frame before to keep and which commands
//! are new ([`FrameWriter`]).
//!
//! Layout (`u32` unless said):
//! - `"WKDL"`, version `3`
//! - the string table: count, then each string as its byte length and UTF-8 bytes
//! - the windows: count, then each as id (`i32`), container (string index), layout (string
//!   index), rect, its plan, and its commands
//!
//! A plan is the window's id and version (each `u64`), then its runs: count, then each as id
//! and version (`u64`), from, to, bounds (rect). The commands start with a byte: 0 for the
//! same commands as the frame before, 1 for segments, which follow as a count and then each as
//! a byte 0 (keep: the next count commands of the frame before) or 1 (new: count, then that
//! many commands).
//!
//! A rect is four `i32`: left, top, width, height. A command starts with a tag byte:
//! 1 image (bitmap, rect, alpha `i32`, gamma, smooth `u8`), 2 fill (rect, rgb `i32`, alpha
//! `i32`), 3 window start (region), 4 window end, 5 text (text, font, size `i32`, rgb `i32`,
//! rect, alpha `i32`, bold `u8`, shadow, x `f32`, baseline `f32`), 6 bitmap text (text, rect),
//! 7 clip (rect), 8 unclip, 9 pixels (rect, w `i32`, h `i32`, alpha `i32`, stable `u8`, then
//! w x h ARGB `i32`).
//!
//! A bitmap is id, file (string indexes), x, y, w, h (`i32`, -1 for a size the file decides),
//! gamma group (string index, or `0xffffffff` for none). A gamma is a `u8` 0 for none or 1
//! followed by r, g, b, boost, gray (`i32`). A font is a `u8` 0 for none or 1 followed by id
//! and file (string indexes). A shadow is a `u8` 0 for none or 1 followed by rgb, dx, dy
//! (`i32`). A region starts with a kind byte: 1 picture (bitmap, rect, alpha, gamma, smooth,
//! as an image command), 2 box (rect), 3 op (`u8` 0 or 1 and the base region, then the add,
//! keep and cut lists, each a count and that many regions).

use std::collections::HashMap;

use crate::draw::{BitmapDef, DrawCmd, Gamma, Rect, Region, RegionImage};
use crate::host::{MenuItem, SkinMenu, WindowId};
use crate::plan::WindowPlan;

/// The version written after the magic.
pub const VERSION: u32 = 3;

/// One window of a frame as the writer takes it: its id, its container and layout ids, where it
/// sits in the stack, its commands and its plan.
pub struct FrameWindow<'a> {
    /// The window.
    pub id: WindowId,
    /// The container's id in the skin's XML.
    pub container: &'a str,
    /// The layout's id in the skin's XML.
    pub layout: &'a str,
    /// Where the window sits in the stack.
    pub rect: Rect,
    /// Its drawing.
    pub cmds: &'a [DrawCmd],
    /// What a renderer may keep of it.
    pub plan: &'a WindowPlan,
}

/// Writes frames for one host, keeping what it sent of each window so the next frame carries
/// only the commands that changed; see the module.
#[derive(Default)]
pub struct FrameWriter {
    sent: HashMap<WindowId, Vec<DrawCmd>>,
}

impl FrameWriter {
    /// A writer that has sent nothing.
    pub fn new() -> FrameWriter {
        FrameWriter::default()
    }

    /// The frame's windows as one buffer. A window not in the frame is forgotten, so its
    /// commands cross whole when it shows again.
    pub fn encode(&mut self, windows: &[FrameWindow]) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(windows.len() as u32);
        let mut sent = HashMap::with_capacity(windows.len());
        for win in windows {
            w.i32(win.id.0 as i32);
            w.string(win.container);
            w.string(win.layout);
            w.rect(&win.rect);
            w.plan(win.plan);
            let before = self.sent.remove(&win.id);
            w.commands(win.cmds, before.as_deref());
            sent.insert(win.id, win.cmds.to_vec());
        }
        self.sent = sent;
        w.finish()
    }
}

/// A menu as one buffer, for a host that passes one back to the engine: item count, then each
/// item as a kind byte (1 command: id `i32`, text, checked `u8`, disabled `u8`; 2 separator;
/// 3 submenu: text, then the menu), with the strings in the table at the front as a frame has.
pub fn encode_menu(menu: &SkinMenu) -> Vec<u8> {
    let mut w = Writer::default();
    w.menu(menu);
    w.finish()
}

/// A menu read back from what [`encode_menu`] wrote, or `None` when the bytes are not one.
pub fn decode_menu(bytes: &[u8]) -> Option<SkinMenu> {
    let mut r = Reader::new(bytes)?;
    r.menu(0)
}

#[derive(Default)]
struct Writer {
    strings: Vec<String>,
    index: HashMap<String, u32>,
    body: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, v: u8) {
        self.body.push(v);
    }

    fn u32(&mut self, v: u32) {
        self.body.extend_from_slice(&v.to_le_bytes());
    }

    fn i32(&mut self, v: i32) {
        self.body.extend_from_slice(&v.to_le_bytes());
    }

    fn f32(&mut self, v: f32) {
        self.body.extend_from_slice(&v.to_le_bytes());
    }

    fn string(&mut self, s: &str) {
        let i = match self.index.get(s) {
            Some(i) => *i,
            None => {
                let i = self.strings.len() as u32;
                self.strings.push(s.to_string());
                self.index.insert(s.to_string(), i);
                i
            }
        };
        self.u32(i);
    }

    fn rect(&mut self, r: &Rect) {
        self.i32(r.left);
        self.i32(r.top);
        self.i32(r.width);
        self.i32(r.height);
    }

    fn bitmap(&mut self, b: &BitmapDef) {
        self.string(&b.id);
        self.string(&b.file);
        self.i32(b.x);
        self.i32(b.y);
        self.i32(b.w.unwrap_or(-1));
        self.i32(b.h.unwrap_or(-1));
        match &b.gamma_group {
            Some(g) => self.string(g),
            None => self.u32(u32::MAX),
        }
    }

    fn gamma(&mut self, g: Option<&Gamma>) {
        match g {
            None => self.u8(0),
            Some(g) => {
                self.u8(1);
                self.i32(g.r);
                self.i32(g.g);
                self.i32(g.b);
                self.i32(g.boost);
                self.i32(g.gray);
            }
        }
    }

    fn image(
        &mut self,
        bitmap: &BitmapDef,
        dst: &Rect,
        alpha: i32,
        gamma: Option<&Gamma>,
        smooth: bool,
    ) {
        self.bitmap(bitmap);
        self.rect(dst);
        self.i32(alpha);
        self.gamma(gamma);
        self.u8(smooth as u8);
    }

    fn region_image(&mut self, img: &RegionImage) {
        self.image(
            &img.bitmap,
            &img.dst,
            img.alpha,
            img.gamma.as_ref(),
            img.smooth,
        );
    }

    fn region(&mut self, r: &Region) {
        match r {
            Region::Pic(img) => {
                self.u8(1);
                self.region_image(img);
            }
            Region::Box(b) => {
                self.u8(2);
                self.rect(b);
            }
            Region::Op {
                base,
                add,
                keep,
                cut,
            } => {
                self.u8(3);
                match base {
                    None => self.u8(0),
                    Some(b) => {
                        self.u8(1);
                        self.region(b);
                    }
                }
                for list in [add, keep, cut] {
                    self.u32(list.len() as u32);
                    for r in list {
                        self.region(r);
                    }
                }
            }
        }
    }

    fn cmd(&mut self, cmd: &DrawCmd) {
        match cmd {
            DrawCmd::Image {
                bitmap,
                dst,
                alpha,
                gamma,
                smooth,
            } => {
                self.u8(1);
                self.image(bitmap, dst, *alpha, gamma.as_ref(), *smooth);
            }
            DrawCmd::Fill { rect, color, alpha } => {
                self.u8(2);
                self.rect(rect);
                self.i32(color.0 as i32);
                self.i32(*alpha);
            }
            DrawCmd::WindowStart(r) => {
                self.u8(3);
                self.region(r);
            }
            DrawCmd::WindowEnd => self.u8(4),
            DrawCmd::Text(t) => {
                self.u8(5);
                self.string(&t.text);
                match &t.font {
                    None => self.u8(0),
                    Some(f) => {
                        self.u8(1);
                        self.string(&f.id);
                        self.string(&f.file);
                    }
                }
                self.i32(t.size);
                self.i32(t.color.0 as i32);
                self.rect(&t.rect);
                self.i32(t.alpha);
                self.u8(t.bold as u8);
                match t.shadow {
                    None => self.u8(0),
                    Some(s) => {
                        self.u8(1);
                        self.i32(s.0 as i32);
                        self.i32(t.shadow_dx);
                        self.i32(t.shadow_dy);
                    }
                }
                self.f32(t.x);
                self.f32(t.baseline);
            }
            DrawCmd::BitmapText(b) => {
                self.u8(6);
                self.string(&b.text);
                self.rect(&b.rect);
            }
            DrawCmd::Clip(r) => {
                self.u8(7);
                self.rect(r);
            }
            DrawCmd::Unclip => self.u8(8),
            DrawCmd::Pixels {
                rect,
                w,
                h,
                argb,
                alpha,
                stable,
            } => {
                self.u8(9);
                self.rect(rect);
                self.i32(*w);
                self.i32(*h);
                self.i32(*alpha);
                self.u8(*stable as u8);
                for p in argb.iter() {
                    self.u32(*p);
                }
            }
        }
    }

    fn u64(&mut self, v: u64) {
        self.body.extend_from_slice(&v.to_le_bytes());
    }

    fn plan(&mut self, plan: &WindowPlan) {
        self.u64(plan.id);
        self.u64(plan.version);
        self.u32(plan.runs.len() as u32);
        for run in &plan.runs {
            self.u64(run.id);
            self.u64(run.version);
            self.u32(run.from as u32);
            self.u32(run.to as u32);
            self.rect(&run.bounds);
        }
    }

    /// `cmds` against `before`, the window's commands as the host has them: the same, or the
    /// stretches kept and the stretches new. A window of another length crosses whole.
    fn commands(&mut self, cmds: &[DrawCmd], before: Option<&[DrawCmd]>) {
        let Some(before) = before.filter(|b| b.len() == cmds.len()) else {
            self.u8(1);
            self.u32(1);
            self.new_segment(cmds);
            return;
        };
        let same: Vec<bool> = cmds
            .iter()
            .zip(before)
            .map(|(c, b)| c.draws_same_as(b))
            .collect();
        if same.iter().all(|s| *s) {
            self.u8(0);
            return;
        }
        let mut segments = Vec::new();
        let mut i = 0;
        while i < cmds.len() {
            let mut j = i;
            while j < cmds.len() && same[j] == same[i] {
                j += 1;
            }
            segments.push((same[i], i, j));
            i = j;
        }
        self.u8(1);
        self.u32(segments.len() as u32);
        for (keep, from, to) in segments {
            if keep {
                self.u8(0);
                self.u32((to - from) as u32);
            } else {
                self.new_segment(&cmds[from..to]);
            }
        }
    }

    fn new_segment(&mut self, cmds: &[DrawCmd]) {
        self.u8(1);
        self.u32(cmds.len() as u32);
        for cmd in cmds {
            self.cmd(cmd);
        }
    }

    fn menu(&mut self, menu: &SkinMenu) {
        self.u32(menu.items.len() as u32);
        for item in &menu.items {
            match item {
                MenuItem::Command {
                    id,
                    text,
                    checked,
                    disabled,
                } => {
                    self.u8(1);
                    self.i32(*id);
                    self.string(text);
                    self.u8(*checked as u8);
                    self.u8(*disabled as u8);
                }
                MenuItem::Separator => self.u8(2),
                MenuItem::Submenu { text, menu } => {
                    self.u8(3);
                    self.string(text);
                    self.menu(menu);
                }
            }
        }
    }

    fn finish(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.body.len() + 64);
        out.extend_from_slice(b"WKDL");
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(self.strings.len() as u32).to_le_bytes());
        for s in &self.strings {
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        out.extend_from_slice(&self.body);
        out
    }
}

/// Reads what [`Writer`] wrote; every read past the end gives `None`.
struct Reader<'a> {
    strings: Vec<&'a str>,
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Option<Self> {
        let mut r = Reader {
            strings: Vec::new(),
            bytes,
            at: 0,
        };
        if r.take(4)? != b"WKDL" || r.u32()? != VERSION {
            return None;
        }
        let n = r.u32()?;
        for _ in 0..n {
            let len = r.u32()? as usize;
            let s = std::str::from_utf8(r.take(len)?).ok()?;
            r.strings.push(s);
        }
        Some(r)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self) -> Option<i32> {
        self.u32().map(|v| v as i32)
    }

    fn string(&mut self) -> Option<&'a str> {
        let i = self.u32()? as usize;
        self.strings.get(i).copied()
    }

    /// A menu `depth` levels down; a menu nested past [`MAX_MENU_DEPTH`] is not read.
    fn menu(&mut self, depth: u32) -> Option<SkinMenu> {
        if depth > MAX_MENU_DEPTH {
            return None;
        }
        let n = self.u32()?;
        if n as usize > MAX_MENU_ITEMS {
            return None;
        }
        let mut items = Vec::with_capacity(n as usize);
        for _ in 0..n {
            items.push(match self.u8()? {
                1 => {
                    let id = self.i32()?;
                    let text = self.string()?.to_string();
                    let checked = self.u8()? != 0;
                    let disabled = self.u8()? != 0;
                    MenuItem::Command {
                        id,
                        text,
                        checked,
                        disabled,
                    }
                }
                2 => MenuItem::Separator,
                3 => {
                    let text = self.string()?.to_string();
                    let menu = self.menu(depth + 1)?;
                    MenuItem::Submenu { text, menu }
                }
                _ => return None,
            });
        }
        Some(SkinMenu { items })
    }
}

/// How deep a menu read back may nest.
const MAX_MENU_DEPTH: u32 = 16;
/// How many items one level of a menu read back may hold.
const MAX_MENU_ITEMS: usize = 4096;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Align, FontDef, Rgb, TextCmd, VAlign};
    use crate::plan::LiveRun;

    #[test]
    fn a_frame_packs_with_its_strings_once() {
        let bitmap = BitmapDef::new("main.bg", "main.png", 0, 0, Some(10), Some(5), None);
        let cmds = vec![
            DrawCmd::WindowStart(Region::Op {
                base: Some(Box::new(Region::Box(Rect::new(0, 0, 10, 5)))),
                add: vec![],
                keep: vec![],
                cut: vec![Region::Pic(RegionImage {
                    bitmap: bitmap.clone(),
                    dst: Rect::new(0, 0, 10, 5),
                    alpha: 255,
                    gamma: None,
                    smooth: false,
                })],
            }),
            DrawCmd::image(bitmap.clone(), Rect::new(1, 2, 10, 5), 255),
            DrawCmd::image(bitmap, Rect::new(3, 4, 10, 5), 128),
            DrawCmd::Fill {
                rect: Rect::new(0, 0, 1, 1),
                color: Rgb(0xff00ff),
                alpha: 200,
            },
            DrawCmd::Text(Box::new(TextCmd {
                text: "Andamp".into(),
                font: Some(FontDef::new("f", "font.ttf")),
                size: 12,
                color: Rgb(0xffffff),
                rect: Rect::new(0, 0, 50, 20),
                align: Align::Left,
                scroll: 0,
                alpha: 255,
                valign: VAlign::Top,
                bold: true,
                shadow: Some(Rgb(0)),
                shadow_dx: 1,
                shadow_dy: 1,
                x: 2.5,
                baseline: 10.25,
            })),
            DrawCmd::Clip(Rect::new(0, 0, 2, 2)),
            DrawCmd::Pixels {
                rect: Rect::new(0, 0, 2, 1),
                w: 2,
                h: 1,
                argb: Rc::from(vec![0xff112233u32, 0x80445566].into_boxed_slice()),
                alpha: 255,
                stable: true,
            },
            DrawCmd::Unclip,
            DrawCmd::WindowEnd,
        ];
        // the layout (object 3) drew the shape and the background; a text object (12) the
        // text, the clip and the pixels; the window's ends are nobody's
        let plan = WindowPlan {
            id: 5,
            version: 6,
            runs: vec![LiveRun {
                id: 8,
                version: 9,
                from: 4,
                to: 7,
                bounds: Rect::new(0, 0, 50, 20),
            }],
        };
        fn window<'a>(cmds: &'a [DrawCmd], plan: &'a WindowPlan) -> FrameWindow<'a> {
            FrameWindow {
                id: WindowId(7),
                container: "main",
                layout: "normal",
                rect: Rect::new(10, 20, 300, 100),
                cmds,
                plan,
            }
        }
        let mut writer = FrameWriter::new();
        let bytes = writer.encode(&[window(&cmds, &plan)]);
        assert_eq!(&bytes[..4], b"WKDL");
        let strings = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        // main, normal, main.bg, main.png, Andamp, f, font.ttf: each once
        assert_eq!(strings, 7);
        assert_eq!(bytes.iter().filter(|b| **b == b'W').count(), 1);
        // the Kotlin side's PackedTest decodes this same buffer
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        println!("FRAME_HEX {hex}");
        assert_eq!(hex, FRAME_HEX);

        // the next frame: the fill's color changed, the plan's version with it; the rest is kept
        let mut next = cmds.clone();
        next[3] = DrawCmd::Fill {
            rect: Rect::new(0, 0, 1, 1),
            color: Rgb(0x00ff00),
            alpha: 200,
        };
        let moved = WindowPlan {
            version: 10,
            ..plan.clone()
        };
        let bytes = writer.encode(&[window(&next, &moved)]);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        println!("NEXT_HEX {hex}");
        assert_eq!(hex, NEXT_HEX);
        assert!(
            bytes.len() < 200,
            "three segments, one command: {} bytes",
            bytes.len()
        );

        // the same again: nothing crosses but the plan
        let bytes = writer.encode(&[window(&next, &moved)]);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        println!("SAME_HEX {hex}");
        assert_eq!(hex, SAME_HEX);
    }

    /// The buffers above, as the Kotlin side's test holds them: a change here is a change of the format.
    const FRAME_HEX: &str = "574b444c0300000007000000040000006d61696e060000006e6f726d616c070000006d61696e2e6267080000006d61696e2e706e6706000000416e64616d70010000006608000000666f6e742e747466010000000700000000000000010000000a000000140000002c01000064000000050000000000000006000000000000000100000008000000000000000900000000000000040000000700000000000000000000003200000014000000010100000001090000000303010200000000000000000a0000000500000000000000000000000100000001020000000300000000000000000000000a00000005000000ffffffff00000000000000000a00000005000000ff000000000001020000000300000000000000000000000a00000005000000ffffffff01000000020000000a00000005000000ff000000000001020000000300000000000000000000000a00000005000000ffffffff03000000040000000a000000050000008000000000000200000000000000000100000001000000ff00ff00c800000005040000000105000000060000000c000000ffffff0000000000000000003200000014000000ff00000001010000000001000000010000000000204000002441070000000000000000020000000200000009000000000000000002000000010000000200000001000000ff00000001332211ff665544800804";
    const NEXT_HEX: &str = "574b444c0300000002000000040000006d61696e060000006e6f726d616c010000000700000000000000010000000a000000140000002c0100006400000005000000000000000a000000000000000100000008000000000000000900000000000000040000000700000000000000000000003200000014000000010300000000030000000101000000020000000000000000010000000100000000ff0000c80000000005000000";
    const SAME_HEX: &str = "574b444c0300000002000000040000006d61696e060000006e6f726d616c010000000700000000000000010000000a000000140000002c0100006400000005000000000000000a00000000000000010000000800000000000000090000000000000004000000070000000000000000000000320000001400000000";

    #[test]
    fn a_menu_reads_back_as_it_was() {
        let menu = SkinMenu {
            items: vec![
                MenuItem::Command {
                    id: 1,
                    text: "Open".into(),
                    checked: false,
                    disabled: false,
                },
                MenuItem::Separator,
                MenuItem::Submenu {
                    text: "More".into(),
                    menu: SkinMenu {
                        items: vec![MenuItem::Command {
                            id: 2,
                            text: "Ticked".into(),
                            checked: true,
                            disabled: true,
                        }],
                    },
                },
            ],
        };
        assert_eq!(decode_menu(&encode_menu(&menu)), Some(menu));
        assert_eq!(decode_menu(b"nope"), None);
    }

    use std::rc::Rc;
}
