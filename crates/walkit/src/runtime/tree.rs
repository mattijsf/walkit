// SPDX-License-Identifier: GPL-3.0-or-later
//! The object tree: finding objects, adding children, showing and hiding, actions on windows,
//! the layout pass and the draw lists.

use super::{
    toggle_guid, ContainerId, DrawCmd, DrawnWindow, Handle, ObjId, Rect, SkinRuntime, Value,
    MAX_LAYOUT_PASSES, TOGGLE_GUIDS,
};

impl SkinRuntime {
    /// The layout `o` is in.
    pub(crate) fn layout_object(&self, o: ObjId) -> Option<ObjId> {
        let mut at = Some(o);
        while let Some(id) = at {
            if self.objects[id].is_layout() {
                return Some(id);
            }
            at = self.objects[id].base.parent;
        }
        None
    }

    /// `findObject(id)` on a group: its children, then their subtrees.
    pub(crate) fn find_object(&self, group: ObjId, wanted: &str) -> Option<ObjId> {
        let lower = wanted.to_lowercase();
        let children = self.objects[group].children();
        for c in children {
            let o = &self.objects[*c];
            if o.base.id == lower
                || o.group
                    .as_ref()
                    .is_some_and(|g| g.instance_id.as_deref() == Some(lower.as_str()))
            {
                return Some(*c);
            }
        }
        for c in children {
            if self.objects[*c].group.is_some() {
                if let Some(f) = self.find_object(*c, &lower) {
                    return Some(f);
                }
            }
        }
        None
    }

    /// `findObject(id)` as Wasabi's windows do it: this object's children, then their subtrees,
    /// then up one parent at a time, so any object in the window is found from anywhere in it.
    pub(crate) fn find_object_chain(&self, o: ObjId, id: &str) -> Option<ObjId> {
        if self.objects[o].group.is_some() {
            if let Some(f) = self.find_object(o, id) {
                return Some(f);
            }
        }
        let mut p = self.objects[o].base.parent;
        while let Some(g) = p {
            if let Some(f) = self.find_object(g, id) {
                return Some(f);
            }
            p = self.objects[g].base.parent;
        }
        None
    }

    /// The child named by `embed_xui`, if any.
    pub(crate) fn embedded(&self, g: ObjId) -> Option<ObjId> {
        let e = self.objects[g].group.as_ref()?.embed_xui.clone()?;
        self.objects[g]
            .children()
            .iter()
            .copied()
            .find(|c| self.objects[*c].base.id == e)
            .or_else(|| self.find_object(g, &e))
    }

    /// Puts `child` into `group`.
    pub(crate) fn add_child(&mut self, group: ObjId, child: ObjId) {
        if let Some(old) = self.objects[child].base.parent {
            if let Some(g) = self.objects[old].group.as_mut() {
                g.children.retain(|c| *c != child);
            }
        }
        self.objects[child].base.parent = Some(group);
        self.objects[group]
            .group
            .as_mut()
            .expect("a group")
            .children
            .push(child);
        self.invalidate();
    }

    /// Puts an object into `group`, as a script does with `obj.init(group)`.
    pub(crate) fn init_into(&mut self, o: ObjId, group: ObjId) {
        self.add_child(group, o);
        self.relayout();
    }

    /// Whether `o` shows: in a window on screen, it and every group around it visible, not
    /// faded out, and with room to draw in.
    pub(crate) fn shows(&mut self, o: ObjId) -> bool {
        let b = &self.objects[o].base;
        let room = b.rect.width > 0 && b.rect.height > 0;
        if !room || !b.visible || b.effective_alpha <= 0 {
            return false;
        }
        let mut top = o;
        while let Some(up) = self.objects[top].base.parent {
            if !self.objects[up].base.visible {
                return false;
            }
            top = up;
        }
        self.objects[top].is_layout() && self.windows().contains(&top)
    }

    /// Whether `o` can take a touch at all. `faded` also lets in one drawn fully transparent.
    pub(crate) fn touchable(&self, o: ObjId, faded: bool) -> bool {
        let b = &self.objects[o].base;
        if !(b.visible
            && b.enabled
            && (faded || b.effective_alpha > 0)
            && b.rect.width > 0
            && b.rect.height > 0)
        {
            return false;
        }
        let mut p = b.parent;
        while let Some(g) = p {
            if !self.objects[g].base.visible {
                return false;
            }
            p = self.objects[g].base.parent;
        }
        true
    }

    /// Whether every group above `o` is shown.
    pub(crate) fn parent_shown(&self, o: ObjId) -> bool {
        let mut p = self.objects[o].base.parent;
        while let Some(g) = p {
            if !self.objects[g].base.visible {
                return false;
            }
            p = self.objects[g].base.parent;
        }
        true
    }

