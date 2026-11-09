// SPDX-License-Identifier: GPL-3.0-or-later
//! A classic skin's cursors: which of its 29 cursor files Winamp shows over each part of each
//! window, and the files themselves decoded. Winamp 5.666 loads each as Windows loads a cursor
//! file, so a `.cur` name may hold an animated cursor, and picks one per part of the main window,
//! the equalizer, the playlist, the video window and the gen windows.

use super::ClassicWindow;

/// What a skin's cursor stands for, and what the host shows when the skin has no file for it
/// (Winamp then shows an arrow with a small sign).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
    /// The plain arrow.
    Arrow,
    /// Moves the window (four arrows).
    Move,
    /// Drags a slider sideways.
    HorizontalSlide,
    /// Drags a slider up and down.
    VerticalSlide,
    /// Over a close button.
    Close,
    /// Sizes the window.
    Resize,
}

/// The cursor files a classic skin holds, one entry per place a classic window shows one, each
/// with what it stands for.
pub const CURSOR_FILES: [(&str, CursorKind); 29] = {
    use CursorKind::{Arrow, Close, HorizontalSlide, Move, Resize, VerticalSlide};
    [
        ("volbal.cur", HorizontalSlide),
        ("posbar.cur", HorizontalSlide),
        ("winbut.cur", Arrow),
        ("min.cur", Arrow),
        ("close.cur", Close),
        ("mainmenu.cur", Arrow),
        ("titlebar.cur", Move),
        ("songname.cur", HorizontalSlide),
        ("normal.cur", Arrow),
        ("winbut.cur", Arrow),
        ("min.cur", Arrow),
        ("wsposbar.cur", HorizontalSlide),
        ("close.cur", Close),
        ("mmenu.cur", Arrow),
        ("wsnormal.cur", Arrow),
        ("pwinbut.cur", Arrow),
        ("pclose.cur", Close),
        ("ptbar.cur", Move),
        ("pvscroll.cur", VerticalSlide),
        ("psize.cur", Resize),
        ("pnormal.cur", Arrow),
        ("pwinbut.cur", Arrow),
        ("pclose.cur", Close),
        ("pwssize.cur", HorizontalSlide),
        ("pwsnorm.cur", Arrow),
        ("eqslid.cur", VerticalSlide),
        ("eqclose.cur", Close),
        ("eqtitle.cur", Move),
        ("eqnormal.cur", Arrow),
    ]
};

/// A box a cursor covers: left, top, right, bottom, inclusive; a negative value counts
/// from the window's right or bottom edge.
type Box4 = (i32, i32, i32, i32);

fn within(b: Box4, w: i32, h: i32, x: i32, y: i32) -> bool {
    let fix = |v: i32, size: i32| if v < 0 { v + size } else { v };
    let (l, t, r, bt) = (fix(b.0, w), fix(b.1, h), fix(b.2, w), fix(b.3, h));
    x >= l && x <= r && y >= t && y <= bt
}

/// The first of `boxes` that holds the point, else `boxes.len()`.
fn first(boxes: &[Box4], w: i32, h: i32, x: i32, y: i32) -> usize {
    boxes
        .iter()
        .position(|b| within(*b, w, h, x, y))
        .unwrap_or(boxes.len())
}

