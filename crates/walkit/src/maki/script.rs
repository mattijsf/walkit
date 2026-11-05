// SPDX-License-Identifier: GPL-3.0-or-later

//! One loaded script and the System object it talks to. The VM that runs the bytecode is a
//! later part of the port; until then a script loads and listens to nothing.

use std::collections::HashSet;

use crate::gui::ObjId;
use crate::maki::Handle;

const RIGHT_BUTTON: [&str; 2] = ["onrightbuttondown", "onrightbuttonup"];
const RIGHT_BUTTON_AND_CLICK: [&str; 3] = ["onrightbuttondown", "onrightbuttonup", "onrightclick"];

/// The events a right click fires: the right button going down and up, and on a Button also onRightClick.
pub fn right_click_events(is_button: bool) -> Vec<&'static str> {
    if is_button {
        RIGHT_BUTTON_AND_CLICK.to_vec()
    } else {
        RIGHT_BUTTON.to_vec()
    }
}

/// One loaded script: its program, its variable cells and the bindings that turn player events
/// into runs of the bytecode.
#[derive(Debug, Default)]
pub struct Script {
    events: HashSet<String>,
    budget_used: i64,
}

impl Script {
    /// The events this script listens to, for a quick check before dispatching.
    pub fn events(&self) -> &HashSet<String> {
        &self.events
    }

    /// Whether a handler here is bound to `target`.
    pub fn listens_to(&self, _target: Handle) -> bool {
        false
    }

    /// Whether a handler here is bound to `target` for one of `events`.
    pub fn listens_to_any(&self, _target: Handle, _events: &[&str]) -> bool {
        false
    }

    /// Whether a handler here for one of `events` bound to `target` can read where the mouse is.
    pub fn reads_mouse(&self, _target: Handle, _events: &[&str]) -> bool {
        false
    }

    /// Whether a handler here for one of `events` bound to `target` can reach a popup menu.
    pub fn reaches_menu(&self, _target: Handle, _events: &[&str]) -> bool {
        false
    }

    /// The events the handlers here for `events` bound to `target` fire by calling them.
    pub fn events_called(&self, _target: Handle, _events: &[&str]) -> Vec<String> {
        Vec::new()
    }

    /// Refills the step budget, at each tick.
    pub fn refill(&mut self) {
        self.budget_used = 0;
    }

    /// Steps run since the last refill.
    pub fn steps_since_refill(&self) -> i64 {
        self.budget_used
    }
}

/// The `System` object a script talks to: one per script instance, since `getScriptGroup()`
/// and `getParam()` differ per place. `started` once its onScriptLoaded ran: xui parameters
/// wait for that.
#[derive(Debug, Clone)]
pub struct SystemObject {
    /// The group the script lives in.
    pub group: Option<ObjId>,
    /// The script's `param`.
    pub param: String,
    /// The script file, for messages.
    pub script_id: String,
    /// Whether onScriptLoaded ran.
    pub started: bool,
}
