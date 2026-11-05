// SPDX-License-Identifier: GPL-3.0-or-later

//! One loaded script: its program, its variable cells and the bindings that turn player events
//! into runs of the bytecode. The runtime runs the bytecode (`runtime::vm`); this holds what a
//! script keeps between runs and what can be read from it without running it.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::gui::ObjId;
use crate::maki::program::{op, Program};
use crate::maki::stdapi::{self, ApiMethod};
use crate::maki::{Cell, Handle, Kind, Value};

/// Steps a script may run per frame, across all its handlers.
pub const STEPS_PER_FRAME: i64 = 4_000_000;
const MAX_FAILURES: i32 = 20;
const MAX_MEMBERS: usize = 65_536;
const RIGHT_BUTTON: [&str; 2] = ["onrightbuttondown", "onrightbuttonup"];
const RIGHT_BUTTON_AND_CLICK: [&str; 3] = ["onrightbuttondown", "onrightbuttonup", "onrightclick"];
/// The PopupMenu calls that show a menu.
const MENU_CALLS: [&str; 2] = ["popatmouse", "popatxy"];
/// The System calls that read where the mouse is.
const MOUSE_CALLS: [&str; 2] = ["getmouseposx", "getmouseposy"];

/// The events a right click fires: the right button going down and up, and on a Button also onRightClick.
pub fn right_click_events(is_button: bool) -> Vec<&'static str> {
    if is_button {
        RIGHT_BUTTON_AND_CLICK.to_vec()
    } else {
        RIGHT_BUTTON.to_vec()
    }
}

/// What a handler's code can reach: a popup menu, the events it fires by calling them, and where the mouse is.
#[derive(Debug, Clone, Default)]
struct Reach {
    pops_menu: bool,
    calls: HashSet<String>,
    reads_mouse: bool,
}

/// One loaded script.
#[derive(Debug)]
pub struct Script {
    /// The program.
    pub program: Rc<Program>,
    /// The variable cells, one per declaration.
    pub cells: Vec<Cell>,
    /// Member variables of each class variable (`Class Layer Mains; Global Mains a, b;`).
    class_members: HashMap<usize, Vec<usize>>,
    /// Binding indices by event name.
    bindings_by_event: HashMap<String, Vec<usize>>,
    /// The `std.mi` entry of each method, read the first time a call needs it.
    apis: Vec<Option<Option<ApiMethod>>>,
    /// Steps the script may still run before the next refill.
    pub budget: i64,
    /// A script that failed too often is switched off: its handlers no longer run.
    pub disabled: bool,
    failures: i32,
    /// What the last handler this script ran returned.
    pub last_return: Value,
    /// The member variables this script keeps on objects (`EQ_p12.setTo`), by object and lowercase name.
    members: HashMap<(Handle, String), Cell>,
    member_count: usize,
    /// Instructions the bytecode walk may still read in this script.
    walk_budget: i64,
    walk_cut: bool,
    reaches: HashMap<usize, Reach>,
    /// A line the walk wants logged, for the runtime to take.
    pub pending_log: Option<String>,
}

