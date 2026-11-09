// SPDX-License-Identifier: GPL-3.0-or-later
//! Containers and windows: which are open, where each sits, what the host sees of them, and the
//! presentation the host sets.

use super::{
    ContainerId, Handle, ObjId, ObjectInfo, Presentation, Rect, SkinRuntime, Value, WindowId,
    WindowInfo,
};

impl SkinRuntime {
    /// The container `id`, matched ignoring case.
    pub(crate) fn container(&self, id: &str) -> Option<ContainerId> {
        self.containers
            .iter()
            .position(|c| c.id.eq_ignore_ascii_case(id))
    }

    pub(crate) fn main_container(&self) -> Option<ContainerId> {
        self.container("main").or(if self.containers.is_empty() {
            None
        } else {
            Some(0)
        })
    }

    pub(crate) fn main_layout(&self) -> Option<ObjId> {
        self.main_container()
            .and_then(|c| self.containers[c].current)
    }

    /// The windows on screen, top to bottom: the main one first.
    pub(crate) fn windows(&mut self) -> Vec<ObjId> {
        if self.windows_at != self.change_count {
            let mut w: Vec<ObjId> = self.main_layout().into_iter().collect();
            w.extend(
                self.opened
                    .iter()
                    .filter_map(|c| self.containers[*c].current),
            );
            self.windows_now = w;
            self.windows_at = self.change_count;
        }
        self.windows_now.clone()
    }

    /// A new id for `layout`, which keeps it; see [`WindowId`].
    pub(crate) fn new_window_id(&mut self) -> WindowId {
        WindowId(self.layouts_by_id.len() as u32 + 1)
    }

    pub(crate) fn register_layout(&mut self, layout: ObjId) {
        self.layouts_by_id.push(layout);
    }

    fn layout_of(&self, id: WindowId) -> ObjId {
        self.layouts_by_id[(id.0 as usize).wrapping_sub(1)]
    }

    pub(crate) fn window_id_of(&self, layout: ObjId) -> WindowId {
        self.objects[layout]
            .layout()
            .map(|l| l.window_id)
            .unwrap_or(WindowId(0))
    }

    /// The windows on screen, top to bottom, as `windows` has them: the main one first.
    pub fn window_ids(&mut self) -> Vec<WindowId> {
        let now = self.windows();
        if self.window_ids_at != self.change_count {
            self.window_ids_now = now.iter().map(|l| self.window_id_of(*l)).collect();
            self.window_ids_at = self.change_count;
        }
        self.window_ids_now.clone()
    }

    /// The main window as it shows now (its normal layout or its shade), or `None` when the skin has none.
    pub fn main_window(&self) -> Option<WindowId> {
        self.main_layout().map(|l| self.window_id_of(l))
    }

    /// What `id` is like now; see [`WindowInfo`].
    pub fn window_info(&mut self, id: WindowId) -> WindowInfo {
        let l = self.layout_of(id);
        let size = self.layout_size(l);
        let both = self.declared_resizable_both_ways(l);
        let d = self.objects[l].layout().expect("a layout");
        WindowInfo {
            id,
            size,
            min_w: d.min_w,
            min_h: d.min_h,
            max_w: d.max_w,
            max_h: d.max_h,
            resizable_both_ways: both,
            edge_x: d.edge_x,
            edge_y: d.edge_y,
            container: d
                .container
                .map(|c| self.containers[c].id.clone())
                .unwrap_or_default(),
            layout: self.objects[l].base.id.clone(),
        }
    }

    /// The object with id `id` in the windows on screen, the top one first; `None` when none has it.
    pub fn object_info(&mut self, id: &str) -> Option<ObjectInfo> {
        let windows = self.windows();
        let o = windows
            .iter()
            .rev()
            .find_map(|w| self.find_object(*w, id))?;
        let layout = self.layout_object(o)?;
        let r = self.objects[o].base.rect;
        Some(ObjectInfo {
            window: self.window_id_of(layout),
            rect: r,
            stack_rect: self.stack_rect_in(layout, r),
            visible: self.objects[o].base.visible,
            class_name: self.objects[o].class_name().to_string(),
        })
    }