    /// onSetVisible for `o` and, down the tree, every child whose own flag is set.
    pub(crate) fn announce_visible(&mut self, o: ObjId, shown: bool) {
        self.dispatch(Handle::Gui(o), "onsetvisible", &[Value::Int(shown as i32)]);
        for c in self.objects[o].children().to_vec() {
            if self.objects[c].base.visible {
                self.announce_visible(c, shown);
            }
        }
    }

    /// Shows `o`.
    pub(crate) fn show_object(&mut self, o: ObjId) {
        if !self.objects[o].base.visible {
            self.objects[o].base.visible = true;
            self.shown_changed(o);
            if self.parent_shown(o) {
                self.announce_visible(o, true);
            }
        }
    }

    /// Hides `o`.
    pub(crate) fn hide_object(&mut self, o: ObjId) {
        if self.objects[o].base.visible {
            self.objects[o].base.visible = false;
            self.shown_changed(o);
            if self.parent_shown(o) {
                self.announce_visible(o, false);
            }
        }
    }

    /// A frame gives a hidden pane's room to the other one.
    fn shown_changed(&mut self, o: ObjId) {
        let in_frame = self.objects[o]
            .base
            .parent
            .is_some_and(|p| self.objects[p].base.tag == "wasabi:frame");
        if in_frame {
            self.relayout();
        } else {
            self.invalidate();
        }
    }

    /// `action;param;target` as a button or a double click does it.
    pub(crate) fn run_action(&mut self, o: ObjId, spec: &str) {
        let parts: Vec<&str> = spec.split(';').collect();
        let param = parts.get(1).map(|s| s.trim().to_string());
        let target = parts.get(2).map(|s| s.trim().to_string());
        self.action(o, parts[0].trim(), param.as_deref(), target.as_deref());
    }

    /// The window a TOGGLE names: a container id, a GUID alias such as "pl", or a component GUID.
    pub(crate) fn toggle_target(&self, id: &str) -> Option<ContainerId> {
        let guid = toggle_guid(id)
            .map(|g| g.to_string())
            .unwrap_or_else(|| id.trim_matches(['{', '}']).to_string());
        self.container(id).or_else(|| {
            self.containers.iter().position(|c| {
                c.attr("component").map(|v| {
                    v.to_lowercase()
                        .trim_start_matches("guid:")
                        .trim_matches(['{', '}'])
                        .to_string()
                }) == Some(guid.clone())
            })
        })
    }

    /// The short name of a window a TOGGLE names by short name or component GUID.
    pub(crate) fn window_kind(id: &str) -> String {
        let bare = id.trim_matches(['{', '}']).to_lowercase();
        // a GUID or an alias ("library") by its first alias ("ml")
        let guid = toggle_guid(&bare)
            .map(|g| g.to_string())
            .unwrap_or_else(|| bare.clone());
        TOGGLE_GUIDS
            .iter()
            .find(|(_, g)| *g == guid)
            .map(|(k, _)| k.to_string())
            .unwrap_or(bare)
    }

    /// Whether a component, a window holder or a container with the attributes `attrs` shows
    /// the component `guid`, named by its `hold`, `param` or `component` attribute: by GUID or its alias.
    pub(crate) fn holds(attrs: &[(String, String)], guid: &str) -> bool {
        ["hold", "param", "component"].iter().any(|k| {
            attrs
                .iter()
                .find(|(key, _)| key == k)
                .is_some_and(|(_, v)| {
                    v.split(';').any(|p| {
                        let id = p.trim().to_lowercase();
                        let id = id.trim_start_matches("guid:").trim_matches(['{', '}']);
                        id == guid || toggle_guid(id) == Some(guid)
                    })
                })
        })
    }

    /// The windows on screen changed: tells the host when they did since the last time.
    fn tell_windows(&mut self) {
        let ids = self.window_ids();
        if ids != self.told_windows {
            self.told_windows.clone_from(&ids);
            self.host.windows_changed(&ids);
        }
    }

    /// Lays the main window out now, for a script that asks where something is.
    pub fn ensure_layout(&mut self) {
        if self.needs_layout {
            self.do_layout();
        }
    }

    /// Lays out until scripts stop changing the tree (a few passes at most).
    pub(crate) fn do_layout(&mut self) {
        let mut passes = 0;
        loop {
            self.layout_pass();
            passes += 1;
            if !(self.needs_layout && passes < MAX_LAYOUT_PASSES) {
                break;
            }
        }
        self.tell_windows();
    }

    /// The stack's size, in layout pixels; among windows set out apart, the view's (the
    /// viewport's until the host gives a view size).
    pub fn stack_size(&mut self) -> (i32, i32) {
        if self.needs_layout {
            self.do_layout();
        }
        if self.free.on {
            return self.viewport();
        }
        let rects: Vec<Rect> = self
            .windows()
            .into_iter()
            .map(|l| self.window_rect_of(l))
            .collect();
        (
            rects.iter().map(|r| r.right()).max().unwrap_or(0),
            rects.iter().map(|r| r.bottom()).max().unwrap_or(0),
        )
    }

