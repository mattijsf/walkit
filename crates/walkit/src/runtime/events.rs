// SPDX-License-Identifier: GPL-3.0-or-later
//! Change tracking and events: what asks for a frame or a layout pass, and how an event reaches
//! the scripts that listen.

use super::{Handle, ObjId, SkinRuntime, Value, MAX_EVENT_DEPTH};

impl SkinRuntime {
    /// Asks for a frame and a new window shape.
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.change_count += 1;
    }

    /// Asks for a frame for a change that moves, resizes, shows, hides and reshapes nothing.
    pub(crate) fn repaint(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn relayout(&mut self) {
        self.needs_layout = true;
        self.dirty = true;
        self.change_count += 1;
    }

    /// Whether the skin's scripts have started: what is set from here on is set by them.
    pub(crate) fn scripts_running(&self) -> bool {
        self.scripts_started
    }

    // ---- events ------------------------------------------------------------------------------

    /// Whether another event may fire inside the ones already firing.
    fn enter_event(&mut self, event: &str) -> bool {
        if self.event_depth >= MAX_EVENT_DEPTH {
            self.host.log(&format!(
                "event {event} not fired: events nested {} deep",
                self.event_depth
            ));
            return false;
        }
        self.event_depth += 1;
        self.firing.push(event.to_string());
        true
    }

    fn leave_event(&mut self) {
        self.event_depth -= 1;
        self.firing.pop();
    }

    /// The event firing right now, innermost.
    pub(crate) fn firing_now(&self) -> Option<&str> {
        self.firing.last().map(|s| s.as_str())
    }

    /// Fires `event` on `target` in every script; how many handlers ran.
    pub(crate) fn dispatch(&mut self, target: Handle, event: &str, args: &[Value]) -> i32 {
        if !self.enter_event(event) {
            return 0;
        }
        self.record(target, event, args);
        let mut n = 0;
        for s in 0..self.scripts.len() {
            if s < self.scripts.len() && self.scripts[s].hears(event) {
                n += self.script_dispatch(s, target, event, args).0;
            }
        }
        self.leave_event();
        // what happens to the child a xui group exposes happens to the group
        if let Handle::Gui(o) = target {
            if let Some(p) = self.xui_group_exposing(o) {
                n += self.dispatch(Handle::Gui(p), event, args);
            }
        }
        n
    }

    /// Fires `event` and gives back what the last handler that ran returned (Null when none ran).
    pub(crate) fn dispatch_for_result(
        &mut self,
        target: Handle,
        event: &str,
        args: &[Value],
    ) -> Value {
        if !self.enter_event(event) {
            return Value::Null;
        }
        self.record(target, event, args);
        let mut result = Value::Null;
        for s in 0..self.scripts.len() {
            if s < self.scripts.len() && self.scripts[s].hears(event) {
                let (n, last) = self.script_dispatch(s, target, event, args);
                if n > 0 {
                    result = last;
                }
            }
        }
        self.leave_event();
        if let Handle::Gui(o) = target {
            if let Some(p) = self.xui_group_exposing(o) {
                let r = self.dispatch_for_result(Handle::Gui(p), event, args);
                if r != Value::Null {
                    result = r;
                }
            }
        }
        result
    }

    /// The group that exposes `o` as its xui child, if any.
    fn xui_group_exposing(&self, o: ObjId) -> Option<ObjId> {
        let p = self.objects[o].base.parent?;
        let g = self.objects[p].group.as_ref()?;
        if g.embed_xui.as_deref() == Some(self.objects[o].base.id.as_str()) {
            Some(p)
        } else {
            None
        }
    }

    /// A script hears System events only once it has run onScriptLoaded (Wasabi SDK objcontroller).
    pub(crate) fn dispatch_system(&mut self, event: &str, args: &[Value]) {
        for s in 0..self.systems.len() {
            if self.systems[s].started {
                self.dispatch(Handle::System(s), event, args);
            }
        }
    }

    /// Runs a script's onScriptLoaded; its group's xui parameters follow at once.
    pub(crate) fn start_script(&mut self, s: usize) {
        self.systems[s].started = true;
        self.dispatch(Handle::System(s), "onscriptloaded", &[]);
        if let Some(g) = self.systems[s].group {
            self.deliver_xui_params(g);
        }
    }

    /// A xui element's parameters reach its scripts once they all ran onScriptLoaded.
    pub(crate) fn deliver_xui_params(&mut self, g: ObjId) {
        let Some(group) = self.objects[g].group.as_ref() else {
            return;
        };
        if group.xui_params.is_empty() {
            return;
        }
        // a default-set group the engine runs hears its parameters at once
        if group.behavior.is_some() {
            let params =
                std::mem::take(&mut self.objects[g].group.as_mut().expect("a group").xui_params);
            for (k, v) in params {
                self.behavior_xui_param(g, &k, &v);
            }
            return;
        }
        let systems = group.system_objects.clone();
        if systems.is_empty() || !systems.iter().all(|s| self.systems[*s].started) {
            return;
        }
        let params =
            std::mem::take(&mut self.objects[g].group.as_mut().expect("a group").xui_params);
        for (k, v) in params {
            for s in &systems {
                self.dispatch(
                    Handle::System(*s),
                    "onsetxuiparam",
                    &[Value::str(&k), Value::str(&v)],
                );
            }
        }
    }
}
