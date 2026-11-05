// SPDX-License-Identifier: GPL-3.0-or-later

//! The tick: timers, animations, target slides, what the player changed, and the objects that
//! move with the clock.

use super::{switches_of, SkinRuntime, EQ_FLAT};
use crate::gui::{Kind, ObjId};
use crate::maki::Value;
use crate::player::Status;

impl SkinRuntime {
    /// Whether a target slide runs now.
    pub fn sliding(&self) -> bool {
        !self.moving.is_empty()
    }

    /// Whether something moves by itself right now: a target slide, or an animation playing and not paused.
    pub fn animating(&self) -> bool {
        !self.moving.is_empty() || self.animations.iter().any(|a| self.anim_running(*a))
    }

    fn anim_running(&self, o: ObjId) -> bool {
        match &self.objects[o].kind {
            Kind::AnimatedLayer(_, a) => a.playing && !a.paused,
            _ => false,
        }
    }

    /// Whether something on the skin moves smoothly by itself now and wants a frame on every
    /// display frame.
    pub fn frames_wanted(&mut self) -> bool {
        if !self.moving.is_empty() {
            return true;
        }
        let status = self.player.status();
        for v in self.vis_objects.clone() {
            if self.vis_moving(v, status) && self.shows(v) {
                return true;
            }
        }
        if status == Status::Stopped {
            return false;
        }
        let anims = self.animations.clone();
        anims
            .into_iter()
            .any(|a| self.anim_running(a) && self.shows(a))
    }

    /// When, in clock milliseconds, something on the skin next changes by itself; `None` when nothing will.
    pub fn next_due_at(&mut self) -> Option<i64> {
        let now = self.now();
        if !self.moving.is_empty() {
            return Some(now);
        }
        let status = self.player.status();
        let mut due = i64::MAX;
        for t in self.timers.clone() {
            if let Some(next) = self.timer_next_at(t) {
                due = due.min(next);
            }
        }
        for a in self.animations.clone() {
            if self.anim_running(a) {
                if let Kind::AnimatedLayer(_, d) = &self.objects[a].kind {
                    due = due.min(d.next_at);
                }
            }
        }
        for t in self.texts.clone() {
            if self.shows(t) {
                if let Some(d) = self.text_next_due_at(t, now, status) {
                    due = due.min(d);
                }
            }
        }
        for v in self.vis_objects.clone() {
            if self.vis_moving(v, status) && self.shows(v) {
                due = due.min(self.vis_next_step_at(v, now));
            }
        }
        if status == Status::Playing {
            for g in self.progress_grids.clone() {
                if self.shows(g) {
                    if let Some(d) = self.progress_next_fill_at(g, now) {
                        due = due.min(d);
                    }
                }
            }
        }
        if due == i64::MAX {
            None
        } else {
            Some(due)
        }
    }

    /// `tick` with the next target slide placed `slide_lead` ms ahead of now.
    pub fn tick_with_lead(&mut self, slide_lead: i64) -> bool {
        self.slide_lead = slide_lead;
        self.tick()
    }

    /// Advances timers, tickers and player-driven objects. Returns true when a redraw is due.
    pub fn tick(&mut self) -> bool {
        let t = self.now();
        self.tick_changed.clear();
        self.advance_clocks(t);
        let status = self.player.status();
        self.announce_player(status);
        if self.needs_layout {
            self.do_layout();
        }
        for o in self.sliders.clone() {
            self.slider_sync(o);
        }
        // a text out of sight still keeps its ticker and its message up to date
        for o in self.texts.clone() {
            if self.text_tick(o, t) && self.shows(o) {
                self.moved(o);
            }
        }
        for o in self.vis_objects.clone() {
            if self.vis_moving(o, status) && self.vis_step_due(o, t) && self.shows(o) {
                self.moved(o);
            }
        }
        for o in self.progress_grids.clone() {
            if self.progress_filled_changed(o) && self.shows(o) {
                self.moved(o);
            }
        }
        for o in self.buttons.clone() {
            if self.button_state_changed(o) && self.shows(o) {
                self.dirty = true;
            }
        }
        if self.needs_layout {
            self.do_layout();
        }
        self.dirty
    }

    fn moved(&mut self, o: ObjId) {
        self.dirty = true;
        self.tick_changed.push(o);
    }

    /// Refills every script's step budget, then moves animations and target slides and fires
    /// due timers, at `t`.
    fn advance_clocks(&mut self, t: i64) {
        for s in &mut self.scripts {
            s.refill();
        }
        let due_anims: Vec<ObjId> = self
            .animations
            .iter()
            .copied()
            .filter(|a| match &self.objects[*a].kind {
                Kind::AnimatedLayer(_, d) => d.playing && t >= d.next_at,
                _ => false,
            })
            .collect();
        for a in due_anims {
            self.anim_advance(a);
        }
        let due_slides = self.moving.clone();
        for o in due_slides {
            self.step_target(o, t + self.slide_lead);
        }
        self.fire_timers(t);
    }

