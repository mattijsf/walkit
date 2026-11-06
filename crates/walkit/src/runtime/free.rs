// SPDX-License-Identifier: GPL-3.0-or-later

//! The windows set out apart, as on Winamp's desktop: each window where it is, moved by a drag
//! on its move areas, docking and snapping to the viewport's edges and to each other as Winamp
//! 5.666 does. The host turns this on with `Presentation::free_windows`; off, the windows are
//! one stack.
//!
//! The rules, measured against Winamp: a moved window's edge within [`DOCK_DISTANCE`] (strictly)
//! of the viewport's same edge, of another window's same edge, or of another window's facing
//! edge lands on it, one snap per axis per target; a snap to a window holds only when the moved
//! window then touches it (an edge line in common, the ranges on the other axis overlapping)
//! without overlapping it. The rects dock by their part inside the script's `snapAdjust`
//! margins. The main window carries every window that touches it, and theirs, transitively; a
//! member snapping shifts the whole group. Any other window moves alone. `beforeRedock` and
//! `redock` keep the windows docked to a window's edge on it when the script changes that edge.
//! A window that opens comes on top; a press raises its window, and raising the main window
//! brings the windows docked to it along, under it in their order.

use super::SkinRuntime;
use crate::draw::Rect;
use crate::gui::ObjId;
use crate::maki::Handle;

/// How close an edge comes before it snaps, in layout pixels: Winamp's "Dock windows at 10 pixels".
pub const DOCK_DISTANCE: i32 = 10;

/// What the free arrangement keeps.
#[derive(Debug)]
pub(crate) struct FreeState {
    /// Whether the windows are set out apart.
    pub(crate) on: bool,
    /// How close an edge comes before it snaps.
    pub(crate) dock_distance: i32,
    drag: Option<Drag>,
    /// What `beforeRedock` noted: the window, its docking rect then, and the windows docked to it.
    redock: Vec<(ObjId, Rect, Vec<ObjId>)>,
    /// The windows from the bottom to the top.
    z: Vec<ObjId>,
}

impl Default for FreeState {
    fn default() -> Self {
        FreeState {
            on: false,
            dock_distance: DOCK_DISTANCE,
            drag: None,
            redock: Vec::new(),
            z: Vec::new(),
        }
    }
}

/// A window being dragged.
#[derive(Debug, Clone)]
struct Drag {
    /// The window under the finger.
    window: ObjId,
    /// Where the finger went down, in the viewport.
    origin: (i32, i32),
    /// The windows that move, the dragged one first, each with where it started.
    group: Vec<(ObjId, Rect)>,
    /// Whether the dragged window touched another when the drag began.
    docked_before: bool,
    /// Whether the finger moved the window yet.
    moved: bool,
}

/// The snap margins of a window: left, top, right, bottom.
#[derive(Debug, Clone, Copy, Default)]
struct Margins {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `r` less `m`: the part of a window that docks.
fn inset(r: Rect, m: Margins) -> Rect {
    Rect::new(
        r.left + m.left,
        r.top + m.top,
        r.width - m.left - m.right,
        r.height - m.top - m.bottom,
    )
}

/// The window rect of a docking rect `r` with margins `m`.
fn outset(r: Rect, m: Margins) -> Rect {
    Rect::new(
        r.left - m.left,
        r.top - m.top,
        r.width + m.left + m.right,
        r.height + m.top + m.bottom,
    )
}

/// Whether `a` and `b` share an edge line with their ranges on the other axis overlapping (inclusive).
fn touches(a: Rect, b: Rect) -> bool {
    let along_x =
        a.left == b.right() || a.right() == b.left || a.right() == b.right() || a.left == b.left;
    let along_y =
        a.top == b.bottom() || a.bottom() == b.top || a.bottom() == b.bottom() || a.top == b.top;
    (along_x && b.bottom() >= a.top && a.bottom() >= b.top)
        || (along_y && b.right() >= a.left && a.right() >= b.left)
}

/// Whether `a` and `b` cover some pixel both.
fn overlaps(a: Rect, b: Rect) -> bool {
    a.left.max(b.left) < a.right().min(b.right()) && a.top.max(b.top) < a.bottom().min(b.bottom())
}

/// `r` moved by `dx`, `dy`.
fn moved(r: Rect, dx: i32, dy: i32) -> Rect {
    Rect::new(r.left + dx, r.top + dy, r.width, r.height)
}

impl SkinRuntime {
    /// Whether the windows are set out apart.
    pub(crate) fn free_windows(&self) -> bool {
        self.free.on
    }