impl Script {
    /// A script of `program` with its cells filled: constants, `system` for System variables,
    /// what `make` gives for the other predeclared globals, else the declared initial value.
    pub fn new(
        program: Rc<Program>,
        system: Handle,
        mut make: impl FnMut(&str) -> Option<Handle>,
    ) -> Self {
        let cells = program
            .variables
            .iter()
            .map(|decl| {
                let initial = if let Some(c) = &decl.constant {
                    c.clone()
                } else if let (Some(guid), None) = (&decl.guid, decl.class_var) {
                    if stdapi::is_a(guid, "System") {
                        Value::Obj(system)
                    } else if decl.system {
                        Value::of(make(guid))
                    } else {
                        decl.initial.clone()
                    }
                } else {
                    decl.initial.clone()
                };
                Cell::new(decl.kind, initial)
            })
            .collect();
        let mut class_members: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, v) in program.variables.iter().enumerate() {
            if let Some(cv) = v.class_var {
                class_members.entry(cv).or_default().push(i);
            }
        }
        let mut bindings_by_event: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, b) in program.bindings.iter().enumerate() {
            if let Some(m) = program.methods.get(b.method) {
                bindings_by_event.entry(m.name.clone()).or_default().push(i);
            }
        }
        let apis = vec![None; program.methods.len()];
        Script {
            program,
            cells,
            class_members,
            bindings_by_event,
            apis,
            budget: STEPS_PER_FRAME,
            disabled: false,
            failures: 0,
            last_return: Value::Null,
            members: HashMap::new(),
            member_count: 0,
            walk_budget: STEPS_PER_FRAME,
            walk_cut: false,
            reaches: HashMap::new(),
            pending_log: None,
        }
    }

    /// Whether this script listens to `event` at all: none once it is disabled.
    pub fn hears(&self, event: &str) -> bool {
        !self.disabled && self.bindings_by_event.contains_key(event)
    }

    /// The events this script listens to.
    pub fn events(&self) -> Vec<String> {
        if self.disabled {
            Vec::new()
        } else {
            self.bindings_by_event.keys().cloned().collect()
        }
    }

    /// Whether binding `b` is bound to `target`: a variable holding it, or a class it belongs to.
    fn binds(&self, b: usize, target: Handle) -> bool {
        let var = self.program.bindings[b].variable;
        self.cells[var].value.as_object() == Some(target)
            || self.class_members.get(&var).is_some_and(|ms| {
                ms.iter()
                    .any(|m| self.cells[*m].value.as_object() == Some(target))
            })
    }

    /// Whether a handler here is bound to `target`.
    pub fn listens_to(&self, target: Handle) -> bool {
        (0..self.program.bindings.len()).any(|b| self.binds(b, target))
    }

    /// Whether a handler here is bound to `target` for one of `events`.
    pub fn listens_to_any(&self, target: Handle, events: &[&str]) -> bool {
        (0..self.program.bindings.len()).any(|b| {
            let name = &self.program.methods[self.program.bindings[b].method].name;
            events.contains(&name.as_str()) && self.binds(b, target)
        })
    }

    /// The bindings of `event`, with the variable each one is on and where its handler starts.
    pub fn handlers(&self, event: &str, target: Handle) -> Vec<(usize, usize, bool)> {
        let Some(bs) = self.bindings_by_event.get(event) else {
            return Vec::new();
        };
        bs.iter()
            .filter_map(|b| {
                let binding = &self.program.bindings[*b];
                let held = self.cells[binding.variable].value.as_object();
                if held == Some(target) {
                    Some((binding.variable, binding.code_index, false))
                } else if self.class_members.get(&binding.variable).is_some_and(|ms| {
                    ms.iter()
                        .any(|m| self.cells[*m].value.as_object() == Some(target))
                }) {
                    // a handler on a class variable: it sees the member that fired as itself
                    Some((binding.variable, binding.code_index, true))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Whether a handler here for one of `events` bound to `target` can reach a popup menu, read from the bytecode.
    pub fn reaches_menu(&mut self, target: Handle, events: &[&str]) -> bool {
        if self.disabled {
            return false;
        }
        self.handler_starts(target, events)
            .into_iter()
            .any(|s| self.reach(s).pops_menu)
    }

    /// The events the handlers here for `events` bound to `target` fire by calling them.
    pub fn events_called(&mut self, target: Handle, events: &[&str]) -> Vec<String> {
        if self.disabled {
            return Vec::new();
        }
        let mut out = Vec::new();
        for s in self.handler_starts(target, events) {
            for c in self.reach(s).calls.iter() {
                if !out.contains(c) {
                    out.push(c.clone());
                }
            }
        }
        out
    }

    /// Whether a handler here for one of `events` bound to `target` can read where the mouse is.
    pub fn reads_mouse(&mut self, target: Handle, events: &[&str]) -> bool {
        if self.disabled {
            return false;
        }
        self.handler_starts(target, events)
            .into_iter()
            .any(|s| self.reach(s).reads_mouse)
    }

    /// Where the handlers here for `events` bound to `target` start in the code.
    fn handler_starts(&self, target: Handle, events: &[&str]) -> Vec<usize> {
        let mut out = Vec::new();
        for e in events {
            if let Some(bs) = self.bindings_by_event.get(*e) {
                for b in bs {
                    if self.binds(*b, target) {
                        out.push(self.program.bindings[*b].code_index);
                    }
                }
            }
        }
        out
    }

    fn reach(&mut self, start: usize) -> Reach {
        if let Some(r) = self.reaches.get(&start) {
            return r.clone();
        }
        let r = self.reach_from(start);
        self.reaches.insert(start, r.clone());
        r
    }

    /// Walks every instruction a run from `start` could reach, without running any: the next
    /// one, the targets of jumps, and a global call's function and the instruction it returns
    /// to, each path up to its RETURN. A popup menu counts wherever it is reached, under any
    /// condition. A walk the budget cuts short keeps what it found.
    fn reach_from(&mut self, start: usize) -> Reach {
        let program = self.program.clone();
        let code = &program.code;
        let mut seen = vec![false; code.len()];
        let mut order: Vec<usize> = Vec::new();
        let mut reach = Reach::default();
        let visit = |ip: usize, seen: &mut Vec<bool>, order: &mut Vec<usize>| {
            if ip < code.len() && !seen[ip] {
                seen[ip] = true;
                order.push(ip);
            }
        };
        visit(start, &mut seen, &mut order);
        let mut next = 0;
        while next < order.len() && self.spend_walk() {
            let ip = order[next];
            next += 1;
            let o = code[ip];
            match o.opcode {
                op::RETURN => {}
                op::JUMP => visit(o.arg as usize, &mut seen, &mut order),
                op::JUMP_IF | op::JUMP_IF_NOT | op::CALL_GLOBAL => {
                    visit(o.arg as usize, &mut seen, &mut order);
                    visit(ip + 1, &mut seen, &mut order);
                }
                op::CALL | op::CALL2 => {
                    let name = program
                        .methods
                        .get(o.arg as usize)
                        .map(|m| m.name.as_str())
                        .unwrap_or("");
                    if MENU_CALLS.contains(&name) {
                        reach.pops_menu = true;
                    }
                    if MOUSE_CALLS.contains(&name) {
                        reach.reads_mouse = true;
                    }
                    if name.starts_with("on") {
                        reach.calls.insert(name.to_string());
                    } else if name == "leftclick" {
                        // a Button's leftClick() fires onLeftClick; its rightClick() fires
                        // onRightClick, which a right click on a Button counts already
                        reach.calls.insert("onleftclick".to_string());
                    }
                    visit(ip + 1, &mut seen, &mut order);
                }
                _ => visit(ip + 1, &mut seen, &mut order),
            }
        }
        reach
    }

    /// Takes one instruction from the walk budget; false when none is left, which the log says once.
    fn spend_walk(&mut self) -> bool {
        if self.walk_budget > 0 {
            self.walk_budget -= 1;
            return true;
        }
        if !self.walk_cut {
            self.walk_cut = true;
            self.pending_log = Some(format!(
                "{}: right-click menus read up to the limit, the rest not offered",
                self.program.id
            ));
        }
        false
    }

    /// Refills the step budget, at each tick.
    pub fn refill(&mut self) {
        self.budget = STEPS_PER_FRAME;
    }

    /// Steps run since the last refill.
    pub fn steps_since_refill(&self) -> i64 {
        STEPS_PER_FRAME - self.budget
    }

    /// The `std.mi` entry of method `index`, read the first time a call needs it.
    pub fn api(&mut self, index: usize) -> Option<ApiMethod> {
        if let Some(Some(a)) = self.apis.get(index) {
            return *a;
        }
        let m = &self.program.methods[index];
        let a = stdapi::method(&self.program.classes[m.class_index], &m.name);
        if index < self.apis.len() {
            self.apis[index] = Some(a);
        }
        a
    }

    /// Notes a failure; true when the script is now switched off.
    pub fn failed(&mut self) -> bool {
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            self.disabled = true;
        }
        self.disabled
    }

    /// How many times it failed.
    pub fn failures(&self) -> i32 {
        self.failures
    }

    /// The slot of member `name` on `owner`, made the first time as a `kind` holding that
    /// kind's zero. `Err` when the script keeps too many members.
    pub fn member(
        &mut self,
        owner: Handle,
        name: &str,
        kind: Kind,
    ) -> Result<(Handle, String), String> {
        let key = (owner, name.to_lowercase());
        if !self.members.contains_key(&key) {
            self.member_count += 1;
            if self.member_count > MAX_MEMBERS {
                return Err(format!("more than {MAX_MEMBERS} member variables"));
            }
            // assigning null gives a cell its kind's zero: 0, 0.0, false, "" or null
            let mut cell = Cell::new(kind, Value::Null);
            cell.assign(Value::Null);
            self.members.insert(key.clone(), cell);
        }
        Ok(key)
    }

    /// The member cell at `key`, made by [`Script::member`].
    pub fn member_cell(&mut self, key: &(Handle, String)) -> &mut Cell {
        self.members.get_mut(key).expect("a member made before")
    }

    /// Drops the member variables kept on a deleted object.
    pub fn forget(&mut self, owner: Handle) {
        let before = self.members.len();
        self.members.retain(|(o, _), _| *o != owner);
        self.member_count -= before - self.members.len();
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
