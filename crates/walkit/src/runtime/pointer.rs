// SPDX-License-Identifier: GPL-3.0-or-later

//! Touches: what a finger at a point of the stack means, presses, moves and releases, the
//! skin's menus, and the button actions and config values the objects drive.

use std::collections::HashMap;

use super::{SkinRuntime, HOST_WINDOWS};
use crate::draw::Rect;
use crate::gui::{Kind, ObjId};
use crate::host::SkinMenu;
use crate::maki::{Handle, Value};

/// Menus a script may pop in reply to one touch; more answer 0 without showing.
const MAX_MENUS_PER_GESTURE: i32 = 3;
/// The id of a wasabi title bar's group, and of the layer over the whole bar that takes its drags.
const TITLE_BAR: &str = "wasabi.titlebar";
const TITLE_BAR_TRAP: &str = "mousetrap";
/// The edges a resize grip's `resize` names.
const GRIP_EDGES: [&str; 4] = ["left", "right", "top", "bottom"];
/// The actions whose state `action_state` reads.
const STATE_ACTIONS: [&str; 3] = ["TOGGLE", "EQ_TOGGLE", "EQ_AUTO"];
/// The events of a press or a move; an object a script watches only for others takes no touch beside it.
const POINTER_EVENTS: [&str; 9] = [
    "onleftbuttondown",
    "onleftbuttonup",
    "onrightbuttondown",
    "onrightbuttonup",
    "onleftbuttondblclk",
    "onrightbuttondblclk",
    "onmousemove",
    "onleftclick",
    "onrightclick",
];
/// The events a tap fires on an object that is not a control: the left button going down and up.
const TAP_EVENTS: [&str; 2] = ["onleftbuttondown", "onleftbuttonup"];
/// The event of a finger moving while it presses.
const FOLLOW_EVENTS: [&str; 1] = ["onmousemove"];
/// The event of the left button going down.
const PRESS_EVENTS: [&str; 1] = ["onleftbuttondown"];

/// A drag on a resize grip: the window it resizes, the edges it moves, and where the finger
/// went down and the window's size then, in the window's pixels.
#[derive(Debug, Clone)]
struct Resize {
    window: ObjId,
    edges: String,
    x: i32,
    y: i32,
    size: (i32, i32),
}

/// A press its object has not heard yet: the object, and the point of the stack the finger went down on.
#[derive(Debug, Clone, Copy)]
struct DeferredPress {
    target: ObjId,
    sx: i32,
    sy: i32,
}

/// The object with a skin menu a touch landed on: the touch's seq, and the point in the object's window.
#[derive(Debug, Clone, Copy)]
struct MenuAim {
    seq: i32,
    target: ObjId,
    x: i32,
    y: i32,
}

/// What the pointer machinery keeps between calls.
#[derive(Debug, Default)]
pub(crate) struct PointerState {
    captured_origin: Rect,
    deferred: Option<DeferredPress>,
    resizing: Option<Resize>,
    menu_aim: Option<MenuAim>,
    move_seq: Option<i32>,
    aimed: Option<ObjId>,
    pub(crate) gesture_depth: i32,
    menus_this_gesture: i32,
    menus_ended: bool,
    pub(crate) picked_this_gesture: bool,
    pub(crate) press_ended: bool,
    entered: bool,
    cancelling: bool,
    menu_refusals_logged: Vec<String>,
    /// What `has_skin_menu` answered for each object since the last layout pass.
    pub(crate) skin_menus: HashMap<ObjId, bool>,
    skin_menu_reads: i64,
}

impl SkinRuntime {
    /// The window under a point of the stack, or the nearest within the touch slop.
    pub(crate) fn window_at(&mut self, x: i32, y: i32) -> Option<ObjId> {
        if self.needs_layout {
            self.do_layout();
        }
        if let Some(a) = self.pointer.aimed {
            return Some(a);
        }
        let windows = self.z_windows();
        // apart, a window takes a touch only where it docks: Winamp Modern's main window is
        // shaped around its closed drawer, and a touch there reaches the window under it
        if self.free_windows() {
            if let Some(w) = windows
                .iter()
                .rev()
                .find(|l| self.docking_rect(**l).contains(x, y))
            {
                return Some(*w);
            }
        }
        let distance2 = |r: Rect| -> i64 {
            let dx = (r.left - x).max(0).max(x - (r.right() - 1)) as i64;
            let dy = (r.top - y).max(0).max(y - (r.bottom() - 1)) as i64;
            dx * dx + dy * dy
        };
        if let Some(w) = windows
            .iter()
            .rev()
            .find(|l| self.window_rect_of(**l).contains(x, y))
        {
            return Some(*w);
        }
        let slop = self.touch_slop as i64;
        windows
            .iter()
            .copied()
            .min_by_key(|l| distance2(self.window_rect_of(*l)))
            .filter(|l| distance2(self.window_rect_of(*l)) <= slop * slop)
    }