    /// The window container `container` shows now, or `None` when the skin has no such container.
    pub fn window_of(&self, container: &str) -> Option<WindowId> {
        self.container(container)
            .and_then(|c| self.containers[c].current)
            .map(|l| self.window_id_of(l))
    }

    /// Where `id` sits in the stack, in layout pixels.
    pub fn window_rect(&mut self, id: WindowId) -> Rect {
        self.window_rect_of(self.layout_of(id))
    }

    /// The part of window `id` that docks to others: its rect inside the snap margins its skin
    /// gives it (Winamp Modern's main window leaves out its closed drawer).
    pub fn window_docking_rect(&mut self, id: WindowId) -> Rect {
        let l = self.layout_of(id);
        self.docking_rect(l)
    }

    /// Where `layout` sits in the stack, in layout pixels.
    pub(crate) fn window_rect_of(&mut self, layout: ObjId) -> Rect {
        if let Some(r) = self.placed.get(&layout) {
            *r
        } else {
            let (w, h) = self.screen_size(layout);
            Rect::new(0, 0, w, h)
        }
    }

    /// Screen pixels per layout pixel of window `l`: its container's scale.
    pub(crate) fn layout_scale(&self, l: ObjId) -> f32 {
        self.objects[l]
            .layout()
            .and_then(|d| d.container)
            .map_or(1.0, |c| self.containers[c].scale)
    }

    /// The size window `l` takes on screen: its layout's size at its scale.
    pub(crate) fn screen_size(&mut self, l: ObjId) -> (i32, i32) {
        let (w, h) = self.layout_size(l);
        let r = Rect::new(0, 0, w, h).scaled(self.layout_scale(l));
        (r.width, r.height)
    }

    /// The point `sx`, `sy` of the stack in window `l`'s layout pixels.
    pub(crate) fn layout_point(&mut self, l: ObjId, sx: i32, sy: i32) -> (i32, i32) {
        let at = self.window_rect_of(l);
        let s = self.layout_scale(l);
        (
            ((sx - at.left) as f32 / s).floor() as i32,
            ((sy - at.top) as f32 / s).floor() as i32,
        )
    }

    /// Rectangle `r` of window `l`, in its layout pixels, where it shows in the stack.
    pub(crate) fn stack_rect_in(&mut self, l: ObjId, r: Rect) -> Rect {
        let at = self.window_rect_of(l);
        let r = r.scaled(self.layout_scale(l));
        Rect::new(at.left + r.left, at.top + r.top, r.width, r.height)
    }

    /// Names the windows of `container`: the title a classic `gen.bmp` frame shows (the
    /// visualizer's plug-in, say). False when the skin has no such container.
    pub fn set_window_title(&mut self, container: &str, title: &str) -> bool {
        let Some(c) = self.container(container) else {
            return false;
        };
        if self.containers[c].name != title {
            self.containers[c].name = title.to_string();
            self.repaint();
        }
        true
    }

    /// Screen pixels per layout pixel of the windows of `container`; 1 when it has none.
    pub fn window_scale(&self, container: &str) -> f32 {
        self.container(container)
            .map_or(1.0, |c| self.containers[c].scale)
    }

    /// Shows the windows of `container` `scale` times as large (2 for Winamp's double size),
    /// from 0.25 to 8. Apart, the windows hanging off its edges, and off theirs, stay docked.
    /// False when the skin has no such container.
    pub fn set_window_scale(&mut self, container: &str, scale: f32) -> bool {
        let Some(c) = self.container(container) else {
            return false;
        };
        let scale = if scale.is_finite() {
            scale.clamp(0.25, 8.0)
        } else {
            1.0
        };
        if self.containers[c].scale == scale {
            return true;
        }
        self.keeping_docks(|rt| {
            rt.containers[c].scale = scale;
            rt.relayout();
        });
        true
    }