    /// Sets the windows out apart, or as one stack again.
    pub(crate) fn set_free_windows(&mut self, on: bool) {
        if self.free.on == on {
            return;
        }
        self.free.on = on;
        self.free.drag = None;
        self.relayout();
    }

    /// Places every window at its own spot. A window without one yet takes the spot the stack
    /// gives it, so the first look is the stack's; from then on each window keeps where it is
    /// and the size the skin or the listener gave it.
    pub(crate) fn arrange_free(&mut self) {
        let windows = self.windows();
        let unplaced: Vec<ObjId> = windows
            .iter()
            .copied()
            .filter(|l| !self.objects[*l].layout().is_some_and(|d| d.free_placed))
            .collect();
        if !unplaced.is_empty() {
            self.arrange_stack();
            for l in unplaced {
                let r = self.window_rect_of(l);
                let d = self.objects[l].layout_mut().expect("a layout");
                d.view_x = r.left;
                d.view_y = r.top;
                d.free_placed = true;
            }
        }
        self.placed.clear();
        let Some(main) = self.main_layout() else {
            return;
        };
        self.layout_root(main);
        for l in windows {
            if l != main {
                // the size the listener gave it by its grip, else the skin's
                let sized = self.objects[l].layout().and_then(|d| d.user_size);
                let b = &self.objects[l].base;
                let skin_w = b.attr("w").and_then(crate::gui::to_int_lenient);
                let skin_h = b.attr("h").and_then(crate::gui::to_int_lenient);
                self.objects[l].base.w = sized.map(|s| s.0).or(skin_w);
                self.objects[l].base.h = sized.map(|s| s.1).or(skin_h);
            }
            let (w, h) = self.layout_size(l);
            let d = self.objects[l].layout().expect("a layout");
            let r = Rect::new(d.view_x, d.view_y, w, h);
            self.placed.insert(l, r);
        }
    }

    /// The windows from the bottom to the top, as they draw and as a touch finds them: the
    /// stack's order when the windows are one stack, the raised order apart.
    pub(crate) fn z_windows(&mut self) -> Vec<ObjId> {
        let windows = self.windows();
        if !self.free.on {
            return windows;
        }
        // windows gone leave the order; new ones come on top, the main window on top at first
        self.free.z.retain(|w| windows.contains(w));
        if self.free.z.is_empty() {
            self.free.z = windows.iter().rev().copied().collect();
        } else {
            for w in windows.iter().copied() {
                if !self.free.z.contains(&w) {
                    self.free.z.push(w);
                }
            }
        }
        self.free.z.clone()
    }

    /// A press on window `l` raises it to the top; the main window brings the windows docked
    /// to it along, under it in their order.
    pub(crate) fn raise_window(&mut self, l: ObjId) {
        let mut z = self.z_windows();
        if z.last() == Some(&l) {
            return;
        }
        let group = if Some(l) == self.main_layout() {
            self.docked_group(l)
        } else {
            vec![l]
        };
        let lifted: Vec<ObjId> = z.iter().copied().filter(|w| group.contains(w)).collect();
        z.retain(|w| !group.contains(w));
        z.extend(lifted.into_iter().filter(|w| *w != l));
        z.push(l);
        self.free.z = z;
        self.repaint();
    }

    /// Puts window `l` at `x`, `y` in the viewport, as the host puts back a window where the
    /// listener left it.
    pub(crate) fn free_place(&mut self, l: ObjId, x: i32, y: i32) {
        let d = self.objects[l].layout_mut().expect("a layout");
        d.view_x = x;
        d.view_y = y;
        d.free_placed = true;
        self.relayout();
    }

    fn snap_margins(&self, l: ObjId) -> Margins {
        let d = self.objects[l].layout().expect("a layout");
        Margins {
            left: d.snap_left.max(0),
            top: d.snap_top.max(0),
            right: d.snap_right.max(0),
            bottom: d.snap_bottom.max(0),
        }
    }

    /// The part of window `l` that docks: its rect inside its snap margins.
    pub(crate) fn docking_rect(&mut self, l: ObjId) -> Rect {
        let r = self.window_rect_of(l);
        inset(r, self.snap_margins(l))
    }