    /// Runs `block` with the point `x`, `y` of `window` as a point of the stack, the touch kept to that window.
    fn in_window<T>(
        &mut self,
        window: crate::host::WindowId,
        x: i32,
        y: i32,
        block: impl FnOnce(&mut Self, i32, i32) -> T,
    ) -> T {
        let l = self.layouts_by_id_lookup(window);
        if self.needs_layout {
            self.do_layout();
        }
        let origin = self.window_rect_of(l);
        let before = self.pointer.aimed;
        self.pointer.aimed = Some(l);
        let r = block(self, origin.left + x, origin.top + y);
        self.pointer.aimed = before;
        r
    }

    fn layouts_by_id_lookup(&self, id: crate::host::WindowId) -> ObjId {
        self.layouts_by_id[(id.0 as usize).wrapping_sub(1)]
    }

    /// `pointer_down` at a point of `window`, in that window's pixels.
    pub fn pointer_down_in(&mut self, window: crate::host::WindowId, x: i32, y: i32, seq: i32) {
        self.in_window(window, x, y, |rt, sx, sy| rt.pointer_down(sx, sy, seq))
    }

    /// `pointer_move` at a point of `window`, in that window's pixels.
    pub fn pointer_move_in(&mut self, window: crate::host::WindowId, x: i32, y: i32) {
        self.in_window(window, x, y, |rt, sx, sy| rt.pointer_move(sx, sy))
    }

    /// `pointer_up` at a point of `window`, in that window's pixels.
    pub fn pointer_up_in(&mut self, window: crate::host::WindowId, x: i32, y: i32) {
        self.in_window(window, x, y, |rt, sx, sy| rt.pointer_up(sx, sy))
    }

    /// `double_tap` at a point of `window`, in that window's pixels.
    pub fn double_tap_in(&mut self, window: crate::host::WindowId, x: i32, y: i32, seq: i32) {
        self.in_window(window, x, y, |rt, sx, sy| rt.double_tap(sx, sy, seq))
    }

    /// The topmost object at a point of the stack.
    pub(crate) fn hit_test_stack(&mut self, x: i32, y: i32) -> Option<ObjId> {
        let l = self.window_at(x, y)?;
        let r = self.window_rect_of(l);
        self.hit_test(l, x - r.left, y - r.top)
    }

    /// Whether a touch on `o` does something: a control, or an object a script listens to.
    fn reacts(&self, o: ObjId) -> bool {
        self.is_control(o) || self.scripts.iter().any(|s| s.listens_to(Handle::Gui(o)))
    }

    fn is_control(&self, o: ObjId) -> bool {
        matches!(
            self.objects[o].kind,
            Kind::Button(_) | Kind::Slider(_) | Kind::ColorThemeList(_) | Kind::PlaylistView(_)
        ) || self.is_grip(o)
    }

    /// Whether `o` is a window's resize grip: its `resize`, as a script last set it, names an edge, and it lies in a window.
    fn is_grip(&self, o: ObjId) -> bool {
        let Some(edges) = self.objects[o]
            .base
            .attr("resize")
            .map(|e| e.to_lowercase())
        else {
            return false;
        };
        self.layout_object(o).is_some() && GRIP_EDGES.iter().any(|e| edges.contains(e))
    }

    /// Sizes the window as the finger at `x`, `y` drags its grip, as Winamp sizes it.
    fn drag_grip(&mut self, r: &Resize, x: i32, y: i32) {
        let l = r.window;
        let (mut w, mut h) = r.size;
        if r.edges.contains("right") {
            w += x - r.x;
        }
        if r.edges.contains("left") {
            w -= x - r.x;
        }
        if r.edges.contains("bottom") {
            h += y - r.y;
        }
        if r.edges.contains("top") {
            h -= y - r.y;
        }
        let (vw, vh) = self.viewport();
        let d = self.objects[l].layout().expect("a layout");
        let size = (
            within(w, d.min_w, d.max_w, vw),
            within(h, d.min_h, d.max_h, vh),
        );
        if Some(l) != self.main_layout() {
            self.objects[l].layout_mut().expect("a layout").user_size = Some(size);
        }
        if size != self.layout_size(l) {
            self.resize_layout_to(l, size.0, size.1);
        }
    }

    /// The grip drag ends: the window hears that the user resized it.
    fn end_grip(&mut self) {
        let Some(r) = self.pointer.resizing.take() else {
            return;
        };
        let l = r.window;
        let (w, h) = self.layout_size(l);
        let d = self.objects[l].layout().expect("a layout");
        let (vx, vy) = (d.view_x, d.view_y);
        self.dispatch(
            Handle::Gui(l),
            "onuserresize",
            &[Value::Int(vx), Value::Int(vy), Value::Int(w), Value::Int(h)],
        );
    }