    /// Opens the window of `container` in the stack; false when the skin has no such container.
    pub fn open_window(&mut self, container: &str) -> bool {
        match self.container(container) {
            Some(c) => {
                self.open_container(c);
                true
            }
            None => false,
        }
    }

    /// Closes the window of `container`; false when the skin has no such container.
    pub fn close_window(&mut self, container: &str) -> bool {
        match self.container(container) {
            Some(c) => {
                self.close_container(c);
                true
            }
            None => false,
        }
    }

    /// `id`'s size as it shows now, in layout pixels.
    pub fn window_size(&mut self, id: WindowId) -> (i32, i32) {
        self.layout_size(self.layout_of(id))
    }

    /// Sizes `id` to `width` by `height` where it is, as the host sizes a window to its box.
    pub fn resize_window(&mut self, id: WindowId, width: i32, height: i32) {
        let l = self.layout_of(id);
        // apart, a window besides the main one keeps the size the host gives it, as one the
        // listener sized by its grip
        if self.free.on && Some(l) != self.main_layout() {
            self.objects[l].layout_mut().expect("a layout").user_size = Some((width, height));
        }
        self.resize_layout_to(l, width, height);
    }

    /// Whether a window besides the main one is open, which Back would put away.
    pub fn window_open(&self) -> bool {
        !self.opened.is_empty()
    }

    /// The space the app has for the stack, in layout pixels; `None` leaves each window at the size it has.
    pub fn set_view_size(&mut self, value: Option<(i32, i32)>) {
        if value != self.view_size {
            self.view_size = value;
            for c in 0..self.containers.len() {
                self.fit_viewport(c);
            }
            self.relayout();
        }
    }

    /// The space the app has for the stack.
    pub fn view_size(&self) -> Option<(i32, i32)> {
        self.view_size
    }

    /// What `getSkinName()` answers: the skin's folder name, which scripts key what they remember
    /// by; the name in the skin's `<skininfo>` when the host gave none before `load`.
    pub fn skin_name(&self) -> &str {
        &self.skin_name
    }

    /// Names the skin for its scripts, before or after `load`; a name given stays through `load`.
    pub fn set_skin_name(&mut self, name: &str) {
        self.skin_name = name.to_string();
    }

    /// How the host shows the windows, as one value.
    pub fn presentation(&self) -> Presentation {
        Presentation {
            view_size: self.view_size,
            viewport: (self.viewport_w, self.viewport_h),
            columns: self.columns,
            column_width: self.column_width,
            stretch_last: self.stretch_last,
            touch_slop: self.touch_slop,
            free_windows: self.free.on,
            touch: self.touch,
            skin_list_font: self.skin_list_font,
            list_row_extra: self.list_row_extra,
        }
    }

    /// Sets how the host shows the windows; what differs changes, as setting the viewport, the
    /// columns, the column width, the stretch, the touch slop and the view size one by one does.
    pub fn set_presentation(&mut self, p: &Presentation) {
        self.set_viewport(p.viewport.0, p.viewport.1);
        self.set_columns(p.columns);
        self.set_column_width(p.column_width);
        self.set_stretch_last(p.stretch_last);
        self.touch_slop = p.touch_slop;
        self.touch = p.touch;
        if p.list_row_extra.max(0) != self.list_row_extra {
            self.list_row_extra = p.list_row_extra.max(0);
            self.dirty = true;
        }
        if p.skin_list_font != self.skin_list_font {
            self.skin_list_font = p.skin_list_font;
            self.dirty = true;
        }
        if !p.touch {
            self.classic_open_menu = None;
        }
        self.set_view_size(p.view_size);
        self.set_free_windows(p.free_windows);
    }

    /// Puts window `id` at `x`, `y` in the viewport, among windows set out apart: where the
    /// host keeps it between runs. Nothing changes while the windows are one stack.
    pub fn place_window(&mut self, id: WindowId, x: i32, y: i32) {
        if !self.free.on {
            return;
        }
        let l = self.layout_of(id);
        self.free_place(l, x, y);
    }

