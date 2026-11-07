// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic main window, pixel for pixel as Winamp 5.666 paints it.

use crate::classic::font;
use crate::classic::{region, ClassicWindow};
use crate::draw::{Align, BitmapDef, DrawCmd, Rect, Region, Rgb, TextCmd, VAlign};
use crate::gui::{Kind, ObjId};
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// The title row: from x 111 to 265, 6 rows high at y 27.
const TITLE_LEFT: i32 = 111;
const TITLE_RIGHT: i32 = 265;
const TITLE_TOP: i32 = 27;
/// A title this wide or wider scrolls, with `  ***  ` between its turns.
const SCROLL_FROM: i32 = 147;

impl SkinRuntime {
    /// Draws classic window object `o`.
    pub(crate) fn draw_classic(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let Kind::ClassicWindow(w) = self.objects[o].kind else {
            return;
        };
        let at = self.objects[o].base.rect;
        match w {
            ClassicWindow::Main => self.draw_classic_main(o, at, out),
            ClassicWindow::Equalizer => self.draw_classic_eq(o, at, out),
            ClassicWindow::Playlist => self.draw_classic_pl(o, at, out),
        }
    }

    /// The shape `region.txt` gives classic window object `o`: its section's polygons over the
    /// object's rect; none without a section, which leaves the rectangle.
    pub(crate) fn classic_region(&mut self, o: ObjId, w: ClassicWindow) -> Option<Region> {
        let rect = self.objects[o].base.rect;
        let polys = self.classic_regions.get(w.region_section())?;
        let shape = region::region(polys, rect.width, rect.height);
        let Region::Op { add, .. } = shape else {
            return None;
        };
        let moved = add
            .into_iter()
            .map(|r| match r {
                Region::Box(b) => Region::Box(Rect::new(
                    b.left + rect.left,
                    b.top + rect.top,
                    b.width,
                    b.height,
                )),
                other => other,
            })
            .collect();
        Some(Region::Op {
            base: None,
            add: moved,
            keep: Vec::new(),
            cut: Vec::new(),
        })
    }

    fn draw_classic_main(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let mut p = Painter { at, out };
        // the background, then the title bar over it
        p.blit("main.bmp", 0, 0, 275, 116, 0, 0);
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        p.blit(
            "titlebar.bmp",
            27,
            if active { 0 } else { 15 },
            275,
            14,
            0,
            0,
        );
        // the clutter bar's letters; the title bar's buttons are the strip's own until pressed
        p.blit("titlebar.bmp", 304, 0, 8, 43, 10, 22);
        let status = self.player.status();
        // the play state light and its work indicator
        let state_x = match status {
            Status::Playing => 0,
            Status::Paused => 9,
            Status::Stopped => 18,
        };
        p.blit("playpaus.bmp", state_x, 0, 9, 9, 26, 28);
        if status == Status::Playing {
            p.blit("playpaus.bmp", 36, 0, 3, 9, 24, 28);
        } else {
            p.blit("playpaus.bmp", 27, 0, 2, 9, 24, 28);
        }
        self.draw_classic_time(&mut p, status);
        self.draw_classic_title(&mut p);
        if status != Status::Stopped {
            self.draw_classic_rates(&mut p);
        }
        // the mono and stereo lights, both out while nothing plays
        let channels = if status == Status::Stopped {
            0
        } else {
            self.player.channels()
        };
        p.blit(
            "monoster.bmp",
            29,
            if channels == 1 { 0 } else { 12 },
            28,
            12,
            212,
            41,
        );
        p.blit(
            "monoster.bmp",
            0,
            if channels == 2 { 0 } else { 12 },
            29,
            12,
            239,
            41,
        );
        // the volume and balance sliders: a background frame by the value, the thumb over it
        let volume = self.player.volume().clamp(0, 255);
        p.blit("volume.bmp", 0, (volume * 27 / 255) * 15, 68, 13, 107, 57);
        p.blit(
            "volume.bmp",
            15,
            422,
            14,
            11,
            107 + (volume * 51 / 255).min(54),
            58,
        );
        let balance = self.player.pan().clamp(-127, 127);
        let balance_sheet = if self.files.exists("balance.bmp") {
            "balance.bmp"
        } else {
            "volume.bmp"
        };
        p.blit(
            balance_sheet,
            9,
            (balance.abs() * 27 / 127) * 15,
            38,
            13,
            177,
            57,
        );
        let thumb = (balance * 12 / 127 + 12).clamp(0, 24);
        p.blit(balance_sheet, 15, 422, 14, 11, 177 + thumb, 58);
        // the equalizer and playlist toggles, shuffle and repeat
        let eq_on = self.classic_window_open("equalizer");
        let pl_on = self.classic_window_open("pledit");
        p.blit(
            "shufrep.bmp",
            0,
            61 + if eq_on { 12 } else { 0 },
            23,
            12,
            219,
            58,
        );
        p.blit(
            "shufrep.bmp",
            23,
            61 + if pl_on { 12 } else { 0 },
            23,
            12,
            242,
            58,
        );
        let shuffle = self.player.shuffle();
        let repeat = self.player.repeat();
        p.blit(
            "shufrep.bmp",
            28,
            if shuffle { 15 } else { 0 },
            47,
            15,
            164,
            89,
        );
        p.blit(
            "shufrep.bmp",
            0,
            if repeat { 15 } else { 0 },
            28,
            15,
            210,
            89,
        );
        // the position bar while something plays, the control buttons and eject
        if status != Status::Stopped {
            let length = self.player.length_ms();
            let pos = if length > 0 {
                (self.player.position_ms() as i64 * 255 / length as i64).clamp(0, 255) as i32
            } else {
                0
            };
            let x = (pos * 219 / 256).clamp(0, 219);
            p.blit("posbar.bmp", 0, 0, 248, 10, 16, 72);
            p.blit("posbar.bmp", 248, 0, 29, 10, 16 + x, 72);
        }
        p.blit("cbuttons.bmp", 0, 0, 114, 18, 16, 88);
        p.blit("cbuttons.bmp", 114, 0, 22, 16, 136, 89);
    }