/// The cursor slot (an index into [`CURSOR_FILES`]) at `x`, `y` of classic window `win`,
/// `w` x `h`.
pub fn slot_at(win: ClassicWindow, w: i32, h: i32, x: i32, y: i32) -> Option<usize> {
    Some(match win {
        // volume and balance share one cursor file
        ClassicWindow::Main => first(
            &[
                (107, 58, 215, 66),
                (18, 73, 263, 81),
                (254, 3, 262, 12),
                (244, 3, 253, 12),
                (264, 3, 272, 12),
                (5, 3, 17, 13),
                (0, 0, 275, 13),
                (105, 24, 266, 35),
            ],
            w,
            h,
            x,
            y,
        ),
        ClassicWindow::MainShade => {
            9 + first(
                &[
                    (254, 3, 262, 12),
                    (244, 3, 253, 12),
                    (228, 3, 242, 12),
                    (264, 3, 272, 12),
                    (5, 3, 17, 13),
                ],
                w,
                h,
                x,
                y,
            )
        }
        ClassicWindow::Equalizer | ClassicWindow::EqualizerShade => {
            let slider = (0..11).any(|i| {
                let x0 = if i == 0 { 21 } else { 60 + 18 * i };
                x >= x0 && x <= x0 + 12 && y > 38 && y < 99
            });
            if slider {
                25
            } else {
                26 + first(&[(264, 3, 272, 12), (0, 0, 275, 13)], w, h, x, y)
            }
        }
        ClassicWindow::Playlist => {
            15 + first(
                &[
                    (-21, 3, -13, 12),
                    (-11, 3, -3, 12),
                    (0, 0, -1, 13),
                    (-15, 20, -7, -38),
                    (-20, -20, -1, -1),
                ],
                w,
                h,
                x,
                y,
            )
        }
        ClassicWindow::PlaylistShade => {
            21 + first(
                &[(-21, 3, -13, 12), (-11, 3, -3, 12), (-29, 3, -20, 12)],
                w,
                h,
                x,
                y,
            )
        }
        ClassicWindow::Video => [16, 17, 19, 20][first(
            &[(-11, 3, -3, 12), (0, 0, -1, 13), (-20, -20, -1, -1)],
            w,
            h,
            x,
            y,
        )],
        ClassicWindow::Gen => {
            // the boxes stop a pixel short on the right and bottom (probed in the VM)
            let held = |b: Box4| within((b.0, b.1, b.2 - 1, b.3 - 1), w, h, x, y);
            if held((-11, 3, -3, 12)) {
                16
            } else if held((0, 0, -1, 13)) {
                17
            } else if held((-20, -20, -1, -1)) {
                19
            } else {
                // the frame and what the window holds alike (the VM showed PNormal over the
                // media library's lists)
                20
            }
        }
    })
}

/// One picture of a cursor: ARGB rows, and the point that is the pointer's tip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorFrame {
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// The tip, from the left.
    pub hot_x: u32,
    /// The tip, from the top.
    pub hot_y: u32,
    /// ARGB, row by row from the top, straight alpha.
    pub argb: Vec<u32>,
}

/// A decoded cursor: its pictures and the steps an animated one shows them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinCursor {
    /// The pictures.
    pub frames: Vec<CursorFrame>,
    /// Which picture each step shows and for how many milliseconds; one step without a time
    /// for a still cursor.
    pub steps: Vec<(usize, u32)>,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// A cursor file: a `.cur` (or `.ico`) or an animated `.ani`, whatever its name says, as
/// Windows loads a cursor file. `None` for what neither holds.
pub fn decode(bytes: &[u8]) -> Option<SkinCursor> {
    if bytes.get(0..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"ACON") {
        return decode_ani(bytes);
    }
    let frame = decode_cur(bytes)?;
    Some(SkinCursor {
        frames: vec![frame],
        steps: vec![(0, 0)],
    })
}

/// The first picture of a `.cur` or `.ico`, the one Windows shows when no size is asked for.
fn decode_cur(b: &[u8]) -> Option<CursorFrame> {
    let kind = u16_at(b, 2)?;
    if u16_at(b, 0)? != 0 || !(kind == 1 || kind == 2) || u16_at(b, 4)? == 0 {
        return None;
    }
    let (hx, hy) = (u16_at(b, 10)?, u16_at(b, 12)?);
    let size = u32_at(b, 14)? as usize;
    let at = u32_at(b, 18)? as usize;
    let mut frame = decode_dib(b.get(at..at.checked_add(size)?)?)?;
    if kind == 2 {
        frame.hot_x = u32::from(hx).min(frame.width.saturating_sub(1));
        frame.hot_y = u32::from(hy).min(frame.height.saturating_sub(1));
    } else {
        frame.hot_x = frame.width / 2;
        frame.hot_y = frame.height / 2;
    }
    Some(frame)
}