    /// How many columns the stack is set out in once the host gives it a view size: at 2 the
    /// windows that show a list go in a column right of the main window, else every window
    /// docks under the main one. A change lays the windows out again.
    pub fn set_columns(&mut self, columns: i32) {
        if columns != self.columns {
            self.columns = columns;
            self.relayout();
        }
    }

    /// How wide the windows in the right column of two columns are, in layout pixels; 0 for as
    /// wide as what shows of the main window. A change lays the windows out again.
    pub fn set_column_width(&mut self, width: i32) {
        if width != self.column_width {
            self.column_width = width;
            self.relayout();
        }
    }

    /// Whether the last docked window takes the height the view leaves under the others, when
    /// it can be made taller and shows a list. Off, every window keeps the height the skin gives it.
    pub fn set_stretch_last(&mut self, stretch: bool) {
        if stretch != self.stretch_last {
            self.stretch_last = stretch;
            self.relayout();
        }
    }

    /// How far beside an object a finger may land and still mean it, in layout pixels.
    pub fn set_touch_slop(&mut self, slop: i32) {
        self.touch_slop = slop;
    }

    /// Where `id` visibly ends on the right; with `lay_out_first` after laying it out at its
    /// size, as the stack is measured.
    pub fn content_right_of(&mut self, id: WindowId, lay_out_first: bool) -> i32 {
        let l = self.layout_of(id);
        if lay_out_first {
            self.layout_root(l);
        }
        self.content_right(l)
    }

    /// Whether a window docked under the main one shows a list.
    pub fn list_window_open(&mut self) -> bool {
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        others.iter().any(|l| self.shows_list(*l))
    }

    /// How far right the stack shows in one column while the main window shows `width` across
    /// (0 for what it shows now): that width, or a docked window its minimum width makes wider.
    /// It changes no window's size and runs no layout pass. 0 without a main window.
    pub fn shown_width(&mut self, width: i32) -> i32 {
        let Some(main) = self.main_layout() else {
            return 0;
        };
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        self.layout_root(main);
        let across = if width > 0 {
            width
        } else {
            self.content_right(main)
        };
        let widest = others
            .iter()
            .map(|l| self.size_at(*l, across).0)
            .max()
            .unwrap_or(0);
        across.max(widest)
    }

    /// How large the stack is in `columns` with a view and every window at its natural height,
    /// with a right column `column_width` wide, in layout pixels. It changes no window's size
    /// and runs no layout pass, so the skin hears nothing: the host weighs one column against
    /// two with it. `None` without a main window.
    pub fn natural_stack_size(&mut self, columns: i32, column_width: i32) -> Option<(i32, i32)> {
        let main = self.main_layout()?;
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        let (mut right, mut bottom) = self.layout_size(main);
        self.layout_root(main);
        for column in self.columns_of(main, &others, columns, column_width) {
            let mut y = column.top;
            for l in column.windows {
                let (w, h) = self.size_at(l, column.width);
                right = right.max(column.x + w);
                y += h;
            }
            bottom = bottom.max(y);
        }
        Some((right, bottom))
    }

    /// The left column of the stack in two columns, in layout pixels from the stack's corner:
    /// from the left edge to where the right column starts, and from the top to where the lowest
    /// window in it ends as Winamp docks to it. `None` for a stack in one column: the columns are
    /// not 2, or no list window is open.
    pub fn left_column(&mut self) -> Option<Rect> {
        let main = self.main_layout()?;
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        if self.columns != 2 || !others.iter().any(|l| self.shows_list(*l)) {
            return None;
        }
        let mut column = vec![main];
        column.extend(others.iter().copied().filter(|l| !self.shows_list(*l)));
        let mut right = 0;
        let mut bottom = 0;
        for l in column {
            let r = self.window_rect_of(l);
            right = right.max(r.right());
            bottom = bottom.max(r.top + self.snapped_size(l).1);
        }
        Some(Rect::new(0, 0, right, bottom))
    }