    /// Moves `o` one frame toward its target; at the end the object sits there and hears `onTargetReached`.
    pub(crate) fn step_target(&mut self, o: ObjId, now: i64) {
        let b = &self.objects[o].base;
        let (Some(start), Some(from), Some(to)) = (b.move_start, b.move_from, b.move_to) else {
            return;
        };
        let f = if b.move_millis <= 0 {
            1.0
        } else {
            ((now - start) as f64 / b.move_millis as f64).clamp(0.0, 1.0)
        };
        // slow start, slow end: Winamp 5.666's config drawer follows (1 - cos(pi * t)) / 2 (recorded at 60 fps)
        let eased = (1.0 - (std::f64::consts::PI * f).cos()) / 2.0;
        let lerp = |i: usize| {
            to[i].map(|t| (from[i] as f64 + (t - from[i]) as f64 * eased).round() as i32)
        };
        let place = [b.x, b.y, b.w, b.h];
        let was_zero = b.alpha() == 0;
        let paint = b.paint_key();
        let b = &mut self.objects[o].base;
        if let Some(v) = lerp(0) {
            b.x = Some(v);
        }
        if let Some(v) = lerp(1) {
            b.y = Some(v);
        }
        if let Some(v) = lerp(2) {
            b.w = Some(v);
        }
        if let Some(v) = lerp(3) {
            b.h = Some(v);
        }
        if let Some(v) = lerp(4) {
            b.set_alpha(v.clamp(0, 255));
        }
        // a fade only repaints; the windows are laid out again only when something moved or resized
        if [b.x, b.y, b.w, b.h] != place {
            self.relayout();
        } else if self.objects[o].base.paint_key() != paint {
            self.alpha_changed(o, was_zero);
        }
        if f >= 1.0 {
            self.objects[o].base.move_start = None;
            self.moving.retain(|m| *m != o);
            self.dispatch(crate::maki::Handle::Gui(o), "ontargetreached", &[]);
        }
    }

    /// Starts `o`'s slide to the targets set so far.
    pub(crate) fn go_to_target(&mut self, o: ObjId) {
        let now = self.now();
        let b = &mut self.objects[o].base;
        b.move_from = Some([
            b.x.unwrap_or(0),
            b.y.unwrap_or(0),
            b.w.unwrap_or(b.rect.width),
            b.h.unwrap_or(b.rect.height),
            b.alpha(),
        ]);
        b.move_to = Some(b.target);
        b.target = [None; 5];
        b.move_millis = (b.target_seconds * 1000.0) as i64;
        b.move_start = Some(now);
        if !self.moving.contains(&o) {
            self.moving.push(o);
        }
        self.step_target(o, now);
    }

    /// Ends `o`'s slide where it is.
    pub(crate) fn cancel_target(&mut self, o: ObjId) {
        self.objects[o].base.move_start = None;
        self.moving.retain(|m| *m != o);
    }

    /// Tells the scripts what changed in the player since the last tick, as System events.
    fn announce_player(&mut self, status: Status) {
        if status != self.last_status {
            match status {
                Status::Playing => {
                    let event = if self.last_status == Status::Paused {
                        "onresume"
                    } else {
                        "onplay"
                    };
                    self.dispatch_system(event, &[]);
                    // a start announces the song: Winamp fires onTitleChange when playback begins, and
                    // Bento's file info lays its lines out on that
                    if self.last_status == Status::Stopped {
                        let title = self.player.title();
                        self.dispatch_system("ontitlechange", &[Value::string(title)]);
                    }
                }
                Status::Paused => self.dispatch_system("onpause", &[]),
                Status::Stopped => self.dispatch_system("onstop", &[]),
            }
            self.last_status = status;
            self.dirty = true;
        }
        let title = self.player.title();
        if title != self.last_title {
            self.last_title = title.clone();
            self.dispatch_system("ontitlechange", &[Value::string(title)]);
            self.dirty = true;
        }
        let eq_on = self.player.eq_enabled();
        if eq_on != self.last_eq_on {
            self.last_eq_on = eq_on;
            self.dispatch_system("oneqchanged", &[Value::Int(eq_on as i32)]);
            self.dirty = true;
        }
        let preamp = self.player.eq_preamp();
        if preamp != self.last_preamp {
            self.last_preamp = preamp;
            self.dispatch_system(
                "oneqpreampchanged",
                &[Value::Int((preamp - EQ_FLAT).clamp(-127, 127))],
            );
            self.dirty = true;
        }
        let bands = self.player.eq_bands();
        for i in 0..self.last_bands.len() {
            let v = bands.get(i).copied().unwrap_or(EQ_FLAT);
            if v != self.last_bands[i] {
                self.last_bands[i] = v;
                // the band and its value on getEqBand's scale, -127..127
                self.dispatch_system(
                    "oneqbandchanged",
                    &[
                        Value::Int(i as i32),
                        Value::Int((v - EQ_FLAT).clamp(-127, 127)),
                    ],
                );
                self.dirty = true;
            }
        }
        let volume = self.player.volume();
        if volume != self.last_volume {
            self.last_volume = volume;
            self.dispatch_system("onvolumechanged", &[Value::Int(volume)]);
            self.dirty = true;
        }
        // the buttons and config values that show shuffle, repeat and crossfade
        let switches = switches_of(&*self.player);
        if switches != self.last_switches {
            self.last_switches = switches;
            self.dirty = true;
        }
        // another list with the same rows shows the same (a player may build the list on every read)
        let tracks = self.player.tracks();
        if tracks != self.last_tracks {
            self.dirty = true;
            self.last_tracks = tracks;
        }
        let index = self.player.track_index();
        if index != self.last_track_index {
            self.last_track_index = index;
            self.dirty = true;
        }
    }
}