/// A cursor picture's DIB: the colors (1, 4, 8, 24 or 32 bits) over the AND mask, the two
/// stacked in a bitmap twice as tall. A pixel the mask keeps that has a color is combined with the
/// screen under it by exclusive or on Windows; it shows here as Windows draws it over white (a plain inverting
/// pixel is black). A picture in PNG is not read.
fn decode_dib(d: &[u8]) -> Option<CursorFrame> {
    let header = u32_at(d, 0)? as usize;
    if header < 40 {
        return None;
    }
    let width = i32::from_le_bytes(d.get(4..8)?.try_into().ok()?);
    let height = i32::from_le_bytes(d.get(8..12)?.try_into().ok()?) / 2;
    let bits = u16_at(d, 14)?;
    let compression = u32_at(d, 16)?;
    if width <= 0 || height <= 0 || width > 256 || height > 256 || compression != 0 {
        return None;
    }
    let (w, h) = (width as usize, height as usize);
    let colors = if bits <= 8 {
        match u32_at(d, 32)? {
            0 => 1usize << bits,
            n => n as usize,
        }
    } else {
        0
    };
    let palette: Vec<u32> = (0..colors)
        .map(|i| {
            let p = header + 4 * i;
            d.get(p..p + 3).map_or(0, |c| {
                (u32::from(c[2]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[0])
            })
        })
        .collect();
    let xor_at = header + 4 * colors;
    let xor_row = (w * usize::from(bits)).div_ceil(32) * 4;
    let and_at = xor_at + xor_row * h;
    let and_row = w.div_ceil(32) * 4;
    let mut argb = vec![0u32; w * h];
    let mut alpha_seen = false;
    for y in 0..h {
        let row = xor_at + (h - 1 - y) * xor_row;
        for x in 0..w {
            let px = match bits {
                1 | 4 | 8 => {
                    let bit = x * usize::from(bits);
                    let byte = *d.get(row + bit / 8)?;
                    let shift = 8 - usize::from(bits) - bit % 8;
                    let idx = (usize::from(byte) >> shift) & ((1 << bits) - 1);
                    0xff00_0000 | palette.get(idx).copied().unwrap_or(0)
                }
                24 => {
                    let c = d.get(row + 3 * x..row + 3 * x + 3)?;
                    0xff00_0000 | (u32::from(c[2]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[0])
                }
                32 => {
                    let c = d.get(row + 4 * x..row + 4 * x + 4)?;
                    alpha_seen |= c[3] != 0;
                    (u32::from(c[3]) << 24)
                        | (u32::from(c[2]) << 16)
                        | (u32::from(c[1]) << 8)
                        | u32::from(c[0])
                }
                _ => return None,
            };
            argb[y * w + x] = px;
        }
    }
    if !alpha_seen {
        for y in 0..h {
            let row = and_at + (h - 1 - y) * and_row;
            for x in 0..w {
                let masked = d
                    .get(row + x / 8)
                    .is_some_and(|b| (b >> (7 - x % 8)) & 1 == 1);
                let px = &mut argb[y * w + x];
                *px = match (masked, *px & 0x00ff_ffff) {
                    (true, 0) => 0,
                    (true, c) => 0xff00_0000 | (c ^ 0x00ff_ffff),
                    (false, c) => 0xff00_0000 | c,
                };
            }
        }
    }
    Some(CursorFrame {
        width: w as u32,
        height: h as u32,
        hot_x: 0,
        hot_y: 0,
        argb,
    })
}

/// An animated cursor: `anih` (frames, steps, the time per step in 1/60 s, whether a `seq `
/// orders them), `rate` (a time per step), `seq ` and the `fram` list of `icon` pictures.
fn decode_ani(b: &[u8]) -> Option<SkinCursor> {
    let mut frames = Vec::new();
    let mut steps_n = 0usize;
    let mut jiffies = 0u32;
    let mut sequenced = false;
    let mut rates: Vec<u32> = Vec::new();
    let mut seq: Vec<u32> = Vec::new();
    let mut walk = vec![(12usize, b.len())];
    while let Some((mut i, end)) = walk.pop() {
        while i + 8 <= end {
            let id = b.get(i..i + 4)?;
            let n = u32_at(b, i + 4)? as usize;
            let body = i + 8;
            let stop = body.checked_add(n)?.min(end);
            match id {
                b"LIST" => walk.push((body + 4, stop)),
                b"anih" => {
                    steps_n = u32_at(b, body + 8)? as usize;
                    jiffies = u32_at(b, body + 28)?;
                    sequenced = u32_at(b, body + 32)? & 2 != 0;
                }
                b"rate" => rates = (0..n / 4).filter_map(|k| u32_at(b, body + 4 * k)).collect(),
                b"seq " => seq = (0..n / 4).filter_map(|k| u32_at(b, body + 4 * k)).collect(),
                b"icon" => frames.push(decode_cur(b.get(body..stop)?)?),
                _ => {}
            }
            i = body + n + (n & 1);
        }
    }
    if frames.is_empty() {
        return None;
    }
    let count = if steps_n == 0 { frames.len() } else { steps_n };
    let steps = (0..count)
        .map(|k| {
            let frame = if sequenced {
                seq.get(k).copied().unwrap_or(0) as usize
            } else {
                k
            };
            let jif = rates.get(k).copied().unwrap_or(jiffies).max(1);
            (frame.min(frames.len() - 1), jif * 1000 / 60)
        })
        .collect();
    Some(SkinCursor { frames, steps })
}

impl crate::runtime::SkinRuntime {
    /// The cursor slot (an index into [`CURSOR_FILES`]) Winamp shows at `sx`, `sy` of the
    /// stack, over a classic skin's window; `None` elsewhere.
    pub fn classic_cursor_at(&mut self, sx: i32, sy: i32) -> Option<usize> {
        let o = self.hit_test_stack(sx, sy)?;
        let l = self.layout_object(o)?;
        let win = self.all_objects.iter().copied().find(|&c| {
            matches!(self.objects[c].kind, crate::gui::Kind::ClassicWindow(_))
                && self.layout_object(c) == Some(l)
        })?;
        let crate::gui::Kind::ClassicWindow(kind) = self.objects[win].kind else {
            return None;
        };
        let origin = self.window_rect_of(l);
        let r = self.objects[win].base.rect;
        slot_at(
            kind,
            r.width,
            r.height,
            sx - origin.left - r.left,
            sy - origin.top - r.top,
        )
    }

    /// The skin's picture for cursor `slot`, decoded once; `None` when the skin has no such
    /// file or it cannot be read, and the host shows a cursor for the slot's [`CursorKind`].
    pub fn classic_cursor(&mut self, slot: usize) -> Option<std::rc::Rc<SkinCursor>> {
        let (file, _) = CURSOR_FILES.get(slot)?;
        if let Some(c) = self.classic_cursors.get(file) {
            return c.clone();
        }
        let c = self
            .files
            .read(file)
            .and_then(|b| decode(&b))
            .map(std::rc::Rc::new);
        self.classic_cursors.insert(file, c.clone());
        c
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, slot_at, CURSOR_FILES};
    use crate::classic::ClassicWindow;

    /// A 2 x 2 cursor, 24 bits: red, green / blue, white; the mask keeps the white one out.
    fn cur(hot: (u16, u16)) -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend(40u32.to_le_bytes());
        dib.extend(2i32.to_le_bytes());
        dib.extend(4i32.to_le_bytes());
        dib.extend(1u16.to_le_bytes());
        dib.extend(24u16.to_le_bytes());
        dib.extend([0u8; 24]);
        // rows bottom up, each 8 bytes (6 used)
        dib.extend([0xff, 0, 0, 0xff, 0xff, 0xff, 0, 0]); // blue, white
        dib.extend([0, 0, 0xff, 0, 0xff, 0, 0, 0]); // red, green
        dib.extend([0x40, 0, 0, 0]); // mask bottom row: white out
        dib.extend([0, 0, 0, 0]);
        let mut f = vec![0, 0, 2, 0, 1, 0, 2, 2, 0, 0];
        f.extend(hot.0.to_le_bytes());
        f.extend(hot.1.to_le_bytes());
        f.extend((dib.len() as u32).to_le_bytes());
        f.extend(22u32.to_le_bytes());
        f.extend(dib);
        f
    }

    #[test]
    fn a_cur_gives_its_picture_its_mask_and_its_tip() {
        let c = decode(&cur((1, 0))).expect("decoded");
        let f = &c.frames[0];
        assert_eq!((f.width, f.height, f.hot_x, f.hot_y), (2, 2, 1, 0));
        assert_eq!(
            f.argb,
            vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xff00_0000]
        );
        assert_eq!(c.steps, vec![(0, 0)]);
    }

    #[test]
    fn an_ani_steps_through_its_icons_at_its_rates() {
        let icon = cur((0, 0));
        let chunk = |id: &[u8], body: &[u8]| {
            let mut c = id.to_vec();
            c.extend((body.len() as u32).to_le_bytes());
            c.extend(body);
            if body.len() % 2 == 1 {
                c.push(0);
            }
            c
        };
        let mut anih = Vec::new();
        for v in [36u32, 2, 3, 0, 0, 0, 0, 6, 3] {
            anih.extend(v.to_le_bytes());
        }
        let mut rate = Vec::new();
        for v in [6u32, 12, 3] {
            rate.extend(v.to_le_bytes());
        }
        let mut seq = Vec::new();
        for v in [0u32, 1, 0] {
            seq.extend(v.to_le_bytes());
        }
        let mut fram = b"fram".to_vec();
        fram.extend(chunk(b"icon", &icon));
        fram.extend(chunk(b"icon", &icon));
        let mut body = b"ACON".to_vec();
        body.extend(chunk(b"anih", &anih));
        body.extend(chunk(b"rate", &rate));
        body.extend(chunk(b"seq ", &seq));
        body.extend(chunk(b"LIST", &fram));
        let file = chunk(b"RIFF", &body);
        let c = decode(&file).expect("decoded");
        assert_eq!(c.frames.len(), 2);
        assert_eq!(c.steps, vec![(0, 100), (1, 200), (0, 50)]);
    }

    #[test]
    fn the_main_window_picks_winamps_cursors() {
        let at = |x, y| slot_at(ClassicWindow::Main, 275, 116, x, y).map(|s| CURSOR_FILES[s].0);
        assert_eq!(
            at(180, 60),
            Some("volbal.cur"),
            "balance shares the volume's"
        );
        assert_eq!(at(100, 75), Some("posbar.cur"));
        assert_eq!(at(268, 5), Some("close.cur"));
        assert_eq!(at(100, 5), Some("titlebar.cur"));
        assert_eq!(at(150, 30), Some("songname.cur"));
        assert_eq!(at(50, 100), Some("normal.cur"));
    }

    #[test]
    fn the_other_windows_pick_theirs() {
        let s = |w, ww, hh, x, y| slot_at(w, ww, hh, x, y).map(|s| CURSOR_FILES[s].0);
        assert_eq!(
            s(ClassicWindow::Equalizer, 275, 116, 25, 60),
            Some("eqslid.cur")
        );
        assert_eq!(
            s(ClassicWindow::Equalizer, 275, 116, 80, 60),
            Some("eqslid.cur")
        );
        assert_eq!(
            s(ClassicWindow::Equalizer, 275, 116, 50, 60),
            Some("eqnormal.cur")
        );
        assert_eq!(
            s(ClassicWindow::Playlist, 300, 145, 290, 140),
            Some("psize.cur")
        );
        assert_eq!(
            s(ClassicWindow::Playlist, 300, 145, 289, 50),
            Some("pvscroll.cur")
        );
        assert_eq!(
            s(ClassicWindow::PlaylistShade, 300, 14, 275, 7),
            Some("pwssize.cur")
        );
        assert_eq!(
            s(ClassicWindow::MainShade, 275, 14, 230, 7),
            Some("wsposbar.cur")
        );
        assert_eq!(
            s(ClassicWindow::Video, 275, 232, 268, 5),
            Some("pclose.cur")
        );
        assert_eq!(s(ClassicWindow::Gen, 275, 116, 100, 5), Some("ptbar.cur"));
        assert_eq!(
            s(ClassicWindow::Gen, 275, 116, 100, 50),
            Some("pnormal.cur"),
            "over the library too"
        );
        assert_eq!(
            s(ClassicWindow::Gen, 275, 116, 100, 13),
            Some("pnormal.cur")
        );
        assert_eq!(s(ClassicWindow::Gen, 275, 116, 273, 114), Some("psize.cur"));
        assert_eq!(
            s(ClassicWindow::Gen, 275, 116, 274, 114),
            Some("pnormal.cur")
        );
        assert_eq!(s(ClassicWindow::Gen, 275, 116, 270, 110), Some("psize.cur"));
    }
}