    /// How many times a script moved the main window against an edge of the viewport after the
    /// listener picked from one of its menus in the same touch.
    pub fn edge_placements(&self) -> i32 {
        self.edge_placements
    }

    pub(crate) fn main_placed_at_edge(&mut self) {
        if self.pointer.gesture_depth > 0 && self.pointer.picked_this_gesture {
            self.edge_placements += 1;
        }
    }

    /// Whether a skin window is open: on a phone (a view size), when it is in the stack,
    /// whatever its own visibility says; without one, as the window itself says.
    pub(crate) fn is_open(&self, c: ContainerId) -> bool {
        if self.view_size.is_none() {
            self.containers[c].visible
        } else {
            Some(c) == self.main_container() || self.opened.contains(&c)
        }
    }

    pub(crate) fn open_container(&mut self, c: ContainerId) {
        if Some(c) == self.main_container() || self.opened.contains(&c) {
            return;
        }
        self.opened.push(c);
        self.shown_before.insert(c);
        self.container_show(c);
        self.relayout();
    }

    /// A component shows in one place at a time: a window holder that takes it as it shows
    /// (`autoopen`) closes the window made for the component (Winamp Modern's drawer, its
    /// visualizer reattached, closes the Visualizations window). Whether a window closed.
    pub(crate) fn hand_components_to_holders(&mut self) -> bool {
        let mut shown = Vec::new();
        for l in self.windows() {
            let mut stack = vec![l];
            while let Some(o) = stack.pop() {
                let obj = &self.objects[o];
                if !obj.base.visible {
                    continue;
                }
                if super::HOLDER_TAGS.contains(&obj.base.tag.as_str())
                    && obj.base.attr("autoopen") == Some("1")
                {
                    shown.push((o, l));
                }
                stack.extend(obj.children().iter().copied());
            }
        }
        let before =
            std::mem::replace(&mut self.holders_shown, shown.iter().map(|s| s.0).collect());
        let mut closed = false;
        for (holder, l) in shown {
            if before.contains(&holder) {
                continue;
            }
            let own = self.containers.iter().position(|c| c.layouts.contains(&l));
            let made_for: Vec<ContainerId> = (0..self.containers.len())
                .filter(|&c| Some(c) != own && self.opened.contains(&c))
                .filter(|&c| {
                    self.containers[c].attr("component").is_some_and(|v| {
                        let guid = v.to_lowercase();
                        let guid = guid.trim_start_matches("guid:").trim_matches(['{', '}']);
                        Self::holds(&self.objects[holder].base.attrs, guid)
                    })
                })
                .collect();
            for c in made_for {
                self.close_container(c);
                closed = true;
            }
        }
        closed
    }

    pub(crate) fn close_container(&mut self, c: ContainerId) {
        let Some(at) = self.opened.iter().position(|o| *o == c) else {
            return;
        };
        self.opened.remove(at);
        self.container_hide(c);
        self.relayout();
    }

    /// Gives `l` the focus: a layout switched to takes it.
    pub(crate) fn focus(&mut self, l: ObjId) {
        self.focused = Some(l);
    }

    pub(crate) fn has_focus(&mut self, l: Option<ObjId>) -> bool {
        let Some(l) = l else { return false };
        let windows = self.windows();
        let focused = self
            .focused
            .filter(|f| windows.contains(f))
            .or_else(|| self.main_layout());
        Some(l) == focused
    }

    /// The window that has the focus: the one pressed last, else the main window.
    pub fn focused_window(&mut self) -> Option<WindowId> {
        let windows = self.windows();
        self.focused
            .filter(|f| windows.contains(f))
            .or_else(|| self.main_layout())
            .map(|l| self.window_id_of(l))
    }

    /// Gives `l` the focus as a touch does: the window losing it and `l` show their alphas and
    /// pictures for it on the next frame, with nothing moved or resized.
    pub(crate) fn take_focus(&mut self, l: ObjId) {
        if Some(l) == self.focused {
            return;
        }
        let windows = self.windows();
        let had = self
            .focused
            .filter(|f| windows.contains(f))
            .or_else(|| self.main_layout());
        self.focused = Some(l);
        if had == Some(l) {
            return;
        }
        if let Some(h) = had {
            self.refresh_alphas(h);
        }
        self.refresh_alphas(l);
        self.invalidate();
    }