    /// The time in `numbers.bmp` (else `nums_ex.bmp`): minutes at 48 and 60, seconds at 78 and
    /// 90, 9 x 13 digits at y 26; the slot at 36 holds a blank, a hundreds digit or the minus sign.
    fn draw_classic_time(&mut self, p: &mut Painter, status: Status) {
        let (sheet, ex) = if self.files.exists("numbers.bmp") {
            ("numbers.bmp", false)
        } else {
            ("nums_ex.bmp", true)
        };
        let blank = |p: &mut Painter, x: i32| p.blit(sheet, 90, 0, 9, 13, x, 26);
        if status == Status::Stopped {
            blank(p, 36);
            if !ex {
                p.blit(sheet, 9, 6, 5, 1, 40, 32);
            }
            for x in [48, 60, 78, 90] {
                blank(p, x);
            }
            return;
        }
        let seconds = (self.player.position_ms() / 1000).max(0);
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        blank(p, 36);
        if minutes / 100 != 0 {
            p.blit(sheet, ((minutes / 100) % 10) * 9, 0, 9, 13, 36, 26);
        }
        p.blit(sheet, ((minutes / 10) % 10) * 9, 0, 9, 13, 48, 26);
        p.blit(sheet, (minutes % 10) * 9, 0, 9, 13, 60, 26);
        p.blit(sheet, ((seconds / 10) % 10) * 9, 0, 9, 13, 78, 26);
        p.blit(sheet, (seconds % 10) * 9, 0, 9, 13, 90, 26);
    }

    /// The song title in the `text.bmp` font along the title row: "1. Title (3:35)" with the
    /// playlist number, the rest of the row blank columns; a title that does not fit scrolls.
    fn draw_classic_title(&mut self, p: &mut Painter) {
        let text = self.classic_title_text();
        let width = text.chars().count() as i32 * 5;
        let (text, offset) = if width < SCROLL_FROM {
            (text, 0)
        } else {
            let gap = "  ***  ";
            let turn = width + gap.chars().count() as i32 * 5;
            let scroll = self.classic_scroll.rem_euclid(turn);
            (format!("{text}{gap}{text}"), scroll)
        };
        let mut x = TITLE_LEFT;
        let mut skip = offset % 5;
        let mut chars = text.chars().skip((offset / 5) as usize);
        while x < TITLE_RIGHT + 4 {
            let Some(c) = chars.next() else {
                break;
            };
            let (cx, cy) = font::cell(c);
            if x > TITLE_RIGHT - 6 {
                p.blit("text.bmp", cx, cy, TITLE_RIGHT + 4 - x, 6, x, TITLE_TOP);
                return;
            }
            p.blit("text.bmp", cx + skip, cy, 5 - skip, 6, x, TITLE_TOP);
            x += 5 - skip;
            skip = 0;
        }
        // the row's rest: the blank column at the end of the first cell
        while x < TITLE_RIGHT + 4 {
            p.blit("text.bmp", 4, 0, 1, 6, x, TITLE_TOP);
            x += 1;
        }
    }