    /// Whether a script hears a tap on `o`: one listens to it for the tap events, or to the group
    /// that exposes it as its xui child.
    fn hears_tap(&self, o: ObjId) -> bool {
        self.dispatch_chain(o).iter().any(|c| {
            self.scripts
                .iter()
                .any(|s| s.listens_to_any(Handle::Gui(*c), &TAP_EVENTS))
        })
    }

    /// Whether a script follows the finger across `o`, or across the group that exposes it.
    fn follows_finger(&mut self, o: ObjId) -> bool {
        let chain = self.dispatch_chain(o);
        let mut any = false;
        for c in &chain {
            for s in 0..self.scripts.len() {
                if self.scripts[s].listens_to_any(Handle::Gui(*c), &FOLLOW_EVENTS)
                    || self.scripts[s].reads_mouse(Handle::Gui(*c), &PRESS_EVENTS)
                {
                    any = true;
                }
            }
        }
        self.take_script_logs();
        any
    }

    /// Whether a mouse could click `o` in `window` at one of a few of its points.
    fn clickable(&mut self, o: ObjId, window: ObjId, x: i32, y: i32) -> bool {
        let r = self.objects[o].base.rect;
        let right = r.right() - 1;
        let bottom = r.bottom() - 1;
        let points = [
            (
                x.clamp(r.left, right.max(r.left)),
                y.clamp(r.top, bottom.max(r.top)),
            ),
            ((r.left + right) / 2, (r.top + bottom) / 2),
            (r.left, r.top),
            (right, r.top),
            (r.left, bottom),
            (right, bottom),
        ];
        points.iter().any(|(px, py)| {
            let mut at = self.hit_test(window, *px, *py);
            while let Some(h) = at {
                if h == o {
                    return true;
                }
                at = self.objects[h].base.parent;
            }
            false
        })
    }

    /// Squared distance from a point to `o`'s rectangle; 0 inside.
    fn distance2(&self, o: ObjId, x: i32, y: i32) -> i64 {
        let r = self.objects[o].base.rect;
        let dx = (r.left - x).max(0).max(x - (r.right() - 1)) as i64;
        let dy = (r.top - y).max(0).max(y - (r.bottom() - 1)) as i64;
        dx * dx + dy * dy
    }