    /// A script's Container.show / hide: the same as a button's, into the phone's stack and out.
    pub(crate) fn show_container(&mut self, c: ContainerId, show: bool) {
        if !show && !self.containers[c].can_close() {
            return;
        }
        if self.view_size.is_none() {
            if show {
                self.container_show(c);
            } else {
                self.container_hide(c);
            }
        } else if show {
            self.open_container(c);
        } else {
            self.close_container(c);
        }
    }

    /// Shows window `id`'s container in the layout named (`normal`, `shade`), as Winamp's
    /// windowshade toggle does; nothing when the container has no such layout.
    pub fn set_window_layout(&mut self, id: WindowId, layout: &str) {
        let l = self.layout_of(id);
        let Some(c) = self.objects[l].layout().and_then(|d| d.container) else {
            return;
        };
        if self.container_layout(c, layout).is_some() {
            self.switch_to(c, layout);
        }
    }

    /// Closes the window on top, the one opened last.
    pub fn close_top_window(&mut self) {
        if let Some(c) = self.opened.last().copied() {
            self.close_container(c);
        }
    }

    pub(crate) fn container_show(&mut self, c: ContainerId) {
        if !self.containers[c].visible {
            self.containers[c].visible = true;
            self.invalidate();
            let cur = self.containers[c].current;
            self.dispatch(
                Handle::Container(c),
                "onshowlayout",
                &[Value::of(cur.map(Handle::Gui))],
            );
        }
    }

    pub(crate) fn container_hide(&mut self, c: ContainerId) {
        if self.containers[c].visible {
            self.containers[c].visible = false;
            self.invalidate();
            let cur = self.containers[c].current;
            self.dispatch(
                Handle::Container(c),
                "onhidelayout",
                &[Value::of(cur.map(Handle::Gui))],
            );
        }
    }

    /// Shows layout `id` of container `c`, or the first one when there is none by that id; the
    /// new layout hears onSetVisible(1) before the container's onSwitchToLayout, and takes the focus.
    pub(crate) fn switch_to(&mut self, c: ContainerId, id: &str) {
        let target = self
            .container_layout(c, id)
            .or_else(|| self.containers[c].layouts.first().copied());
        let Some(target) = target else { return };
        let current = self.containers[c].current;
        if current == Some(target) {
            return;
        }
        self.dispatch(
            Handle::Container(c),
            "onbeforeswitchtolayout",
            &[
                Value::of(current.map(Handle::Gui)),
                Value::Obj(Handle::Gui(target)),
            ],
        );
        if let Some(cur) = current {
            self.announce_visible(cur, false);
        }
        // apart, the new layout takes the old one's place and the windows docked to it
        let carried = current.map(|cur| self.free_switch_begin(cur));
        self.containers[c].current = Some(target);
        self.relayout();
        if let Some(carried) = carried {
            self.free_switch_end(target, carried);
        }
        self.announce_visible(target, true);
        self.focus(target);
        self.dispatch(
            Handle::Container(c),
            "onswitchtolayout",
            &[Value::Obj(Handle::Gui(target))],
        );
    }

    /// The layout `id` of container `c`.
    pub(crate) fn container_layout(&self, c: ContainerId, id: &str) -> Option<ObjId> {
        self.containers[c]
            .layouts
            .iter()
            .copied()
            .find(|l| self.objects[*l].base.id.eq_ignore_ascii_case(id))
    }

    /// Puts each layout of container `c` where a phone draws it in a new viewport.
    pub(crate) fn fit_viewport(&mut self, c: ContainerId) {
        let middle = Some(c) == self.main_container();
        for l in self.containers[c].layouts.clone() {
            self.layout_fit_viewport(l, middle);
        }
    }
}
