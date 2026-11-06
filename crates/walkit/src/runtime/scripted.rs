// SPDX-License-Identifier: GPL-3.0-or-later

//! What a host's tests do to a loaded skin the way its scripts would: call a method of an
//! object, a container, a layout or something made with `new`, as `std.mi` names them; make
//! such an object; reach a `Config` item; put an object on a slide; read the next timer; and
//! hear the events the engine fires. The host itself goes through the rest of the API; these
//! stand in for a skin's scripts where a test has none.

use crate::gui::ObjId;
use crate::host::WindowId;
use crate::maki::{stdapi, Handle, Value};

use super::SkinRuntime;

/// An event the engine fired while [`SkinRuntime::record_events`] was on: on what, which, with what.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedEvent {
    /// What the event was fired on.
    pub target: Handle,
    /// The event's name, lowercase, as scripts listen to it.
    pub event: String,
    /// The event's arguments, as the handlers get them.
    pub args: Vec<Value>,
}

impl SkinRuntime {
    /// The object with id `id` in the main window, as a script's `getObject` finds it.
    pub fn script_object(&self, id: &str) -> Option<Handle> {
        let main = self.main_layout()?;
        self.find_object(main, id).map(Handle::Gui)
    }

    /// The layout `window` shows, as a script holds a `Layout`.
    pub fn script_layout(&self, window: WindowId) -> Option<Handle> {
        let o = *self
            .layouts_by_id
            .get((window.0 as usize).wrapping_sub(1))?;
        Some(Handle::Gui(o))
    }

    /// The container with id `id`, as a script's `getContainer` finds it.
    pub fn script_container(&self, id: &str) -> Option<Handle> {
        self.container(id).map(Handle::Container)
    }

    /// An object of class `class` (Timer, PopupMenu, ...), as a script's `new` makes it.
    pub fn script_make(&mut self, class: &str) -> Option<Handle> {
        let guid = stdapi::guid_of(class)?;
        self.create(guid)
    }

    /// The `Config` item named `name` with `guid`, as `getItem` and `newItem` reach it.
    pub fn script_config_item(&mut self, name: &str, guid: &str) -> Option<Handle> {
        self.config_item(name, Some(guid)).map(Handle::Made)
    }

    /// Runs `method` (any case) on `receiver` with `args`, as a script's call does; `None` when
    /// the receiver has no such method.
    pub fn script_call(&mut self, receiver: Handle, method: &str, args: &[Value]) -> Option<Value> {
        self.maki_call(receiver, &method.to_lowercase(), args)
    }

    /// Puts object `o` on its way to a target, as `gotoTarget` does: the windows count as
    /// sliding until [`SkinRuntime::end_slides_as_script`].
    pub fn begin_slide_as_script(&mut self, o: Handle) {
        let Handle::Gui(o) = o else { return };
        let o: ObjId = o;
        if !self.moving.contains(&o) {
            self.moving.push(o);
        }
    }

    /// Ends every slide, as the slides reaching their targets do.
    pub fn end_slides_as_script(&mut self) {
        self.moving.clear();
    }

    /// When the next of the skin's timers fires, in the clock's ms; `None` when none runs.
    pub fn next_timer_at(&self) -> Option<i64> {
        self.timers
            .iter()
            .filter_map(|t| self.timer_next_at(*t))
            .min()
    }

    /// Starts or stops writing down every event the engine fires, for [`SkinRuntime::take_recorded_events`].
    pub fn record_events(&mut self, on: bool) {
        self.recorded = if on { Some(Vec::new()) } else { None };
    }

    /// The events fired since the last take, in order; empty while nothing is recorded.
    pub fn take_recorded_events(&mut self) -> Vec<RecordedEvent> {
        self.recorded
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    pub(crate) fn record(&mut self, target: Handle, event: &str, args: &[Value]) {
        if let Some(r) = &mut self.recorded {
            r.push(RecordedEvent {
                target,
                event: event.to_string(),
                args: args.to_vec(),
            });
        }
    }
}
