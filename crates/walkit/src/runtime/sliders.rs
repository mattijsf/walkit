// SPDX-License-Identifier: GPL-3.0-or-later
//! Sliders: thumb and bar, the value they show, and what a drag does.

use super::objects::{scaled, PAN_REACH};
use super::SkinRuntime;
use crate::draw::{DrawCmd, Rect};
use crate::gui::{Kind, ObjId};
use crate::maki::{Handle, Value};
use crate::player::Status;

impl SkinRuntime {
    fn slider(&self, o: ObjId) -> &crate::gui::SliderData {
        match &self.objects[o].kind {
            Kind::Slider(s) => s,
            _ => panic!("not a slider"),
        }
    }

    fn slider_mut(&mut self, o: ObjId) -> &mut crate::gui::SliderData {
        match &mut self.objects[o].kind {
            Kind::Slider(s) => s,
            _ => panic!("not a slider"),
        }
    }

    fn slider_range(&self, o: ObjId) -> i32 {
        let s = self.slider(o);
        (s.high - s.low).max(1)
    }

    fn thumb_size(&mut self, o: ObjId) -> (i32, i32) {
        let thumb = self.slider(o).thumb.clone();
        thumb.and_then(|t| self.bitmap_size(&t)).unwrap_or((0, 0))
    }

    /// The thumb's box for the current position.
    fn thumb_rect(&mut self, o: ObjId) -> Rect {
        let (tw, th) = self.thumb_size(o);
        let rect = self.objects[o].base.rect;
        let s = self.slider(o);
        let frac = (s.position - s.low) as f64 / self.slider_range(o) as f64;
        if s.vertical {
            let travel = (rect.height - th).max(0);
            Rect::new(
                rect.left + (rect.width - tw) / 2,
                rect.top + ((1.0 - frac) * travel as f64) as i32,
                tw,
                th,
            )
        } else {
            let travel = (rect.width - tw).max(0);
            Rect::new(
                rect.left + (frac * travel as f64) as i32,
                rect.top + (rect.height - th) / 2,
                tw,
                th,
            )
        }
    }

    pub(super) fn draw_slider(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let s = self.slider(o).clone();
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        let bl = s.bar_left.as_ref().and_then(|b| self.bitmap(b));
        let bm = s.bar_middle.as_ref().and_then(|b| self.bitmap(b));
        let br = s.bar_right.as_ref().and_then(|b| self.bitmap(b));
        if let Some(bm) = bm {
            let lw = bl
                .as_ref()
                .and_then(|b| self.bitmap_size(&b.id))
                .map(|s| s.0)
                .unwrap_or(0);
            let rw = br
                .as_ref()
                .and_then(|b| self.bitmap_size(&b.id))
                .map(|s| s.0)
                .unwrap_or(0);
            if let Some(bl) = bl {
                out.push(DrawCmd::image(
                    bl,
                    Rect::new(rect.left, rect.top, lw, rect.height),
                    alpha,
                ));
            }
            out.push(DrawCmd::image(
                bm,
                Rect::new(rect.left + lw, rect.top, rect.width - lw - rw, rect.height),
                alpha,
            ));
            if let Some(br) = br {
                out.push(DrawCmd::image(
                    br,
                    Rect::new(rect.right() - rw, rect.top, rw, rect.height),
                    alpha,
                ));
            }
        }
        // a seek bar has no thumb while nothing plays
        if s.action.as_deref() == Some("seek")
            && self.player.status() == Status::Stopped
            && !s.dragging
        {
            return;
        }
        let img = if s.dragging {
            s.down_thumb.clone()
        } else {
            None
        }
        .or(s.thumb.clone());
        let Some(img) = img else { return };
        if let Some(bm) = self.bitmap(&img) {
            let tr = self.thumb_rect(o);
            out.push(DrawCmd::image(bm, tr, alpha));
        }
    }

    fn slider_position_from(&mut self, o: ObjId, px: i32, py: i32) -> i32 {
        let (tw, th) = self.thumb_size(o);
        let rect = self.objects[o].base.rect;
        let s = self.slider(o);
        let frac = if s.vertical {
            let travel = (rect.height - th).max(1);
            1.0 - (py - rect.top - s.grab) as f64 / travel as f64
        } else {
            let travel = (rect.width - tw).max(1);
            (px - rect.left - s.grab) as f64 / travel as f64
        };
        let value = s.low + (frac.clamp(0.0, 1.0) * self.slider_range(o) as f64) as i32;
        self.slider_hot_spot(o, value)
    }