    /// The title row's text: the playlist number, the title and the length, as Winamp writes it
    /// with numbers in the playlist on; "Winamp 5.666" with nothing loaded.
    fn classic_title_text(&mut self) -> String {
        let title = self.player.title();
        if title.is_empty() {
            return "Winamp 5.666".to_string();
        }
        let length = self.player.length_ms() / 1000;
        let index = self.player.track_index() + 1;
        format!("{index}. {title} ({}:{:02})", length / 60, length % 60)
    }

    /// The bitrate at 111 and the sample rate at 156, 5 x 6 digits from `text.bmp` at y 43; a
    /// short number keeps its leading cells blank.
    fn draw_classic_rates(&mut self, p: &mut Painter) {
        let digit =
            |p: &mut Painter, n: i32, x: i32| p.blit("text.bmp", (n % 10) * 5, 6, 5, 6, x, 43);
        let blank = |p: &mut Painter, x: i32| p.blit("text.bmp", 100, 12, 5, 6, x, 43);
        let kbps = self.player.bitrate_kbps().max(0);
        if kbps >= 1000 {
            // four digits: the thousands and hundreds, then the tens as a letter cell
            digit(p, kbps / 1000, 111);
            digit(p, kbps / 100, 116);
            let (cx, cy) = font::cell(char::from(b'0' + ((kbps / 10) % 10) as u8));
            p.blit("text.bmp", cx, cy, 5, 6, 121, 43);
        } else {
            if kbps / 100 == 0 {
                blank(p, 111);
            } else {
                digit(p, kbps / 100, 111);
            }
            if kbps / 10 == 0 {
                blank(p, 116);
            } else {
                digit(p, kbps / 10, 116);
            }
            digit(p, kbps, 121);
        }
        let khz = self.player.sample_rate_khz().max(0);
        if khz / 10 == 0 {
            blank(p, 156);
        } else {
            digit(p, khz / 10, 156);
        }
        digit(p, khz, 161);
    }

    /// Whether the classic window of container `id` is open.
    fn classic_window_open(&mut self, id: &str) -> bool {
        self.container(id).is_some_and(|c| self.is_open(c))
    }
}

/// Blits into a window's draw list at the window's place.
pub(super) struct Painter<'a> {
    pub(super) at: Rect,
    pub(super) out: &'a mut Vec<DrawCmd>,
}

impl Painter<'_> {
    /// `w` x `h` of `file` from `sx`, `sy` stretched to `dw` x `dh` at `dx`, `dy`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stretch(
        &mut self,
        file: &str,
        sx: i32,
        sy: i32,
        w: i32,
        h: i32,
        dx: i32,
        dy: i32,
        dw: i32,
        dh: i32,
    ) {
        if w <= 0 || h <= 0 || dw <= 0 || dh <= 0 {
            return;
        }
        self.out.push(DrawCmd::Image {
            bitmap: BitmapDef::new(file, file, sx, sy, Some(w), Some(h), None),
            dst: Rect::new(self.at.left + dx, self.at.top + dy, dw, dh),
            alpha: 255,
            gamma: None,
            smooth: false,
        });
    }

    /// `text` in the platform's font at `size`, in the box `dx`, `dy`, `w` x `h`, `align`ed.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn text(
        &mut self,
        text: &str,
        dx: i32,
        dy: i32,
        w: i32,
        h: i32,
        color: Rgb,
        align: Align,
    ) {
        self.out.push(DrawCmd::Text(Box::new(TextCmd {
            text: text.to_string(),
            font: None,
            size: super::pl::ROW_FONT,
            color,
            rect: Rect::new(self.at.left + dx, self.at.top + dy, w, h),
            align,
            scroll: 0,
            alpha: 255,
            valign: VAlign::Center,
            bold: false,
            shadow: None,
            shadow_dx: 1,
            shadow_dy: 1,
            x: 0.0,
            baseline: 0.0,
        })));
    }

    /// `w` x `h` at `dx`, `dy` filled with `color`.
    pub(super) fn fill(&mut self, dx: i32, dy: i32, w: i32, h: i32, color: Rgb) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.out.push(DrawCmd::Fill {
            rect: Rect::new(self.at.left + dx, self.at.top + dy, w, h),
            color,
            alpha: 255,
        });
    }

    /// `w` x `h` of `file` from `sx`, `sy` to `dx`, `dy` of the window.
    pub(super) fn blit(&mut self, file: &str, sx: i32, sy: i32, w: i32, h: i32, dx: i32, dy: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.out.push(DrawCmd::Image {
            bitmap: BitmapDef::new(file, file, sx, sy, Some(w), Some(h), None),
            dst: Rect::new(self.at.left + dx, self.at.top + dy, w, h),
            alpha: 255,
            gamma: None,
            smooth: false,
        });
    }
}
