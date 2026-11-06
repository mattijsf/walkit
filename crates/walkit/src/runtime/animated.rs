// SPDX-License-Identifier: GPL-3.0-or-later
//! Animated layers: their frames, speed and direction, and the script methods that drive them.

use super::SkinRuntime;
use crate::gui::{Kind, ObjId};
use crate::maki::{Handle, Value};

impl SkinRuntime {
    fn anim(&self, o: ObjId) -> (&crate::gui::LayerData, &crate::gui::AnimData) {
        match &self.objects[o].kind {
            Kind::AnimatedLayer(l, a) => (l, a),
            _ => panic!("not an animated layer"),
        }
    }

    fn anim_mut(&mut self, o: ObjId) -> &mut crate::gui::AnimData {
        match &mut self.objects[o].kind {
            Kind::AnimatedLayer(_, a) => a,
            _ => panic!("not an animated layer"),
        }
    }

    fn anim_full_size(&mut self, o: ObjId) -> Option<(i32, i32)> {
        let image = self.anim(o).0.image.clone()?;
        self.bitmap_size(&image)
    }

    /// Whether the frames run down the picture; with neither framewidth nor frameheight written,
    /// a picture as wide as the object runs down and any other across. `None` for a picture the
    /// object's size: one frame.
    pub(crate) fn anim_vertical(&mut self, o: ObjId) -> Option<bool> {
        if let Some(v) = self.anim(o).1.vertical_set {
            return Some(v);
        }
        let Some(full) = self.anim_full_size(o) else {
            return Some(true);
        };
        let b = &self.objects[o].base;
        let ow = b.w.unwrap_or(full.0);
        let oh = b.h.unwrap_or(full.1);
        if full.0 == ow && full.1 != oh {
            Some(true)
        } else if full.0 != ow {
            Some(false)
        } else {
            None
        }
    }

    pub(crate) fn anim_frame_size(&mut self, o: ObjId) -> (i32, i32) {
        let Some(full) = self.anim_full_size(o) else {
            return (0, 0);
        };
        let vertical = self.anim_vertical(o);
        let b = &self.objects[o].base;
        let ow = b.w.unwrap_or(full.0);
        let oh = b.h.unwrap_or(full.1);
        let a = self.anim(o).1;
        // across the strip a frame is as large as the picture
        match vertical {
            Some(true) => (full.0, a.frame_h.unwrap_or(oh)),
            Some(false) => (a.frame_w.unwrap_or(ow), full.1),
            None => (ow, oh),
        }
    }

    /// How many frames the strip holds.
    pub(crate) fn anim_length(&mut self, o: ObjId) -> i32 {
        let Some(full) = self.anim_full_size(o) else {
            return 0;
        };
        let (fw, fh) = self.anim_frame_size(o);
        match self.anim_vertical(o) {
            Some(true) => full.1 / fh.max(1),
            Some(false) => full.0 / fw.max(1),
            None => 1,
        }
    }

    /// Shows frame `n`, and scripts hear `onFrame`. Going to the frame it already shows asks for no new picture.
    pub(crate) fn anim_goto_frame(&mut self, o: ObjId, n: i32) {
        let moved = n != self.anim(o).1.frame;
        self.anim_mut(o).frame = n;
        if moved {
            self.anim_frame_changed(o);
        }
        self.dispatch(Handle::Gui(o), "onframe", &[Value::Int(n)]);
    }

    /// Asks for a frame for a new picture of the strip: only while it shows, and with its
    /// window's shape worked out again only when the strip is part of it.
    fn anim_frame_changed(&mut self, o: ObjId) {
        if self.region_op(o) != 0 {
            self.invalidate();
        } else if self.shows(o) {
            self.repaint();
        }
    }

    pub(crate) fn anim_play(&mut self, o: ObjId) {
        let length = self.anim_length(o);
        let a = self.anim_mut(o);
        let end = a.end_frame.unwrap_or(length - 1);
        a.step = if a.start_frame < end { 1 } else { -1 };
        a.paused = false;
        let start = a.start_frame;
        let auto_replay = a.auto_replay;
        self.anim_goto_frame(o, start);
        self.dispatch(Handle::Gui(o), "onplay", &[]);
        if start == end && !auto_replay {
            self.anim_stop(o);
            return;
        }
        let now = self.now();
        let a = self.anim_mut(o);
        a.playing = true;
        a.next_at = now + a.speed as i64;
        if !self.animations.contains(&o) {
            self.animations.push(o);
        }
    }

    pub(crate) fn anim_stop(&mut self, o: ObjId) {
        if self.anim(o).1.playing {
            self.anim_mut(o).playing = false;
            self.animations.retain(|a| *a != o);
        }
        self.dispatch(Handle::Gui(o), "onstop", &[]);
    }

    /// One step of a playing strip; called by the runtime when `next_at` is due.
    pub(crate) fn anim_advance(&mut self, o: ObjId) {
        if self.anim(o).1.paused {
            return;
        }
        let length = self.anim_length(o);
        let a = self.anim(o).1;
        let end = a.end_frame.unwrap_or(length - 1);
        if a.frame == end {
            if !a.auto_replay {
                self.anim_stop(o);
                return;
            }
            // back to the start without onFrame; a strip that starts where it ends asks for no new picture
            let moved = a.frame != a.start_frame;
            let start = a.start_frame;
            self.anim_mut(o).frame = start;
            if moved {
                self.anim_frame_changed(o);
            }
        } else {
            let next = a.frame + a.step;
            self.anim_goto_frame(o, next);
        }
        // on the beat, catching up ticks that came late
        let now = self.now();
        let a = self.anim_mut(o);
        let speed = a.speed.max(1) as i64;
        a.next_at += speed;
        if a.next_at < now - speed * 8 {
            a.next_at = now;
        }
    }

    pub(crate) fn anim_start_if_auto(&mut self, o: ObjId) {
        let a = self.anim(o).1;
        if a.auto_play && !a.playing {
            self.anim_play(o);
        }
    }
}