    /// What a finger at a point means; see the Kotlin engine's `touchTarget` for the order.
    pub(crate) fn touch_target(&mut self, sx: i32, sy: i32) -> Option<ObjId> {
        let window = self.window_at(sx, sy)?;
        let origin = self.window_rect_of(window);
        let x = sx - origin.left;
        let y = sy - origin.top;
        let hit = self.hit_test(window, x, y);
        if let Some(h) = hit {
            if self.is_control(h) || matches!(self.objects[h].kind, Kind::Menu(_)) {
                return Some(h);
            }
        }
        let slop2 = (self.touch_slop as i64) * (self.touch_slop as i64);
        let objects = self.all_objects.clone();
        let nearest = |rt: &mut SkinRuntime,
                       shown: bool,
                       pick: &dyn Fn(&SkinRuntime, ObjId) -> bool|
         -> Option<ObjId> {
            let mut best: Option<(ObjId, i64)> = None;
            for o in objects.iter().rev().copied() {
                if rt.layout_object(o) != Some(window) || !rt.touchable(o, shown) {
                    continue;
                }
                let d = rt.distance2(o, x, y);
                if d > slop2 || !pick(rt, o) {
                    continue;
                }
                if shown && !rt.clickable(o, window, x, y) {
                    continue;
                }
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((o, d));
                }
            }
            best.map(|(o, _)| o)
        };
        let control = nearest(self, true, &|rt, o| rt.is_control(o));
        if let Some(h) = hit {
            if self.hears_tap(h)
                && !control.is_some_and(|c| {
                    self.objects[c]
                        .base
                        .rect
                        .overlaps(&self.objects[h].base.rect)
                })
            {
                return Some(h);
            }
        }
        if let Some(c) = control {
            return Some(c);
        }
        if let Some(h) = hit.filter(|h| self.reacts(*h)) {
            return Some(h);
        }
        if let Some(o) = nearest(self, false, &|rt, o| {
            !rt.is_control(o)
                && rt
                    .scripts
                    .iter()
                    .any(|s| s.listens_to_any(Handle::Gui(o), &POINTER_EVENTS))
        }) {
            return Some(o);
        }
        hit
    }

    /// Pointer events take points of the stack; objects hear points of their window. `seq`
    /// names this touch for the menu and move targets and `right_click`.
    pub fn pointer_down(&mut self, sx: i32, sy: i32, seq: i32) {
        self.gesture(true, |rt| {
            rt.aim_menu(sx, sy, seq);
            rt.pointer.move_seq = None;
            // apart, a press raises its window
            if rt.free_windows() {
                if let Some(w) = rt.window_at(sx, sy) {
                    rt.raise_window(w);
                }
            }
            let touched = rt.pointer_down_at(sx, sy);
            // a press on no object moves the window; a press that opened a menu has been let go of, and
            // moves nothing; a press held back from an object with a skin menu moves the window as a
            // press on that object would
            let held = rt.captured.or(rt.pointer.deferred.map(|d| d.target));
            if touched.is_none() || held.is_some_and(|h| rt.moves_window(h)) {
                rt.pointer.move_seq = Some(seq);
                // set out apart, the window under the finger moves with it
                if rt.free_windows() {
                    if let Some(w) = rt.window_at(sx, sy) {
                        rt.free_drag_begin(w, sx, sy);
                    }
                }
            }
        })
    }

    /// Presses what a finger at a point of the stack means, and returns it; `None` for nothing.
    fn pointer_down_at(&mut self, sx: i32, sy: i32) -> Option<ObjId> {
        self.drop_deferred();
        if self.needs_layout {
            self.do_layout();
        }
        let target = self.touch_target(sx, sy)?;
        // the touched window takes the focus, as a press of either mouse button gives it
        if let Some(l) = self.layout_object(target) {
            self.take_focus(l);
        }
        if self.pointer.menu_aim.is_some_and(|a| a.target == target)
            && !matches!(self.objects[target].kind, Kind::Slider(_))
        {
            self.pointer.deferred = Some(DeferredPress { target, sx, sy });
            self.show_pressed(target, true);
            return Some(target);
        }
        self.press(target, sx, sy);
        Some(target)
    }

    /// Shows a held press's button pressed, or no longer; its scripts hear nothing of it.
    fn show_pressed(&mut self, target: ObjId, pressed: bool) {
        if let Kind::Button(b) = &mut self.objects[target].kind {
            b.pressed = pressed;
            self.repaint();
        }
    }

    /// The object hears the button go down at a point of the stack, and the finger come in.
    fn press(&mut self, target: ObjId, sx: i32, sy: i32) {
        self.pointer.captured_origin = self
            .layout_object(target)
            .map(|l| self.window_rect_of(l))
            .unwrap_or(Rect::EMPTY);
        let x = sx - self.pointer.captured_origin.left;
        let y = sy - self.pointer.captured_origin.top;
        self.last_pointer = (x, y);
        self.captured = Some(target);
        if let Some(window) = self.layout_object(target) {
            if self.is_grip(target) {
                let edges = self.objects[target]
                    .base
                    .attr("resize")
                    .unwrap_or("")
                    .to_lowercase();
                let size = self.layout_size(window);
                self.pointer.resizing = Some(Resize {
                    window,
                    edges,
                    x,
                    y,
                    size,
                });
            }
        }
        self.object_pointer_down(target, x, y);
        // a menu the press opened has let go of it already
        if self.captured == Some(target) {
            self.pointer.entered = true;
            self.dispatch(Handle::Gui(target), "onenterarea", &[]);
        }
    }

    /// The finger moves to a point of the stack.
    pub fn pointer_move(&mut self, sx: i32, sy: i32) {
        self.gesture(false, |rt| rt.pointer_move_at(sx, sy))
    }

    /// Sends the deferred press where the finger went down, unless its object has gone from the skin.
    fn send_deferred(&mut self) {
        let Some(d) = self.pointer.deferred else {
            return;
        };
        self.drop_deferred();
        if self.needs_layout {
            self.do_layout();
        }
        if self.all_objects.contains(&d.target) {
            self.press(d.target, d.sx, d.sy);
        }
    }

    /// Lets the deferred press go unsent: its object hears nothing of it.
    fn drop_deferred(&mut self) {
        let Some(d) = self.pointer.deferred.take() else {
            return;
        };
        self.show_pressed(d.target, false);
    }

    fn pointer_move_at(&mut self, sx: i32, sy: i32) {
        if self.free_dragging() {
            self.free_drag_to(sx, sy);
        }
        if let Some(d) = self.pointer.deferred {
            let dx = sx - d.sx;
            let dy = sy - d.sy;
            // still a tap or a long press
            if dx * dx + dy * dy <= self.touch_slop * self.touch_slop {
                return;
            }
            self.send_deferred();
        }
        let x = sx - self.pointer.captured_origin.left;
        let y = sy - self.pointer.captured_origin.top;
        self.last_pointer = (x, y);
        if let Some(r) = self.pointer.resizing.clone() {
            self.drag_grip(&r, x, y);
        }
        if let Some(c) = self.captured {
            self.object_pointer_move(c, x, y);
        }
    }

    /// The finger lifts at a point of the stack.
    pub fn pointer_up(&mut self, sx: i32, sy: i32) {
        self.gesture(false, |rt| rt.pointer_up_at(sx, sy))
    }

    fn pointer_up_at(&mut self, sx: i32, sy: i32) {
        self.free_drag_end();
        self.send_deferred();
        let x = sx - self.pointer.captured_origin.left;
        let y = sy - self.pointer.captured_origin.top;
        self.last_pointer = (x, y);
        if let Some(r) = self.pointer.resizing.clone() {
            self.drag_grip(&r, x, y);
        }
        self.end_grip();
        let Some(target) = self.captured.take() else {
            return;
        };
        // a finger that drifts a little still counts as on the control
        let r = self.objects[target].base.rect;
        let slop = self.touch_slop;
        let inside = x >= r.left - slop
            && x < r.right() + slop
            && y >= r.top - slop
            && y < r.bottom() + slop;
        self.object_pointer_up(target, x, y, inside);
        self.leave_area(target);
    }

    /// A second tap of a double tap, at a point of the stack.
    pub fn double_tap(&mut self, sx: i32, sy: i32, seq: i32) {
        self.gesture(true, |rt| {
            rt.aim_menu(sx, sy, seq);
            rt.pointer.move_seq = None;
            rt.double_tap_at(sx, sy);
        })
    }

    fn double_tap_at(&mut self, sx: i32, sy: i32) {
        self.drop_deferred();
        let Some(target) = self.hit_test_stack(sx, sy) else {
            return;
        };
        let origin = self
            .layout_object(target)
            .map(|l| self.window_rect_of(l))
            .unwrap_or(Rect::EMPTY);
        self.object_double_tap(target, sx - origin.left, sy - origin.top);
    }

    /// The seq of the last press when it landed on an object with a skin menu, else `None`.
    pub fn menu_target(&self) -> Option<i32> {
        self.pointer.menu_aim.map(|a| a.seq)
    }

    /// The seq of the last press when it landed on no object, or the object it pressed moves the window.
    pub fn move_target(&self) -> Option<i32> {
        self.pointer.move_seq
    }

    /// Whether a drag that starts on `o` moves the window.
    pub(crate) fn moves_window(&mut self, o: ObjId) -> bool {
        if matches!(
            self.objects[o].kind,
            Kind::Slider(_) | Kind::ColorThemeList(_) | Kind::PlaylistView(_)
        ) {
            return false;
        }
        if self.is_grip(o) {
            return false;
        }
        if matches!(self.objects[o].kind, Kind::Button(_) | Kind::Menu(_)) {
            return !self.follows_finger(o);
        }
        self.is_move_area(o) || !self.follows_finger(o)
    }

    /// Whether Winamp moves the window by a drag on `o`: what `o` writes in `move`, else what the
    /// nearest group around it writes; a wasabi title bar's mousetrap moves it unless it says otherwise.
    fn is_move_area(&self, o: ObjId) -> bool {
        if let Some(m) = self.objects[o].base.move_window {
            return m;
        }
        let b = &self.objects[o].base;
        if b.id == TITLE_BAR_TRAP
            && b.parent
                .is_some_and(|p| self.objects[p].base.id == TITLE_BAR)
        {
            return true;
        }
        let mut p = b.parent;
        while let Some(g) = p {
            if let Some(m) = self.objects[g].base.move_window {
                return m;
            }
            p = self.objects[g].base.parent;
        }
        false
    }

    /// Remembers the object with a skin menu under a touch at a point of the stack, or none.
    fn aim_menu(&mut self, sx: i32, sy: i32, seq: i32) {
        if self.needs_layout {
            self.do_layout();
        }
        self.pointer.menu_aim = self.menu_aim_at(sx, sy, seq);
    }

    fn menu_aim_at(&mut self, sx: i32, sy: i32, seq: i32) -> Option<MenuAim> {
        let window = self.window_at(sx, sy)?;
        let origin = self.window_rect_of(window);
        let target = self.menu_object_at(window, sx - origin.left, sy - origin.top)?;
        Some(MenuAim {
            seq,
            target,
            x: sx - origin.left,
            y: sy - origin.top,
        })
    }

    /// The object a right click at a point of `window` means, among objects with a skin menu only.
    fn menu_object_at(&mut self, window: ObjId, x: i32, y: i32) -> Option<ObjId> {
        let slop2 = (self.touch_slop as i64) * (self.touch_slop as i64);
        let mut best: Option<(ObjId, i64)> = None;
        for o in self.all_objects.clone().into_iter().rev() {
            if self.layout_object(o) != Some(window)
                || self.objects[o].base.ghost
                || !self.touchable(o, true)
            {
                continue;
            }
            let d = self.distance2(o, x, y);
            if d > slop2 {
                continue;
            }
            let has = match self.pointer.skin_menus.get(&o) {
                Some(h) => *h,
                None => {
                    let h = self.has_skin_menu(o);
                    self.pointer.skin_menus.insert(o, h);
                    h
                }
            };
            if has && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((o, d));
            }
        }
        best.map(|(o, _)| o)
    }

    /// Whether a right click on `o` can open a menu of the skin's, read from the scripts'
    /// bytecode without running them.
    pub(crate) fn has_skin_menu(&mut self, o: ObjId) -> bool {
        self.pointer.skin_menu_reads += 1;
        let chain = self.dispatch_chain(o);
        let is_button = matches!(self.objects[o].kind, Kind::Button(_));
        let events = crate::maki::script::right_click_events(is_button);
        let mut reaches = false;
        for c in &chain {
            for s in 0..self.scripts.len() {
                if self.scripts[s].reaches_menu(Handle::Gui(*c), &events) {
                    reaches = true;
                }
            }
        }
        self.take_script_logs();
        if reaches {
            return true;
        }
        let mut called: Vec<String> = Vec::new();
        for c in &chain {
            for s in 0..self.scripts.len() {
                for e in self.scripts[s].events_called(Handle::Gui(*c), &events) {
                    if !called.contains(&e) {
                        called.push(e);
                    }
                }
            }
        }
        let called: Vec<&str> = called.iter().map(|s| s.as_str()).collect();
        let mut any = false;
        if !called.is_empty() {
            for c in &chain {
                for s in 0..self.scripts.len() {
                    if self.scripts[s].reaches_menu(Handle::Gui(*c), &called) {
                        any = true;
                    }
                }
            }
        }
        self.take_script_logs();
        any
    }

    /// Writes the lines the scripts' bytecode walks asked to log.
    fn take_script_logs(&mut self) {
        for s in 0..self.scripts.len() {
            if let Some(line) = self.scripts[s].pending_log.take() {
                self.host.log(&line);
            }
        }
    }

    /// The objects an event on `o` fires on: `o`, then each group that exposes the one before as its xui child.
    fn dispatch_chain(&self, o: ObjId) -> Vec<ObjId> {
        let mut chain = vec![o];
        let mut at = o;
        while let Some(p) = self.objects[at].base.parent {
            let exposes = self.objects[p]
                .group
                .as_ref()
                .is_some_and(|g| g.embed_xui.as_deref() == Some(self.objects[at].base.id.as_str()));
            if !exposes || chain.contains(&p) {
                break;
            }
            chain.push(p);
            at = p;
        }
        chain
    }

    /// A right click on the object the touch `seq` landed on, at the point it landed, as a separate touch.
    pub fn right_click(&mut self, seq: i32) {
        self.gesture(true, |rt| {
            if let Some(aim) = rt.pointer.menu_aim.filter(|a| a.seq == seq) {
                rt.right_click_at(aim);
            }
        })
    }

    fn right_click_at(&mut self, aim: MenuAim) {
        // the touch was a long press: a deferred press goes nowhere
        self.drop_deferred();
        if self.needs_layout {
            self.do_layout();
        }
        if !self.all_objects.contains(&aim.target) {
            return;
        }
        // a press still held lets go first, as a right click takes the mouse
        if self.captured.is_some() {
            self.end_press();
        }
        self.last_pointer = (aim.x, aim.y);
        self.object_right_click(aim.target, aim.x, aim.y);
        if matches!(self.objects[aim.target].kind, Kind::Button(_)) {
            self.dispatch(Handle::Gui(aim.target), "onrightclick", &[]);
        }
    }

    /// Runs `block` as a touch the listener made, from which a script may pop a menu. `fresh`
    /// starts a new touch, whose menu count starts over.
    pub fn gesture<T>(&mut self, fresh: bool, block: impl FnOnce(&mut Self) -> T) -> T {
        self.gesture_begin(fresh);
        let r = block(self);
        self.gesture_end();
        r
    }

    /// Starts a touch the listener made, as [`SkinRuntime::gesture`] does around its block, for
    /// a host across a boundary that cannot pass the block; [`SkinRuntime::gesture_end`] ends it.
    pub fn gesture_begin(&mut self, fresh: bool) {
        if fresh {
            self.pointer.menus_this_gesture = 0;
            self.pointer.menus_ended = false;
            self.pointer.picked_this_gesture = false;
            self.pointer.press_ended = false;
        }
        self.pointer.gesture_depth += 1;
    }

    /// Ends the touch [`SkinRuntime::gesture_begin`] started; one that was not started ends nothing.
    pub fn gesture_end(&mut self) {
        self.pointer.gesture_depth = self.pointer.gesture_depth.saturating_sub(1);
    }

    /// The listener's pick from a menu a script popped, by command id, or 0.
    pub fn menu_choice(&mut self, menu: &SkinMenu) -> i32 {
        let refusal = if self.pointer.gesture_depth == 0 {
            Some("outside a touch".to_string())
        } else if self.pointer.menus_ended {
            Some("after one closed without the listener".to_string())
        } else if self.pointer.cancelling {
            Some("while a press ends without a release".to_string())
        } else if self.pointer.menus_this_gesture >= MAX_MENUS_PER_GESTURE {
            Some(format!("more than {MAX_MENUS_PER_GESTURE} in one touch"))
        } else {
            None
        };
        if let Some(refusal) = refusal {
            if !self.pointer.menu_refusals_logged.contains(&refusal) {
                self.pointer.menu_refusals_logged.push(refusal.clone());
                self.host.log(&format!("popup menu {refusal}: not shown"));
            }
            return 0;
        }
        self.pointer.menus_this_gesture += 1;
        self.end_press();
        let picked = self.host.choose_from_menu(menu);
        if picked.is_none() {
            self.pointer.menus_ended = true;
        }
        if picked.unwrap_or(0) != 0 {
            self.pointer.picked_this_gesture = true;
        }
        picked.unwrap_or(0)
    }

    /// Ends the press under the finger without a release.
    pub fn pointer_cancel(&mut self) {
        self.gesture(false, |rt| rt.end_press())
    }

    /// Lets go of the pressed object without a release, and tells it the finger left.
    fn end_press(&mut self) {
        self.pointer.press_ended = true;
        self.drop_deferred();
        self.end_grip();
        let Some(held) = self.captured.take() else {
            return;
        };
        self.pointer.cancelling = true;
        self.object_pointer_cancel(held);
        self.pointer.cancelling = false;
        self.leave_area(held);
    }

    /// `onLeaveArea` for `o`, once per press that sent it `onEnterArea`.
    fn leave_area(&mut self, o: ObjId) {
        if !self.pointer.entered {
            return;
        }
        self.pointer.entered = false;
        self.dispatch(Handle::Gui(o), "onleavearea", &[]);
    }

    // ---- actions -----------------------------------------------------------------------------

    /// A button action, as Winamp does it.
    pub(crate) fn action(
        &mut self,
        source: ObjId,
        action: &str,
        param: Option<&str>,
        target: Option<&str>,
    ) {
        let a = action.to_uppercase();
        let source_id = self.objects[source].base.id.clone();
        self.host.log(&format!(
            "action {a} {} from {source_id}",
            param.unwrap_or("")
        ));
        match a.as_str() {
            "PLAY" => self.player.play(),
            "PAUSE" => self.player.pause(),
            "STOP" => self.player.stop(),
            "PREV" => self.player.previous(),
            "NEXT" => self.player.next(),
            "EJECT" => self.player.eject(),
            "VOLUME_UP" => {
                let v = (self.player.volume() + 10).min(255);
                self.player.set_volume(v);
            }
            "VOLUME_DOWN" => {
                let v = (self.player.volume() - 10).max(0);
                self.player.set_volume(v);
            }
            "TOGGLE" => {
                let Some(param) = param else { return };
                let id = param.to_lowercase();
                let id = id.strip_prefix("guid:").unwrap_or(&id).to_string();
                let kind = self.window_kind(&id);
                let answers = self.host_actions
                    && (HOST_WINDOWS.contains(&kind.as_str()) || self.toggle_target(&id).is_none());
                if answers {
                    self.host.host_action(&a, Some(&kind));
                    // the app shows it: the skin's frame for it (an empty library) stays shut
                    if HOST_WINDOWS.contains(&kind.as_str()) {
                        self.invalidate();
                        return;
                    }
                }
                self.toggle_window(&id, param);
                self.invalidate();
            }
            "SWITCH" => {
                if let (Some(p), Some(c)) = (
                    param,
                    self.layout_object(source)
                        .and_then(|l| self.objects[l].layout().and_then(|d| d.container)),
                ) {
                    self.switch_to(c, p);
                }
            }
            "CB_PREVPAGE" | "CB_PREV" | "CB_NEXTPAGE" | "CB_NEXT" => {
                if let Some(b) = self.bucket_for(source, target) {
                    self.bucket_page(b, if a.starts_with("CB_PREV") { -1 } else { 1 });
                }
            }
            // a docked window's close button puts it away, unless canclose="0"; the main window stays
            "CLOSE" => {
                let c = self
                    .layout_object(source)
                    .and_then(|l| self.objects[l].layout().and_then(|d| d.container));
                if c.is_some() && c == self.main_container() {
                    if self.host_actions {
                        self.host.host_action(&a, param);
                    }
                } else if let Some(c) = c.filter(|c| self.containers[*c].can_close()) {
                    self.close_container(c);
                }
            }
            "MINIMIZE" | "SYSMENU" | "CONTROLMENU" | "MENU" | "TRACKINFO" | "VIS_NEXT"
            | "VIS_PREV" | "VIS_MENU" | "VIS_FS" | "VID_FS" => {
                if self.host_actions {
                    self.host.host_action(&a, param);
                }
            }
            "EQ_TOGGLE" => {
                let on = !self.player.eq_enabled();
                self.player.set_eq_enabled(on);
            }
            "EQ_AUTO" => {
                let on = !self.config_flags.get("eq_auto").copied().unwrap_or(false);
                self.config_flags.insert("eq_auto".into(), on);
            }
            "TEXT_LARGER" | "TEXT_SMALLER" => {}
            "COLORTHEMES_SWITCH" | "COLORTHEMES_PREVIOUS" | "COLORTHEMES_NEXT" => {
                let parent = self.objects[source].base.parent;
                let named = target.and_then(|t| {
                    parent.and_then(|p| self.find_object(p, t)).or_else(|| {
                        let windows = self.windows();
                        windows.iter().find_map(|l| self.find_object(*l, t))
                    })
                });
                let list = named
                    .filter(|l| matches!(self.objects[*l].kind, Kind::ColorThemeList(_)))
                    .or_else(|| {
                        self.all_objects
                            .iter()
                            .copied()
                            .find(|o| matches!(self.objects[*o].kind, Kind::ColorThemeList(_)))
                    });
                let Some(list) = list else { return };
                match a.as_str() {
                    "COLORTHEMES_SWITCH" => self.themes_switch_to_selected(list),
                    "COLORTHEMES_PREVIOUS" => self.themes_step(list, -1),
                    _ => self.themes_step(list, 1),
                }
                self.invalidate();
            }
            _ => {
                let parent = self.objects[source].base.parent;
                let t = target.and_then(|t| {
                    parent.and_then(|p| self.find_object(p, t)).or_else(|| {
                        let windows = self.windows();
                        windows.iter().find_map(|l| self.find_object(*l, t))
                    })
                });
                if let Some(t) = t {
                    self.dispatch(
                        Handle::Gui(t),
                        "onaction",
                        &[
                            Value::str(action),
                            Value::str(param.unwrap_or("")),
                            Value::Int(0),
                            Value::Int(0),
                            Value::Int(0),
                            Value::Int(0),
                            Value::Obj(Handle::Gui(source)),
                        ],
                    );
                }
            }
        }
        self.invalidate();
    }

    /// A TOGGLE action: opens or closes the window `id` names, or flips a flag of that name when there is no such window.
    fn toggle_window(&mut self, id: &str, param: &str) {
        match self.toggle_target(id) {
            Some(c) => {
                if self.opened.contains(&c) {
                    self.close_container(c);
                } else {
                    // a component opened by its GUID takes the focus (Winamp Modern's playlist
                    // editor, autofocus); a container shown for the first time does not
                    // (Winamp3's EQ), one shown again does
                    let takes_focus =
                        param.to_lowercase().contains("guid:") || self.shown_before.contains(&c);
                    self.open_container(c);
                    if takes_focus {
                        self.focused = self.containers[c].current;
                    }
                }
            }
            None => {
                let key = format!("window:{id}");
                let on = !self.config_flags.get(&key).copied().unwrap_or(false);
                self.config_flags.insert(key, on);
            }
        }
    }

    /// Whether the window or switch a button's action names is on, for its active image.
    pub(crate) fn action_state(&mut self, action: Option<&str>, param: Option<&str>) -> bool {
        match action.map(|a| a.to_uppercase()).as_deref() {
            Some("TOGGLE") => {
                let Some(param) = param else { return false };
                let id = param.to_lowercase();
                let id = id.strip_prefix("guid:").unwrap_or(&id).to_string();
                let kind = self.window_kind(&id);
                if let Some(open) = self.host.window_open(&kind) {
                    return open;
                }
                match self.toggle_target(&id) {
                    Some(c) => self.is_open(c),
                    None => self
                        .config_flags
                        .get(&format!("window:{id}"))
                        .copied()
                        .unwrap_or(false),
                }
            }
            Some("EQ_TOGGLE") => self.player.eq_enabled(),
            Some("EQ_AUTO") => self.config_flags.get("eq_auto").copied().unwrap_or(false),
            _ => false,
        }
    }

    /// Whether `action_state` reads the state of `action`: a window, EQ on, EQ auto.
    pub(crate) fn has_action_state(&self, action: Option<&str>) -> bool {
        action.is_some_and(|a| STATE_ACTIONS.contains(&a.to_uppercase().as_str()))
    }
}

/// `v` kept to at least `min` and at most `max` (none when 0) and `view`, the view's side winning over a minimum.
fn within(v: i32, min: i32, max: i32, view: i32) -> i32 {
    let top = if max > 0 { max.min(view) } else { view };
    v.min(top).max(min.min(top)).max(1)
}