    /// The windows docked to `l`, and to those, transitively: each touches one of the set
    /// without overlapping it. `l` comes first.
    pub(crate) fn docked_group(&mut self, l: ObjId) -> Vec<ObjId> {
        if self.needs_layout {
            self.do_layout();
        }
        let windows = self.windows();
        let rects: Vec<(ObjId, Rect)> = windows
            .iter()
            .map(|w| (*w, self.docking_rect(*w)))
            .collect();
        let mut group = vec![l];
        let mut i = 0;
        while i < group.len() {
            let a = rects.iter().find(|(w, _)| *w == group[i]).map(|x| x.1);
            let Some(a) = a else {
                break;
            };
            for (w, b) in &rects {
                if !group.contains(w) && touches(a, *b) && !overlaps(a, *b) {
                    group.push(*w);
                }
            }
            i += 1;
        }
        group
    }

    /// Whether window `l` touches another window.
    fn touches_any(&mut self, l: ObjId) -> bool {
        self.docked_group(l).len() > 1
    }

    /// Where `r`, window `l`'s rect, lands when the listener moves it there: snapped to the
    /// viewport's edges and to the other windows, those in `group` left out as the ones moving
    /// along.
    pub(crate) fn autodock(&mut self, l: ObjId, r: Rect, group: &[ObjId]) -> Rect {
        if self.needs_layout {
            self.do_layout();
        }
        let d = self.free.dock_distance;
        let m = self.snap_margins(l);
        let mut ri = inset(r, m);
        // the viewport first, then the windows from the last opened to the first, as Winamp
        let mut targets: Vec<(bool, Rect)> =
            vec![(false, Rect::new(0, 0, self.viewport_w, self.viewport_h))];
        let others: Vec<ObjId> = self
            .windows()
            .into_iter()
            .rev()
            .filter(|w| *w != l && !group.contains(w))
            .collect();
        for w in others {
            let dr = self.docking_rect(w);
            targets.push((true, dr));
        }
        let near = |a: i32, b: i32| (b - d) < a && a < (b + d);
        for (window, o) in targets {
            let before = ri;
            let mut snapped_x = false;
            let mut snapped_y = false;
            // the same edge of the viewport or the window, and the facing edge of a window, in
            // the order Winamp tries them
            if near(ri.left, o.left) {
                ri = moved(ri, o.left - ri.left, 0);
                snapped_x = true;
            }
            if window && !snapped_x && near(ri.right(), o.left) {
                ri = moved(ri, o.left - ri.right(), 0);
                snapped_x = true;
            }
            if near(ri.top, o.top) {
                ri = moved(ri, 0, o.top - ri.top);
                snapped_y = true;
            }
            if window && !snapped_y && near(ri.bottom(), o.top) {
                ri = moved(ri, 0, o.top - ri.bottom());
                snapped_y = true;
            }
            if !snapped_x && near(ri.right(), o.right()) {
                ri = moved(ri, o.right() - ri.right(), 0);
                snapped_x = true;
            }
            if window && !snapped_x && near(ri.left, o.right()) {
                ri = moved(ri, o.right() - ri.left, 0);
            }
            if !snapped_y && near(ri.bottom(), o.bottom()) {
                ri = moved(ri, 0, o.bottom() - ri.bottom());
                snapped_y = true;
            }
            if window && !snapped_y && near(ri.top, o.bottom()) {
                ri = moved(ri, 0, o.bottom() - ri.top);
            }
            // a snap to a window holds only where the window is then touched, not overlapped
            if window && ri != before && (!touches(ri, o) || overlaps(ri, o)) {
                ri = before;
            }
        }
        outset(ri, m)
    }

    /// The finger goes down on window `window` at a point of the viewport, to move it.
    pub(crate) fn free_drag_begin(&mut self, window: ObjId, sx: i32, sy: i32) {
        let members = if Some(window) == self.main_layout() {
            self.docked_group(window)
        } else {
            vec![window]
        };
        let group: Vec<(ObjId, Rect)> = members
            .into_iter()
            .map(|l| (l, self.window_rect_of(l)))
            .collect();
        let docked_before = self.touches_any(window);
        self.free.drag = Some(Drag {
            window,
            origin: (sx, sy),
            group,
            docked_before,
            moved: false,
        });
    }