    /// Whether `s` is an EQ band slider (not the preamp).
    fn is_eq_band(s: &crate::gui::SliderData) -> bool {
        s.action.as_deref() == Some("eq_band")
            && !s
                .param
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case("preamp"))
    }

    /// The other EQ band of the same window under the finger at `sx`, `sy` of the stack while
    /// the band `o` is dragged: Winamp hands the drag to it, as a band dragged over another
    /// band of its window lets that one take over. The preamp neither
    /// hands its drag on nor takes one.
    pub(crate) fn eq_band_reached(&mut self, o: ObjId, sx: i32, sy: i32) -> Option<ObjId> {
        match &self.objects[o].kind {
            Kind::Slider(s) if s.dragging && Self::is_eq_band(s) => {}
            _ => return None,
        }
        let h = self.hit_test_stack(sx, sy)?;
        if h == o || self.layout_object(h) != self.layout_object(o) {
            return None;
        }
        match &self.objects[h].kind {
            Kind::Slider(s) if Self::is_eq_band(s) && !s.locked => Some(h),
            _ => None,
        }
    }

    /// Where a dragged balance or EQ slider settles: Winamp's balance, band and preamp sliders
    /// snap to their middle when the finger brings them within a reach of it:
    /// 7.5 % of the track's pixels, cut to whole pixels, in steps of the range.
    /// A volume or seek slider has no such spot.
    fn slider_hot_spot(&self, o: ObjId, value: i32) -> i32 {
        let s = self.slider(o);
        let hot = match s.action.as_deref() {
            Some("pan") => s.low + PAN_REACH,
            Some("eq_band" | "eq_preamp") => 0,
            _ => return value,
        };
        let rect = self.objects[o].base.rect;
        let track = if s.vertical { rect.height } else { rect.width };
        if track <= 0 {
            return value;
        }
        let reach = (f64::from(track) * 0.075) as i32 * (s.high - s.low) / track;
        if (value - hot).abs() < reach {
            hot
        } else {
            value
        }
    }

    /// Sets the position and tells the script; `from_user` also drives the player first, and
    /// the thumb goes where the player snapped to, so the script shows the snapped value.
    pub(crate) fn slider_set_position(&mut self, o: ObjId, p: i32, from_user: bool) {
        let (low, high) = (self.slider(o).low, self.slider(o).high);
        let clamped = p.clamp(low, high.max(low));
        if clamped == self.slider(o).position && from_user {
            return;
        }
        self.slider_mut(o).position = clamped;
        if from_user {
            if let Some(snapped) = self.slider_moved(o, false) {
                self.slider_mut(o).position = snapped.clamp(low, high.max(low));
            }
        }
        self.repaint();
        let pos = self.slider(o).position;
        self.dispatch(Handle::Gui(o), "onsetposition", &[Value::Int(pos)]);
    }

    pub(crate) fn slider_pointer_down(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).locked {
            return;
        }
        self.slider_mut(o).dragging = true;
        // held where it was grabbed; a tap beside the thumb lands it just right of (below) the finger
        let (tw, th) = self.thumb_size(o);
        let t = self.thumb_rect(o);
        let vertical = self.slider(o).vertical;
        self.slider_mut(o).grab = if vertical {
            th / 2 - 2
        } else if px >= t.left && px < t.right() {
            px - t.left - 1
        } else {
            tw / 2 - 2
        };
        let p = self.slider_position_from(o, px, py);
        self.slider_set_position(o, p, true);
        self.dispatch(
            Handle::Gui(o),
            "onleftbuttondown",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn slider_pointer_move(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).dragging {
            let p = self.slider_position_from(o, px, py);
            self.slider_set_position(o, p, true);
        }
        self.dispatch(
            Handle::Gui(o),
            "onmousemove",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn slider_pointer_up(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).dragging {
            self.slider_mut(o).dragging = false;
            self.repaint();
            self.dispatch(
                Handle::Gui(o),
                "onleftbuttonup",
                &[Value::Int(px), Value::Int(py)],
            );
            // a menu the release handler opened took the touch
            if self.pointer.press_ended {
                return;
            }
            let pos = self.slider(o).position;
            self.dispatch(Handle::Gui(o), "onsetfinalposition", &[Value::Int(pos)]);
            self.dispatch(Handle::Gui(o), "onpostedposition", &[Value::Int(pos)]);
            self.slider_moved(o, true);
        }
    }

    pub(crate) fn slider_pointer_cancel(&mut self, o: ObjId) {
        if self.slider(o).dragging {
            self.slider_mut(o).dragging = false;
            self.repaint();
            let (px, py) = self.last_pointer;
            self.dispatch(
                Handle::Gui(o),
                "onleftbuttonup",
                &[Value::Int(px), Value::Int(py)],
            );
        }
    }

    /// Puts the thumb where the player is until its first sync.
    pub(crate) fn slider_place_if_unsynced(&mut self, o: ObjId) {
        let s = self.slider(o);
        if s.dragging || s.synced {
            return;
        }
        if let Some(t) = self.slider_target(o) {
            let s = self.slider_mut(o);
            s.position = t.clamp(s.low, s.high.max(s.low));
        }
    }

    /// Follows the player when the user is not holding the thumb.
    pub(crate) fn slider_sync(&mut self, o: ObjId) {
        if self.slider(o).dragging {
            return;
        }
        let Some(target) = self.slider_target(o) else {
            return;
        };
        let s = self.slider(o);
        if target != s.position || !s.synced {
            let synced = s.synced;
            let s = self.slider_mut(o);
            s.position = target.clamp(s.low, s.high.max(s.low));
            // a thumb out of sight draws nothing; it shows where it is when it shows again
            if self.shows(o) {
                self.repaint();
            }
            // the first placement is the skin starting up, not a change scripts react to
            if synced {
                let pos = self.slider(o).position;
                self.dispatch(Handle::Gui(o), "onsetposition", &[Value::Int(pos)]);
                self.dispatch(Handle::Gui(o), "onpostedposition", &[Value::Int(pos)]);
            }
            self.slider_mut(o).synced = true;
        }
    }

    /// Where the player says a slider should sit, or `None` for one the player does not drive.
    pub(crate) fn slider_target(&mut self, o: ObjId) -> Option<i32> {
        let s = self.slider(o).clone();
        let span = s.high - s.low;
        let p = &*self.player;
        Some(match s.action.as_deref() {
            Some("seek") => {
                if p.length_ms() > 0 {
                    s.low + scaled(p.position_ms(), p.length_ms(), span)
                } else {
                    s.low
                }
            }
            Some("volume") => s.low + scaled(p.volume(), 255, span),
            Some("eq_band") => s.low + scaled(self.eq_value(s.param.as_deref())?, 255, span),
            Some("eq_preamp") => s.low + scaled(p.eq_preamp(), 255, span),
            // the middle at 127 of 0..255, which Winamp Modern's display reads as the center;
            // the balance is the position less 127
            Some("pan") => s.low + (p.pan() + PAN_REACH) * span / 255,
            _ => return None,
        })
    }

    /// An EQ slider's value: `param` is a band counted from 1, or "preamp".
    fn eq_value(&self, param: Option<&str>) -> Option<i32> {
        let param = param?;
        if param.eq_ignore_ascii_case("preamp") {
            Some(self.player.eq_preamp())
        } else {
            let i = param.parse::<i32>().ok()? - 1;
            usize::try_from(i)
                .ok()
                .and_then(|i| self.player.eq_bands().get(i).copied())
        }
    }

    /// Drives the player from a slider the listener moved; the values round to the nearest.
    /// Returns where the thumb belongs when the player took another value than the slider's,
    /// or `None` when it took the slider's.
    pub(crate) fn slider_moved(&mut self, o: ObjId, final_pos: bool) -> Option<i32> {
        let s = self.slider(o).clone();
        let frac = (s.position - s.low) as f64 / (s.high - s.low).max(1) as f64;
        let level = (frac * 255.0).round() as i32;
        let taken = match s.action.as_deref() {
            Some("seek") => {
                if final_pos {
                    let to = (frac * self.player.length_ms() as f64).round() as i32;
                    self.player.seek_to(to);
                    let pos = self.player.position_ms();
                    self.dispatch_system("onseek", &[Value::Int(pos)]);
                }
                true
            }
            Some("volume") => {
                self.player.set_volume(level);
                true
            }
            Some("eq_band") => {
                if s.param
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case("preamp"))
                {
                    self.player.set_eq_preamp(level);
                    self.player.eq_preamp() == level
                } else {
                    if let Some(band) = s.param.as_deref().and_then(|p| p.parse::<i32>().ok()) {
                        self.player.set_eq_band(band - 1, level);
                    }
                    true
                }
            }
            Some("eq_preamp") => {
                self.player.set_eq_preamp(level);
                self.player.eq_preamp() == level
            }
            Some("pan") => {
                let pan = (level - PAN_REACH).clamp(-PAN_REACH, PAN_REACH);
                self.player.set_pan(pan);
                self.player.pan() == pan
            }
            _ => true,
        };
        if taken {
            None
        } else {
            self.slider_target(o)
        }
    }
}
