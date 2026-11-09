// SPDX-License-Identifier: GPL-3.0-or-later
//! The classic main window, pixel for pixel as Winamp 5.666 paints it.

use crate::classic::font;
use crate::classic::touch::Control;
use crate::classic::vis::{self, VisMode};
use crate::classic::{region, ClassicWindow};
use crate::draw::{Align, BitmapDef, DrawCmd, Rect, Region, Rgb, TextCmd, VAlign};
use crate::gui::{Kind, ObjId};
use crate::player::Status;
use crate::runtime::SkinRuntime;

/// The title row: from x 111 up to 265, 6 rows high at y 27.
const TITLE_LEFT: i32 = 111;
const TITLE_RIGHT: i32 = 265;
const TITLE_TOP: i32 = 27;
/// A title this wide or wider scrolls, with `  ***  ` between its turns.
const SCROLL_FROM: i32 = 147;
/// The title scrolls a cell every 200 ms (the VM showed 5 px steps every 205 ms on average).
pub(crate) const TITLE_STEP_MS: i64 = 200;
const TITLE_STEP_PX: i32 = 5;
/// Paused, the time digits go dark and light again in turns of this long (the VM showed
/// 1367 ms dark, 1337 ms lit).
pub(crate) const PAUSE_BLINK_MS: i64 = 1350;