    /// The part of the stack its windows show, in layout pixels: the stack with each window cut
    /// on its right to where it shows and on its bottom to the part Winamp docks and snaps to.
    pub fn drawn_size(&mut self) -> (i32, i32) {
        if self.needs_layout {
            self.do_layout();
        }
        let mut right = 0;
        let mut bottom = 0;
        for l in self.windows() {
            let r = self.window_rect_of(l);
            right = right.max(r.left + self.content_right(l));
            bottom = bottom.max(r.top + self.snapped_size(l).1);
        }
        (right, bottom)
    }

    /// The main window's drawing.
    pub fn draw_main(&mut self) -> Vec<DrawCmd> {
        match self.main_layout() {
            Some(l) => self.draw_layout(l),
            None => Vec::new(),
        }
    }

    /// Every window's drawing with its place in the stack; draw each translated to its rectangle.
    pub fn draw_windows(&mut self) -> Vec<(Rect, Vec<DrawCmd>)> {
        if self.needs_layout {
            self.do_layout();
        }
        self.drawings()
            .into_iter()
            .map(|(l, cmds, _)| (self.window_rect_of(l), self.scaled_cmds(l, cmds)))
            .collect()
    }

    /// Every window's drawing, bottom to top, with the object that drew each command. After a
    /// tick in which only objects that move by themselves changed, the other windows keep their
    /// last drawing.
    fn drawings(&mut self) -> Vec<(ObjId, Vec<DrawCmd>, Vec<Option<ObjId>>)> {
        let moved_only = self.moved_dirty && !self.dirty;
        self.moved_dirty = false;
        let moved = std::mem::take(&mut self.moved_windows);
        let mut out = Vec::new();
        for l in self.z_windows() {
            let kept = self
                .window_drawings
                .get(&l)
                .filter(|_| moved_only && !moved.contains(&l))
                .cloned();
            let (cmds, owners) = kept.unwrap_or_else(|| {
                let drawn = self.draw_layout_owned(l);
                // a window that moves by itself will likely move again: it is not kept
                if moved.contains(&l) {
                    self.window_drawings.remove(&l);
                } else {
                    self.window_drawings.insert(l, drawn.clone());
                }
                drawn
            });
            out.push((l, cmds, owners));
        }
        out
    }

    /// Window `l`'s drawing at its scale.
    fn scaled_cmds(&self, l: ObjId, cmds: Vec<DrawCmd>) -> Vec<DrawCmd> {
        let s = self.layout_scale(l);
        if s == 1.0 {
            return cmds;
        }
        cmds.iter().map(|c| c.scaled(s)).collect()
    }

    /// The windows in the order [`SkinRuntime::draw_windows`] draws them, bottom to top: the
    /// stack's order, or the raised order while the windows are apart, which differs from
    /// [`SkinRuntime::window_ids`] once a window was raised. A host pairs the drawn windows
    /// with these.
    pub fn drawn_window_ids(&mut self) -> Vec<crate::host::WindowId> {
        self.z_windows()
            .into_iter()
            .map(|l| self.window_id_of(l))
            .collect()
    }

    /// Every window's drawing as [`SkinRuntime::draw_windows`] gives it, with the object that
    /// drew each command, for a host that plans what it keeps between frames.
    pub fn draw_windows_owned(&mut self) -> Vec<DrawnWindow> {
        if self.needs_layout {
            self.do_layout();
        }
        self.drawings()
            .into_iter()
            .map(|(l, cmds, owners)| DrawnWindow {
                rect: self.window_rect_of(l),
                cmds: self.scaled_cmds(l, cmds),
                owners,
            })
            .collect()
    }

    /// When the windows only moved since they were last drawn, where each one shows now, bottom
    /// to top as [`SkinRuntime::draw_windows_owned`] gives them, and the move taken as drawn: a
    /// host places its windows again and keeps their drawings. `None` when something needs
    /// drawing, which [`SkinRuntime::draw_windows_owned`] then does.
    pub fn take_placed_only(&mut self) -> Option<Vec<(crate::host::WindowId, Rect)>> {
        if !self.moved_dirty
            || self.dirty
            || self.needs_layout
            || !self.moved_windows.is_empty()
            || !self.tick_changed.is_empty()
        {
            return None;
        }
        let windows = self.z_windows();
        if !windows.iter().all(|l| self.window_drawings.contains_key(l)) {
            return None;
        }
        self.moved_dirty = false;
        Some(
            windows
                .into_iter()
                .map(|l| (self.window_id_of(l), self.window_rect_of(l)))
                .collect(),
        )
    }

    /// The objects the last tick found moving by themselves (a ticker stepping, a visualizer, a
    /// seek thumb), taken once: a host's planner counts them live from their first change.
    pub fn take_tick_changed(&mut self) -> Vec<ObjId> {
        std::mem::take(&mut self.tick_changed)
    }
}