    /// Whether a window is being dragged.
    pub(crate) fn free_dragging(&self) -> bool {
        self.free.drag.is_some()
    }

    /// The finger moves to a point of the viewport while a window is dragged: the window and
    /// its group go along, snapped as Winamp snaps them.
    pub(crate) fn free_drag_to(&mut self, sx: i32, sy: i32) {
        let Some(drag) = self.free.drag.clone() else {
            return;
        };
        let (dx, dy) = (sx - drag.origin.0, sy - drag.origin.1);
        let members: Vec<ObjId> = drag.group.iter().map(|g| g.0).collect();
        let (lead, start) = drag.group[0];
        let wanted = moved(start, dx, dy);
        let got = self.autodock(lead, wanted, &members);
        let mut shift_x = got.left - wanted.left;
        let mut shift_y = got.top - wanted.top;
        // a member snapping to the viewport or to a window outside the group shifts the group
        let mut more_x = 0;
        let mut more_y = 0;
        for (l, r0) in drag.group.iter().skip(1) {
            let want = moved(*r0, dx + shift_x, dy + shift_y);
            let got = self.autodock(*l, want, &members);
            if more_x == 0 {
                more_x = got.left - want.left;
            }
            if more_y == 0 {
                more_y = got.top - want.top;
            }
        }
        shift_x += more_x;
        shift_y += more_y;
        let mut changed = false;
        for (l, r0) in &drag.group {
            let (x, y) = (r0.left + dx + shift_x, r0.top + dy + shift_y);
            let d = self.objects[*l].layout().expect("a layout");
            if d.view_x != x || d.view_y != y {
                changed = true;
                let d = self.objects[*l].layout_mut().expect("a layout");
                d.view_x = x;
                d.view_y = y;
            }
        }
        if !changed {
            return;
        }
        if let Some(d) = self.free.drag.as_mut() {
            d.moved = true;
        }
        self.relayout();
        for (l, _) in &drag.group {
            self.dispatch(Handle::Gui(*l), "onmove", &[]);
        }
    }

    /// The finger lifts: the moved windows hear it, and the dragged one hears whether it is
    /// docked now where it was not, or the other way round.
    pub(crate) fn free_drag_end(&mut self) {
        let Some(drag) = self.free.drag.take() else {
            return;
        };
        if !drag.moved {
            return;
        }
        for (l, _) in &drag.group {
            self.dispatch(Handle::Gui(*l), "onendmove", &[]);
        }
        let docked = self.touches_any(drag.window);
        if docked != drag.docked_before {
            let event = if docked { "ondock" } else { "onundock" };
            self.dispatch(Handle::Gui(drag.window), event, &[]);
        }
    }

    /// A script's `beforeRedock` on window `l`: notes the windows docked to it and where its
    /// docking rect is, before the script changes that rect.
    pub(crate) fn before_redock(&mut self, l: ObjId) {
        if !self.free.on {
            return;
        }
        let rect = self.docking_rect(l);
        let docked: Vec<ObjId> = self.docked_group(l).into_iter().skip(1).collect();
        self.free.redock.retain(|(w, _, _)| *w != l);
        self.free.redock.push((l, rect, docked));
    }

    /// A script's `redock` on window `l`: the windows docked to an edge of it that moved go
    /// along with that edge, as the playlist under Winamp Modern's main window follows its
    /// drawer.
    pub(crate) fn redock(&mut self, l: ObjId) {
        if !self.free.on {
            return;
        }
        let Some(i) = self.free.redock.iter().position(|(w, _, _)| *w == l) else {
            return;
        };
        let (_, old, docked) = self.free.redock.remove(i);
        if self.needs_layout {
            self.do_layout();
        }
        let new = self.docking_rect(l);
        if new == old {
            return;
        }
        for w in docked {
            let r = self.docking_rect(w);
            let (dx, dy) = if r.top >= old.bottom() {
                (0, new.bottom() - old.bottom())
            } else if r.left >= old.right() {
                (new.right() - old.right(), 0)
            } else if r.bottom() <= old.top {
                (0, new.top - old.top)
            } else if r.right() <= old.left {
                (new.left - old.left, 0)
            } else {
                (0, 0)
            };
            if dx != 0 || dy != 0 {
                let d = self.objects[w].layout_mut().expect("a layout");
                d.view_x += dx;
                d.view_y += dy;
            }
        }
        self.relayout();
    }
}