impl SkinRuntime {
    /// Draws classic window object `o`.
    pub(crate) fn draw_classic(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let Kind::ClassicWindow(w) = self.objects[o].kind else {
            return;
        };
        let at = self.objects[o].base.rect;
        match w {
            ClassicWindow::Main => self.draw_classic_main(o, at, out),
            ClassicWindow::MainShade => self.draw_classic_main_shade(o, at, out),
            ClassicWindow::Equalizer => self.draw_classic_eq(o, at, out),
            ClassicWindow::EqualizerShade => self.draw_classic_eq_shade(o, at, out),
            ClassicWindow::Playlist => self.draw_classic_pl(o, at, out),
            ClassicWindow::PlaylistShade => self.draw_classic_pl_shade(o, at, out),
            ClassicWindow::Gen => self.draw_classic_gen(o, at, out),
            ClassicWindow::Video => self.draw_classic_video(o, at, out),
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
        let status = self.player.status();
        // the vis picture first: the analyzer reads the player and steps the clock; off, the
        // area shows the palette's background while something plays (the VM, sa=0)
        let vis = match (status, self.classic_vis_mode) {
            (Status::Playing, VisMode::Analyzer) => {
                let levels = self.player.spectrum();
                let now = self.now();
                let style = self.classic_vis_style;
                Some(self.classic_vis.frame(now, &levels, &style))
            }
            (Status::Playing, VisMode::Scope) => Some(vis::scope(
                &self.player.waveform(),
                self.classic_vis_style.scope,
            )),
            (Status::Playing, VisMode::Off) => Some([vis::OFF; vis::WIDTH * vis::HEIGHT]),
            _ => None,
        };
        let palette = self.classic_palette;
        let mut p = Painter { at, out };
        // the background, then the title bar over it
        p.blit("main.bmp", 0, 0, 275, 116, 0, 0);
        if let Some(px) = &vis {
            p.vis(px, vis::WIDTH, &palette, 24, 43);
        }
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        let press = self
            .classic_press_on(o)
            .map(|p| (p.control, p.inside, p.position));
        let pressed = |c: Control| press.is_some_and(|(pc, inside, _)| pc == c && inside);
        p.blit(
            "titlebar.bmp",
            27,
            if active { 0 } else { 15 },
            275,
            14,
            0,
            0,
        );
        // the clutter bar: a held letter shows the column from y 44 with that
        // letter pressed; A and D light while always on top and double size are on, unless held
        let letters = [
            Control::ClutterO,
            Control::ClutterA,
            Control::ClutterI,
            Control::ClutterD,
            Control::ClutterV,
        ];
        let held = letters.iter().position(|c| pressed(*c));
        match held {
            Some(k) => p.blit("titlebar.bmp", 304 + 8 * k as i32, 44, 8, 43, 10, 22),
            None => p.blit("titlebar.bmp", 304, 0, 8, 43, 10, 22),
        }
        if held != Some(1) {
            let on = self.host.window_open("ontop") == Some(true);
            let (sx, sy) = if on { (313, 55) } else { (305, 11) };
            p.blit("titlebar.bmp", sx, sy, 7, 8, 11, 33);
        }
        if held != Some(3) {
            let on = self.host.window_open("doublesize") == Some(true);
            let (sx, sy) = if on { (329, 71) } else { (305, 27) };
            p.blit("titlebar.bmp", sx, sy, 7, 6, 11, 49);
        }
        // the title bar's buttons are the strip's until pressed
        if pressed(Control::Menu) {
            p.blit("titlebar.bmp", 0, 9, 9, 9, 6, 3);
        }
        if pressed(Control::Minimize) {
            p.blit("titlebar.bmp", 9, 9, 9, 9, 244, 3);
        }
        if pressed(Control::Shade) {
            p.blit("titlebar.bmp", 9, 18, 9, 9, 254, 3);
        }
        if pressed(Control::Close) {
            p.blit("titlebar.bmp", 18, 9, 9, 9, 264, 3);
        }
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
        let thumb_x = if pressed(Control::Volume) { 0 } else { 15 };
        p.blit(
            "volume.bmp",
            thumb_x,
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
        let thumb_x = if pressed(Control::Balance) { 0 } else { 15 };
        p.blit(balance_sheet, thumb_x, 422, 14, 11, 177 + thumb, 58);
        // the equalizer and playlist toggles, shuffle and repeat
        let eq_on = self.classic_window_open("equalizer");
        let pl_on = self.classic_window_open("pledit");
        let eq_px = if pressed(Control::EqToggle) { 46 } else { 0 };
        let pl_px = if pressed(Control::PlToggle) { 46 } else { 0 };
        let eq_y = 61 + if eq_on { 12 } else { 0 };
        let pl_y = 61 + if pl_on { 12 } else { 0 };
        p.blit("shufrep.bmp", eq_px, eq_y, 23, 12, 219, 58);
        p.blit("shufrep.bmp", 23 + pl_px, pl_y, 23, 12, 242, 58);
        let shuffle = self.player.shuffle();
        let repeat = self.player.repeat();
        // shufrep.bmp stacks off, off pressed, on, on pressed
        let shuffle_y =
            if pressed(Control::Shuffle) { 15 } else { 0 } + if shuffle { 30 } else { 0 };
        let repeat_y = if pressed(Control::Repeat) { 15 } else { 0 } + if repeat { 30 } else { 0 };
        p.blit("shufrep.bmp", 28, shuffle_y, 47, 15, 164, 89);
        p.blit("shufrep.bmp", 0, repeat_y, 28, 15, 210, 89);
        // the position bar while something plays, the control buttons and eject
        if status != Status::Stopped {
            let length = self.player.length_ms();
            let dragging = press.filter(|(c, _, _)| *c == Control::Position);
            // the thumb at the bar's 0..=219: the song's place out of 256 times 219 / 256, or
            // where the finger holds it
            let x = match dragging {
                Some((_, _, p)) => p,
                None if length > 0 => {
                    let at = self.player.position_ms() as i64 * 256 / length as i64;
                    ((at * 219) >> 8).clamp(0, 219) as i32
                }
                None => 0,
            };
            p.blit("posbar.bmp", 0, 0, 248, 10, 16, 72);
            let thumb_x = if dragging.is_some() { 278 } else { 248 };
            p.blit("posbar.bmp", thumb_x, 0, 29, 10, 16 + x, 72);
        }
        p.blit("cbuttons.bmp", 0, 0, 114, 18, 16, 88);
        let spans = [
            (Control::Previous, 0, 23),
            (Control::Play, 23, 23),
            (Control::Pause, 46, 23),
            (Control::Stop, 69, 23),
            (Control::Next, 92, 22),
        ];
        for (c, sx, w) in spans {
            if pressed(c) {
                p.blit("cbuttons.bmp", sx, 18, w, 18, 16 + sx, 88);
            }
        }
        let eject_y = if pressed(Control::Eject) { 16 } else { 0 };
        p.blit("cbuttons.bmp", 114, eject_y, 22, 16, 136, 89);
    }

    /// The main window's shade bar (`titlebar.bmp` y 29 or 42): the strip, the time in small
    /// `text.bmp` digits at y 4, and the mini position bar while something plays.
    fn draw_classic_main_shade(&mut self, o: ObjId, at: Rect, out: &mut Vec<DrawCmd>) {
        let status = self.player.status();
        let active = self
            .layout_object(o)
            .is_some_and(|l| self.has_focus(Some(l)));
        // the strip's mini vis at (79, 5), 38 x 5, while something plays
        let vis = match (status, self.classic_vis_mode) {
            (Status::Playing, VisMode::Analyzer) => {
                let levels = self.player.spectrum();
                let now = self.now();
                let style = self.classic_vis_style;
                Some(self.classic_vis.frame_shade(now, &levels, &style))
            }
            (Status::Playing, VisMode::Scope) => Some(vis::scope_shade(
                &self.player.waveform(),
                self.classic_vis_style.scope,
            )),
            (Status::Playing, VisMode::Off) => {
                Some([vis::OFF; vis::SHADE_WIDTH * vis::SHADE_HEIGHT])
            }
            _ => None,
        };
        let palette = self.classic_palette;
        let mut p = Painter { at, out };
        p.blit(
            "titlebar.bmp",
            27,
            if active { 29 } else { 42 },
            275,
            14,
            0,
            0,
        );
        if let Some(px) = &vis {
            p.vis(px, vis::SHADE_WIDTH, &palette, 79, 5);
        }
        let blank = |p: &mut Painter, x: i32, w: i32| p.blit("text.bmp", 142, 0, w, 6, x, 4);
        let digit =
            |p: &mut Painter, n: i32, x: i32| p.blit("text.bmp", (n % 10) * 5, 6, 5, 6, x, 4);
        if status == Status::Stopped || self.classic_time_dark() {
            blank(&mut p, 126, 3);
            blank(&mut p, 130, 3);
            for x in [134, 139, 147, 152] {
                blank(&mut p, x, 5);
            }
            return;
        }
        let dragged = self
            .classic_press_on(o)
            .filter(|p| p.control == Control::ShadePosition);
        let ms = match dragged {
            Some(press) => {
                (i64::from(self.player.length_ms()) * i64::from(press.position) / 218) as i32
            }
            None => self.player.position_ms(),
        };
        let seconds = (ms / 1000).max(0);
        let (minutes, seconds) = (seconds / 60, seconds % 60);
        if minutes / 100 == 0 {
            blank(&mut p, 130, 3);
            blank(&mut p, 126, 3);
        } else {
            blank(&mut p, 126, 3);
            digit(&mut p, minutes / 100, 129);
        }
        digit(&mut p, minutes / 10, 134);
        digit(&mut p, minutes, 139);
        digit(&mut p, seconds / 10, 147);
        digit(&mut p, seconds, 152);
        // the mini position bar: the track, a thumb 3 wide stepping through three looks
        let length = self.player.length_ms();
        let pos = if length > 0 {
            (self.player.position_ms() as i64 * 255 / length as i64).clamp(0, 255) as i32
        } else {
            0
        };
        let thumb = ((pos * 12) >> 8).clamp(0, 12) + 1;
        let step = if thumb < 6 {
            0
        } else if thumb > 8 {
            2
        } else {
            1
        };
        p.blit("titlebar.bmp", 0, 36, 17, 7, 226, 4);
        p.blit("titlebar.bmp", 17 + step * 3, 36, 3, 7, 226 + thumb, 4);
    }

    /// The time in `numbers.bmp` (else `nums_ex.bmp`): minutes at 48 and 60, seconds at 78 and
    /// 90, 9 x 13 digits at y 26; the slot at 36 holds a blank, a hundreds digit or the minus sign.
    fn draw_classic_time(&mut self, p: &mut Painter, status: Status) {
        // nums_ex.bmp first, numbers.bmp when a skin has none
        let (sheet, ex) = if self.files.exists("nums_ex.bmp") {
            ("nums_ex.bmp", true)
        } else {
            ("numbers.bmp", false)
        };
        let blank = |p: &mut Painter, x: i32| p.blit(sheet, 90, 0, 9, 13, x, 26);
        if status == Status::Stopped || self.classic_time_dark() {
            // stopped, a nums_ex skin shows blank cells and a dash; a numbers.bmp skin leaves
            // main.bmp's time area (Winamp 5.666 shows the base skin this way)
            if ex {
                blank(p, 36);
                p.blit(sheet, 9, 6, 5, 1, 40, 32);
                for x in [48, 60, 78, 90] {
                    blank(p, x);
                }
            }
            return;
        }
        // the time elapsed, or after a press on it the time left with a minus before it
        let position = self.player.position_ms();
        let length = self.player.length_ms();
        let remaining = self.classic_remaining && length > 0;
        let ms = if remaining {
            (length - position).max(0)
        } else {
            position.max(0)
        };
        let seconds = ms / 1000;
        let minutes = seconds / 60;
        let seconds = seconds % 60;
        blank(p, 36);
        if remaining {
            if ex {
                p.blit(sheet, 99, 0, 9, 13, 36, 26);
            } else {
                p.blit(sheet, 20, 6, 5, 1, 38, 32);
            }
        }
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
        let song = self.classic_title_text();
        // a new song starts its title from the beginning
        if song != self.classic_title_key {
            self.classic_title_key.clone_from(&song);
            self.classic_title_base = 0;
            self.classic_title_epoch = self.now();
        }
        // a slider's value stands in for the song while it is dragged, from its first cell
        let (text, scrolls) = match &self.classic_status {
            Some(status) => (status.clone(), false),
            None => (song, true),
        };
        let width = text.chars().count() as i32 * 5;
        let (text, offset) = if width < SCROLL_FROM || !scrolls {
            (text, 0)
        } else {
            let gap = "  ***  ";
            let turn = width + gap.chars().count() as i32 * 5;
            let scroll = self.classic_title_scroll().rem_euclid(turn);
            (format!("{text}{gap}{text}"), scroll)
        };
        let mut x = TITLE_LEFT;
        let mut skip = offset % 5;
        let mut chars = text.chars().skip((offset / 5) as usize);
        while x < TITLE_RIGHT {
            let Some(c) = chars.next() else {
                break;
            };
            let (cx, cy) = font::cell(c);
            if x > TITLE_RIGHT - 5 {
                p.blit("text.bmp", cx, cy, TITLE_RIGHT - x, 6, x, TITLE_TOP);
                return;
            }
            p.blit("text.bmp", cx + skip, cy, 5 - skip, 6, x, TITLE_TOP);
            x += 5 - skip;
            skip = 0;
        }
        // the row's rest: the blank column at the end of the first cell
        while x < TITLE_RIGHT {
            p.blit("text.bmp", 4, 0, 1, 6, x, TITLE_TOP);
            x += 1;
        }
    }

    /// How far the title has scrolled: where it stood, then a cell per step of the clock since
    /// it last moved on.
    pub(crate) fn classic_title_scroll(&self) -> i32 {
        let now = self.now();
        if now < self.classic_title_epoch {
            return self.classic_title_base;
        }
        let steps = ((now - self.classic_title_epoch) / TITLE_STEP_MS) % i64::from(i32::MAX / 8);
        self.classic_title_base + steps as i32 * TITLE_STEP_PX
    }

    /// Holds the title's scroll where it stands.
    pub(crate) fn classic_title_hold(&mut self) {
        self.classic_title_base = self.classic_title_scroll();
        self.classic_title_epoch = i64::MAX / 2;
    }

    /// Lets the title scroll on again `delay` ms from now: right away after a slider's text, two
    /// seconds after a drag of the title (ten steps of waiting).
    pub(crate) fn classic_title_resume(&mut self, delay: i64) {
        if self.classic_title_epoch > self.now() + delay {
            self.classic_title_epoch = self.now() + delay;
        }
    }

    /// When the title's next scroll step falls, from `now`.
    pub(crate) fn classic_title_next_step(&self, now: i64) -> Option<i64> {
        if self.classic_status.is_some() || self.classic_title_epoch >= i64::MAX / 4 {
            return None;
        }
        if now < self.classic_title_epoch {
            return Some(self.classic_title_epoch);
        }
        let done = (now - self.classic_title_epoch) / TITLE_STEP_MS;
        Some(self.classic_title_epoch + (done + 1) * TITLE_STEP_MS)
    }

    /// Whether the main window's title is long enough to scroll.
    pub(crate) fn classic_title_scrolls(&mut self) -> bool {
        self.classic_title_text().chars().count() as i32 * 5 >= SCROLL_FROM
    }

    /// Whether the time digits are dark now: paused, they blink.
    pub(crate) fn classic_time_dark(&self) -> bool {
        self.player.status() == Status::Paused && (self.now() / PAUSE_BLINK_MS) % 2 == 1
    }

    /// The title row's text: the playlist number, the title and the length, as Winamp writes it
    /// with numbers in the playlist on; the Winamp's name and version with nothing loaded.
    pub(crate) fn classic_title_text(&mut self) -> String {
        let title = self.player.title();
        if title.is_empty() {
            return self.classic_style.name().to_string();
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
        let letter = |p: &mut Painter, c: char| {
            let (cx, cy) = font::cell(c);
            p.blit("text.bmp", cx, cy, 5, 6, 121, 43);
        };
        if kbps >= 10000 {
            // the hundred thousands (or a blank) and the ten thousands, then 'C'
            if kbps / 100_000 == 0 {
                blank(p, 111);
            } else {
                digit(p, kbps / 100_000, 111);
            }
            digit(p, kbps / 10000, 116);
            letter(p, 'C');
        } else if kbps >= 1000 {
            // the thousands and the hundreds, then 'H'
            digit(p, kbps / 1000, 111);
            digit(p, kbps / 100, 116);
            letter(p, 'H');
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

    /// `text` in the system font `face` (the playlist's `pledit.txt` font, a system font named
    /// by its face; none for the host's font) in the box (x, y, width, height) of the window.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn text(
        &mut self,
        text: &str,
        face: Option<&String>,
        size: i32,
        (dx, dy, w, h): (i32, i32, i32, i32),
        color: Rgb,
        align: Align,
    ) {
        let font = face
            .filter(|f| !f.is_empty())
            .map(|f| crate::draw::FontDef::new(&f.to_lowercase(), ""));
        self.out.push(DrawCmd::Text(Box::new(TextCmd {
            text: text.to_string(),
            font,
            size,
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
    #[allow(clippy::too_many_arguments)]
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
