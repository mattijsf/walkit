// SPDX-License-Identifier: GPL-3.0-or-later

//! Laying the windows out: the stack, each layout's size and place, every object's rectangle
//! and alpha, and the regions the window shapes are made of.

use super::SkinRuntime;
use crate::draw::{DrawCmd, Rect, Region, RegionImage};
use crate::gui::{Behavior, Kind, ObjId};
use crate::host::Edge;
use crate::maki::{Handle, Value};

/// A window no screen could show is cut to the largest picture size.
pub(crate) const MAX_SIDE: i32 = 8_192;

/// 1 adds the parent's size, 2 takes a percentage of it.
fn place(v: i32, relat: i32, size: i32) -> i32 {
    match relat {
        1 => v.wrapping_add(size),
        2 => (size as f64 * (v as f64 / 100.0)) as i32,
        _ => v,
    }
}

/// The edge a window starting at `at`, `size` long, sits against in a viewport `extent` long; `None` for neither.
fn edge_at(at: i32, size: i32, extent: i32) -> Option<Edge> {
    if at == 0 {
        Some(Edge::Start)
    } else if at.wrapping_add(size) == extent {
        Some(Edge::End)
    } else {
        None
    }
}

/// Where a window `size` long starts against `edge` of a viewport `extent` long; `None` for no edge.
fn start_at(edge: Option<Edge>, size: i32, extent: i32) -> Option<i32> {
    match edge {
        Some(Edge::Start) => Some(0),
        Some(Edge::End) => Some(extent - size),
        None => None,
    }
}

/// Where a window `size` long starts on an axis of a viewport `extent` long, against neither
/// edge: in the middle; where the middle would touch an edge, at the larger of 1 and one pixel
/// past its place against the far edge.
fn clear_of(size: i32, extent: i32) -> i32 {
    let room = extent - size;
    if room >= 2 {
        room / 2
    } else {
        room.max(0) + 1
    }
}

impl SkinRuntime {
    /// One layout pass: the stack arranged, every window laid out, the objects found, and the
    /// scripts told what moved.
    pub(crate) fn layout_pass(&mut self) {
        self.layout_passes += 1;
        self.needs_layout = false;
        self.change_count += 1;
        self.pointer.skin_menus.clear();
        self.arrange();
        let windows = self.windows();
        for l in &windows {
            self.layout_root(*l);
        }
        let mut all = Vec::new();
        let mut sliders = Vec::new();
        let mut texts = Vec::new();
        let mut vis = Vec::new();
        let mut grids = Vec::new();
        let mut buttons = Vec::new();
        for l in &windows {
            self.walk_found(
                *l,
                &mut all,
                &mut sliders,
                &mut texts,
                &mut vis,
                &mut grids,
                &mut buttons,
            );
        }
        let changed = all != self.all_objects;
        self.all_objects = all.clone();
        self.sliders = sliders;
        self.texts = texts;
        self.vis_objects = vis;
        self.progress_grids = grids;
        self.buttons = buttons;
        // an object gone from every window, such as a group a script dropped, is let go of
        if changed {
            let keys: Vec<ObjId> = self.last_rects.keys().copied().collect();
            for k in keys {
                let in_window = self
                    .layout_object(k)
                    .and_then(|l| self.objects[l].layout().and_then(|d| d.container))
                    .is_some();
                if !in_window {
                    self.last_rects.remove(&k);
                }
            }
        }
        // the player's sliders show where the player is from their first frame
        for o in &all {
            if matches!(self.objects[*o].kind, Kind::Slider(_)) {
                self.slider_place_if_unsynced(*o);
            }
        }
        let starting = self.take_pending_starts();
        if !starting.is_empty() {
            for s in starting {
                self.start_script(s);
            }
            // scripts that just started have not heard any sizes yet
            self.last_rects.clear();
        }
        if self.scripts_started {
            self.start_buckets();
        }
        for o in &all {
            if self.objects[*o].group.is_some() {
                self.deliver_xui_params(*o);
            }
        }
        for o in &all {
            if self.objects[*o]
                .group
                .as_ref()
                .is_some_and(|g| g.behavior.is_some())
            {
                self.behavior_after_layout(*o);
            }
        }
        // objects whose place or size changed hear about it, as Wasabi calls onResize on every move
        if self.scripts_started {
            for o in all {
                let r = self.objects[o].base.rect;
                if self.last_rects.get(&o) != Some(&r) {
                    self.last_rects.insert(o, r);
                    self.dispatch(
                        Handle::Gui(o),
                        "onresize",
                        &[
                            Value::Int(r.left),
                            Value::Int(r.top),
                            Value::Int(r.width),
                            Value::Int(r.height),
                        ],
                    );
                }
            }
        }
    }

    /// Every object of a window, in drawing order, and apart those that move with the player or the clock.
    #[allow(clippy::too_many_arguments)]
    fn walk_found(
        &self,
        o: ObjId,
        all: &mut Vec<ObjId>,
        sliders: &mut Vec<ObjId>,
        texts: &mut Vec<ObjId>,
        vis: &mut Vec<ObjId>,
        grids: &mut Vec<ObjId>,
        buttons: &mut Vec<ObjId>,
    ) {
        all.push(o);
        match &self.objects[o].kind {
            Kind::Slider(_) => sliders.push(o),
            Kind::Text(_) => texts.push(o),
            Kind::ProgressGrid(..) => grids.push(o),
            Kind::Button(_) => buttons.push(o),
            Kind::Vis(_) => vis.push(o),
            _ => {}
        }
        for c in self.objects[o].children() {
            self.walk_found(*c, all, sliders, texts, vis, grids, buttons);
        }
    }

    /// Sets the windows out: apart when the host asks for that (see `free`), else as one stack.
    fn arrange(&mut self) {
        if self.free.on {
            self.arrange_free();
        } else {
            self.arrange_stack();
        }
    }

    /// Stacks the windows: the main one at its size, the others under it.
    pub(crate) fn arrange_stack(&mut self) {
        self.placed.clear();
        let Some(main) = self.main_layout() else {
            return;
        };
        let others: Vec<ObjId> = self.windows().into_iter().skip(1).collect();
        let (mw, mh) = self.layout_size(main);
        self.placed.insert(main, Rect::new(0, 0, mw, mh));
        self.layout_root(main);
        let Some(view) = self.view_size else {
            // without a screen size (the reference tests) every window keeps the size it has
            let mut y = self.content_bottom(main);
            for l in others {
                let (w, h) = self.layout_size(l);
                self.placed.insert(l, Rect::new(0, y, w, h));
                y += h;
            }
            return;
        };
        let (columns, column_width) = (self.columns, self.column_width);
        for column in self.columns_of(main, &others, columns, column_width) {
            let mut y = column.top;
            let last = column.windows.len().saturating_sub(1);
            for (i, l) in column.windows.iter().copied().enumerate() {
                // a classic playlist filling the view leaves the size its grip gave it for apart
                let fills = self.stretch_last && i == last && self.is_classic_playlist(l);
                if let Some((sw, sh)) = self.objects[l]
                    .layout()
                    .and_then(|d| d.user_size)
                    .filter(|_| !fills)
                {
                    // resized by its grip: the size the finger gave it
                    self.objects[l].base.w = Some(sw);
                    self.objects[l].base.h = Some(sh);
                    let (w, h) = self.layout_size(l);
                    self.placed.insert(l, Rect::new(column.x, y, w, h));
                    y += h;
                    continue;
                }
                self.objects[l].base.w = Some(column.width);
                let natural = self.natural_height(l);
                let left = view.1 - y;
                let stretched = if self.stretch_last && i == last && left > natural {
                    self.classic_stretch(l, left).or_else(|| {
                        (self.resizes_vertically(l) && self.shows_list(l)).then_some(left)
                    })
                } else {
                    None
                };
                self.objects[l].base.h = stretched.or_else(|| self.skin_height(l));
                let (w, h) = self.layout_size(l);
                self.placed.insert(l, Rect::new(column.x, y, w, h));
                y += h;
            }
        }
    }

    /// The columns `others` dock in beside or under `main`: one under what is visible of the
    /// main window, as wide as what shows of it; and with `columns` at 2 and a list window among
    /// them, a second for the list windows, from the top, `column_width` wide (0 for as wide as
    /// the first), starting where the main window and every window left in the first one end.
    pub(crate) fn columns_of(
        &mut self,
        main: ObjId,
        others: &[ObjId],
        columns: i32,
        column_width: i32,
    ) -> Vec<Column> {
        let under = self.content_bottom(main);
        let width = self.content_right(main);
        let (lists, rest): (Vec<ObjId>, Vec<ObjId>) =
            others.iter().copied().partition(|l| self.shows_list(*l));
        if columns != 2 || lists.is_empty() {
            return vec![Column {
                x: 0,
                top: under,
                width,
                windows: others.to_vec(),
            }];
        }
        let right = rest
            .iter()
            .map(|l| self.size_at(*l, width).0)
            .max()
            .unwrap_or(0);
        let main_w = self.layout_size(main).0;
        let cw = if column_width > 0 {
            column_width
        } else {
            width
        };
        vec![
            Column {
                x: 0,
                top: under,
                width,
                windows: rest,
            },
            Column {
                x: main_w.max(right),
                top: 0,
                width: cw,
                windows: lists,
            },
        ]
    }

    /// `window`'s size when docked `width` wide at the height the skin gives it; it is left as it was.
    pub(crate) fn size_at(&mut self, window: ObjId, width: i32) -> (i32, i32) {
        let kept_w = self.objects[window].base.w;
        let kept_h = self.objects[window].base.h;
        let sized = self.objects[window].layout().and_then(|d| d.user_size);
        self.objects[window].base.w = Some(sized.map(|s| s.0).unwrap_or(width));
        self.objects[window].base.h = sized.map(|s| s.1).or_else(|| self.skin_height(window));
        let size = self.layout_size(window);
        self.objects[window].base.w = kept_w;
        self.objects[window].base.h = kept_h;
        size
    }

    /// How tall the stack makes `window` when it docks it with a view, unless it is the last
    /// list window and takes the height left.
    pub(crate) fn natural_height(&mut self, window: ObjId) -> i32 {
        let kept = self.objects[window].base.h;
        self.objects[window].base.h = self.skin_height(window);
        let h = self.layout_size(window).1;
        self.objects[window].base.h = kept;
        h
    }

    /// The height the skin's XML gives `window`, or `None` when it gives none.
    fn skin_height(&self, window: ObjId) -> Option<i32> {
        self.objects[window]
            .base
            .attr("h")
            .and_then(crate::gui::to_int_lenient)
    }

    /// The height a classic playlist `l` takes when it stretches into `left`: all of it, at least
    /// 116, so the stack reaches the bottom of the view (Winamp draws the last run of its sides
    /// shortened); `None` for any other window.
    fn classic_stretch(&self, l: ObjId, left: i32) -> Option<i32> {
        self.is_classic_playlist(l).then(|| left.max(116))
    }

    /// Whether `l` is the layout of Winamp's classic playlist.
    fn is_classic_playlist(&self, l: ObjId) -> bool {
        self.objects[l].children().iter().any(|c| {
            matches!(
                self.objects[*c].kind,
                Kind::ClassicWindow(crate::classic::ClassicWindow::Playlist)
            )
        })
    }

    /// Whether `l` shows a list, which shows more rows when the window is taller.
    pub(crate) fn shows_list(&self, l: ObjId) -> bool {
        fn holds_list(attrs: &[(String, String)]) -> bool {
            super::LIST_GUIDS
                .iter()
                .any(|g| SkinRuntime::holds(attrs, g))
        }
        fn walk(rt: &SkinRuntime, o: ObjId) -> bool {
            let obj = &rt.objects[o];
            if matches!(obj.kind, Kind::PlaylistView(_) | Kind::ColorThemeList(_)) {
                return true;
            }
            if obj.group.is_some() {
                (super::HOLDER_TAGS.contains(&obj.base.tag.as_str()) && holds_list(&obj.base.attrs))
                    || obj.children().iter().any(|c| walk(rt, *c))
            } else {
                false
            }
        }
        let container_holds = self.objects[l]
            .layout()
            .and_then(|d| d.container)
            .is_some_and(|c| holds_list(&self.containers[c].attrs));
        container_holds || self.objects[l].children().iter().any(|c| walk(self, *c))
    }

    // ---- a layout's size and place ---------------------------------------------------------

    /// The size the object's attributes give: its w/h, else its default size, background or 320 x 200.
    fn own_size(&mut self, l: ObjId) -> (i32, i32) {
        let bg = self.intrinsic_size(l);
        let b = &self.objects[l].base;
        let d = self.objects[l].layout().expect("a layout");
        (
            b.w.filter(|w| *w > 0)
                .or(bg.map(|s| s.0))
                .unwrap_or(d.min_w),
            b.h.filter(|h| *h > 0)
                .or(bg.map(|s| s.1))
                .unwrap_or(d.min_h),
        )
    }

    /// Size for the screen: the layout's size, a width (height) not written taken from the layout it
    /// links to, then kept within the minimum and maximum, except while it moves to a target size.
    pub(crate) fn layout_size(&mut self, l: ObjId) -> (i32, i32) {
        let own = self.own_size(l);
        let (container, link_width, link_height) = {
            let d = self.objects[l].layout().expect("a layout");
            (d.container, d.link_width.clone(), d.link_height.clone())
        };
        let (w_set, h_set) = (
            self.objects[l].base.w.is_some(),
            self.objects[l].base.h.is_some(),
        );
        let linked = |rt: &mut SkinRuntime, id: Option<&str>| -> Option<(i32, i32)> {
            let other = rt.container_layout(container?, id?).filter(|o| *o != l)?;
            Some(rt.own_size(other))
        };
        let mut width = if w_set {
            own.0
        } else {
            linked(self, link_width.as_deref())
                .map(|s| s.0)
                .unwrap_or(own.0)
        };
        let mut height = if h_set {
            own.1
        } else {
            linked(self, link_height.as_deref())
                .map(|s| s.1)
                .unwrap_or(own.1)
        };
        let d = self.objects[l].layout().expect("a layout");
        if !self.moving.contains(&l) {
            if d.min_w > 0 {
                width = width.max(d.min_w);
            }
            if d.min_h > 0 {
                height = height.max(d.min_h);
            }
            if d.max_w > 0 {
                width = width.min(d.max_w);
            }
            if d.max_h > 0 {
                height = height.min(d.max_h);
            }
        }
        (width.clamp(0, MAX_SIDE), height.clamp(0, MAX_SIDE))
    }

    /// Where the window visibly ends: the lowest edge of its innermost shaped groups that are
    /// shown. The layout's height when it has none.
    pub(crate) fn content_bottom(&mut self, l: ObjId) -> i32 {
        let at = self.change_count;
        if self.objects[l].layout().expect("a layout").bottom_at != at {
            let b = self.work_out_bottom(l);
            let d = self.objects[l].layout_mut().expect("a layout");
            d.bottom_now = b;
            d.bottom_at = at;
        }
        self.objects[l].layout().expect("a layout").bottom_now
    }

    fn work_out_bottom(&mut self, l: ObjId) -> i32 {
        fn walk(rt: &SkinRuntime, o: ObjId, bottom: &mut i32) -> bool {
            let obj = &rt.objects[o];
            if !obj.base.visible || obj.group.is_none() {
                return false;
            }
            let mut inner = false;
            for c in obj.children() {
                if walk(rt, *c, bottom) {
                    inner = true;
                }
            }
            if obj.base.sysregion == 1 && obj.base.rect.height > 0 {
                if !inner {
                    *bottom = (*bottom).max(obj.base.rect.bottom());
                }
                return true;
            }
            inner
        }
        let mut bottom = -1;
        for c in self.objects[l].children().to_vec() {
            walk(self, c, &mut bottom);
        }
        let h = self.layout_size(l).1;
        if bottom > 0 {
            bottom.min(h)
        } else {
            h
        }
    }

    /// The window's shape: the layout's region with its children's, less every whole-window
    /// cut. `None` for a window with no shape at all, which shows as its rectangle.
    pub(crate) fn window_shape(&mut self, l: ObjId) -> (Option<Region>, i64) {
        let at = self.change_count;
        if self.objects[l].layout().expect("a layout").shape_at != at {
            let composed = self.composed(l);
            let cuts = self.objects[l].layout().expect("a layout").cuts.clone();
            let shape = if composed.is_none() && cuts.is_empty() {
                None
            } else {
                let (w, h) = self.layout_size(l);
                Some(Region::Op {
                    base: Some(Box::new(
                        composed.unwrap_or(Region::Box(Rect::new(0, 0, w, h))),
                    )),
                    add: Vec::new(),
                    keep: Vec::new(),
                    cut: cuts,
                })
            };
            let d = self.objects[l].layout_mut().expect("a layout");
            d.shape = shape;
            d.shape_at = at;
        }
        let d = self.objects[l].layout().expect("a layout");
        (d.shape.clone(), d.shape_at)
    }

    /// The layout's region with its children's, and the whole-window cuts, worked out once for each change.
    fn composed(&mut self, l: ObjId) -> Option<Region> {
        let at = self.change_count;
        if self.objects[l].layout().expect("a layout").composed_at != at {
            self.shapes_worked_out += 1;
            let mut cuts = Vec::new();
            let composed = self.composed_region(l, &mut cuts);
            let d = self.objects[l].layout_mut().expect("a layout");
            d.composed = composed;
            d.cuts = cuts;
            d.composed_at = at;
        }
        self.objects[l].layout().expect("a layout").composed.clone()
    }

    /// Where the window visibly ends on the right: the right edge of the parts its children add
    /// to its shape, or of its background where that shows further, at most its width.
    pub(crate) fn content_right(&mut self, l: ObjId) -> i32 {
        let at = self.change_count;
        if self.objects[l].layout().expect("a layout").right_at != at {
            let width = self.layout_size(l).0;
            let r = if self.objects[l].layout().expect("a layout").desktop_alpha {
                width
            } else {
                let shape = self.composed(l);
                self.work_out_right(shape, width)
            };
            let d = self.objects[l].layout_mut().expect("a layout");
            d.right_now = r;
            d.right_at = at;
        }
        self.objects[l].layout().expect("a layout").right_now
    }

    fn work_out_right(&mut self, shape: Option<Region>, width: i32) -> i32 {
        let Some(Region::Op { base, add, .. }) = shape else {
            return width;
        };
        let Some(right) = add
            .iter()
            .filter_map(|r| r.bounds().map(|b| b.right()))
            .max()
        else {
            return width;
        };
        if right >= width {
            return width;
        }
        let background = match base.as_deref() {
            Some(Region::Box(r)) => r.right(),
            Some(Region::Pic(img)) => self.opaque_right(&img.bitmap, img.dst),
            _ => 0,
        };
        right.max(background).clamp(1, width)
    }

    /// The part of the window Winamp docks and snaps to: its size less the right and bottom
    /// snap margins, but never less than what it shows now.
    pub(crate) fn snapped_size(&mut self, l: ObjId) -> (i32, i32) {
        let (width, height) = self.layout_size(l);
        let (sr, sb) = {
            let d = self.objects[l].layout().expect("a layout");
            (d.snap_right, d.snap_bottom)
        };
        let right = if sr > 0 {
            (width - sr).max(self.content_right(l))
        } else {
            width
        };
        let bottom = if sb > 0 {
            (height - sb).max(self.content_bottom(l))
        } else {
            height
        };
        (right.clamp(0, width), bottom.clamp(0, height))
    }

    /// A script's snapAdjust: new margins, then onSnapAdjustChanged when they changed.
    pub(crate) fn snap_adjust(&mut self, l: ObjId, left: i32, top: i32, right: i32, bottom: i32) {
        let d = self.objects[l].layout_mut().expect("a layout");
        if [d.snap_left, d.snap_top, d.snap_right, d.snap_bottom] == [left, top, right, bottom] {
            return;
        }
        d.snap_left = left;
        d.snap_top = top;
        d.snap_right = right;
        d.snap_bottom = bottom;
        self.invalidate();
        self.dispatch(Handle::Gui(l), "onsnapadjustchanged", &[]);
    }

    /// Whether the user can change the window's height: a handle on a top or bottom edge.
    pub(crate) fn resizes_vertically(&self, l: ObjId) -> bool {
        self.has_handle(l, &["top", "bottom"], false)
    }

    /// Whether the skin's XML gives the window handles both ways, whatever its scripts set later.
    pub(crate) fn declared_resizable_both_ways(&self, l: ObjId) -> bool {
        self.has_handle(l, &["left", "right"], true) && self.has_handle(l, &["top", "bottom"], true)
    }

    fn has_handle(&self, l: ObjId, edges: &[&str], declared: bool) -> bool {
        fn walk(rt: &SkinRuntime, o: ObjId, edges: &[&str], declared: bool) -> bool {
            let b = &rt.objects[o].base;
            let r = if declared {
                b.declared_resize.as_deref()
            } else {
                b.attr("resize")
            }
            .map(|s| s.to_lowercase());
            if let Some(r) = r {
                if edges.iter().any(|e| r.contains(e)) {
                    return true;
                }
            }
            rt.objects[o]
                .children()
                .iter()
                .any(|c| walk(rt, *c, edges, declared))
        }
        self.objects[l]
            .children()
            .iter()
            .any(|c| walk(self, *c, edges, declared))
    }

    /// Lays the window out at its size.
    pub(crate) fn layout_root(&mut self, l: ObjId) {
        let (width, height) = self.layout_size(l);
        let rect = Rect::new(0, 0, width, height);
        let alpha = self.objects[l].base.alpha();
        self.objects[l].base.rect = rect;
        self.objects[l].base.effective_alpha = alpha;
        for c in self.objects[l].children().to_vec() {
            self.lay_out(c, rect, alpha);
        }
    }

    /// The alphas of `layout_root` worked out again for the window's focus, with nothing moved or resized.
    pub(crate) fn refresh_alphas(&mut self, l: ObjId) {
        let alpha = self.objects[l].base.alpha();
        self.objects[l].base.effective_alpha = alpha;
        for c in self.objects[l].children().to_vec() {
            self.refresh_alpha(c, alpha);
        }
    }

    /// Works out the alpha `o` and every object inside it paint with again, under a parent
    /// painting with `parent_alpha`, as a layout pass does but moving nothing.
    pub(crate) fn refresh_alpha(&mut self, o: ObjId, parent_alpha: i32) {
        let a = self.paint_alpha(o, parent_alpha);
        self.objects[o].base.effective_alpha = a;
        for c in self.objects[o].children().to_vec() {
            self.refresh_alpha(c, a);
        }
    }

    /// The alpha `o` paints with: the one for the window's focus, times the parent's, 254 shown
    /// as 255 and 1 as 0, and a disabled object at 60 %.
    pub(crate) fn paint_alpha(&mut self, o: ObjId, parent_alpha: i32) -> i32 {
        let layout = self.layout_object(o);
        let focused = self.has_focus(layout);
        let b = &self.objects[o].base;
        let mut a = if focused {
            b.active_alpha
        } else {
            b.inactive_alpha
        };
        a = parent_alpha * a / 255;
        if a == 254 {
            a = 255;
        } else if a == 1 {
            a = 0;
        }
        if b.enabled {
            a
        } else {
            a * 6 / 10
        }
    }

    /// After `o`'s alphas or enabled changed with nothing moved: it and what it holds paint with
    /// the new alpha from the next frame, with no layout pass. An alpha to or from 0 also
    /// changes its window's shape.
    pub(crate) fn alpha_changed(&mut self, o: ObjId, was_zero: bool) {
        if self.objects[o].is_layout() {
            self.refresh_alphas(o);
        } else {
            let pa = self.objects[o]
                .base
                .parent
                .map(|p| self.objects[p].base.effective_alpha)
                .unwrap_or(255);
            self.refresh_alpha(o, pa);
        }
        if was_zero == (self.objects[o].base.alpha() == 0) {
            self.repaint();
        } else {
            self.invalidate();
        }
    }

    /// Lays `o` out in `parent_rect` under `parent_alpha`.
    pub(crate) fn lay_out(&mut self, o: ObjId, parent_rect: Rect, parent_alpha: i32) {
        let pw = parent_rect.width;
        let ph = parent_rect.height;
        let b = &self.objects[o].base;
        let left = place(b.x.unwrap_or(0), b.relat_x, pw);
        let top = place(b.y.unwrap_or(0), b.relat_y, ph);
        let (w, h, rw, rh, pl, pr) = (b.w, b.h, b.relat_w, b.relat_h, b.pad_left, b.pad_right);
        let needs_intrinsic = w.is_none() || h.is_none();
        let intrinsic = if needs_intrinsic {
            self.intrinsic_size(o)
        } else {
            None
        };
        // a size not written is the natural one, whatever relat* says; a written 0 is 0
        let width = w
            .map(|v| place(v, rw, pw))
            .or(intrinsic.map(|s| s.0))
            .unwrap_or(0);
        let height = h
            .map(|v| place(v, rh, ph))
            .or(intrinsic.map(|s| s.1))
            .unwrap_or(0);
        let rect = Rect::new(
            parent_rect.left.wrapping_add(left).wrapping_add(pl),
            parent_rect.top.wrapping_add(top),
            (width.wrapping_sub(pl).wrapping_sub(pr)).max(0),
            height,
        );
        self.objects[o].base.rect = rect;
        let alpha = self.paint_alpha(o, parent_alpha);
        self.objects[o].base.effective_alpha = alpha;
        if matches!(self.objects[o].kind, Kind::Frame(_)) {
            self.frame_lay_out_children(o);
        } else if self.objects[o].group.is_some() {
            for c in self.objects[o].children().to_vec() {
                self.lay_out(c, rect, alpha);
            }
        }
    }

    /// Lays group `g` out to fill `area` exactly, as a frame pane; the group's position attributes do not apply.
    pub(crate) fn layout_in(&mut self, g: ObjId, area: Rect, parent_alpha: i32) {
        self.objects[g].base.rect = area;
        let alpha = self.paint_alpha(g, parent_alpha);
        self.objects[g].base.effective_alpha = alpha;
        for c in self.objects[g].children().to_vec() {
            self.lay_out(c, area, alpha);
        }
    }

    /// Puts the window at `x`, `y` in this layout with no script moving it: no edges are noted.
    pub(crate) fn place_layout_at(&mut self, l: ObjId, x: i32, y: i32) {
        let d = self.objects[l].layout_mut().expect("a layout");
        d.view_x = x;
        d.view_y = y;
    }

    /// Moves the window to `x`, `y` in this layout as a script does, and notes the edges of the
    /// viewport it then sits against.
    pub(crate) fn layout_move_to(&mut self, l: ObjId, x: i32, y: i32, width: i32, height: i32) {
        self.place_layout_at(l, x, y);
        let (vw, vh) = self.viewport();
        let d = self.objects[l].layout_mut().expect("a layout");
        d.edge_x = edge_at(x, width, vw);
        d.edge_y = edge_at(y, height, vh);
        let at_edge = d.edge_x.is_some() || d.edge_y.is_some();
        if Some(l) == self.main_layout() && at_edge {
            self.main_placed_at_edge();
        }
    }

    /// Puts the window where a phone draws it in a new viewport: against the edges noted, and
    /// on an axis with none, in the middle for a `middle` window, or where it was for another.
    pub(crate) fn layout_fit_viewport(&mut self, l: ObjId, middle: bool) {
        let (w, h) = self.layout_size(l);
        let (vw, vh) = self.viewport();
        let d = self.objects[l].layout().expect("a layout");
        let x =
            start_at(d.edge_x, w, vw).unwrap_or(if middle { clear_of(w, vw) } else { d.view_x });
        let y =
            start_at(d.edge_y, h, vh).unwrap_or(if middle { clear_of(h, vh) } else { d.view_y });
        self.place_layout_at(l, x, y);
    }

    /// Sizes the window to `width` by `height` where it is, as the phone sizes a window to its box.
    pub(crate) fn resize_layout_to(&mut self, l: ObjId, width: i32, height: i32) {
        self.objects[l].base.w = Some(width);
        self.objects[l].base.h = Some(height);
        self.relayout();
    }

    // ---- regions -----------------------------------------------------------------------------

    /// How `o`'s region joins its parent's: 1 added, 2 intersected, -1 cut, -2 cut from the whole window.
    pub(crate) fn region_op(&self, o: ObjId) -> i32 {
        let obj = &self.objects[o];
        if obj.group.is_some() {
            // a group that wrote no sysregion shapes the window when one of its children does
            if obj.base.sysregion_set || obj.is_layout() {
                obj.base.sysregion
            } else {
                i32::from(obj.children().iter().any(|c| self.region_op(*c) != 0))
            }
        } else {
            obj.base.sysregion
        }
    }

    /// `o`'s region before its children's.
    pub(crate) fn own_region(&mut self, o: ObjId) -> Option<Region> {
        let rect = self.objects[o].base.rect;
        if rect.width <= 0 || rect.height <= 0 {
            return None;
        }
        match &self.objects[o].kind {
            Kind::Layer(_) | Kind::AnimatedLayer(..) => self.layer_own_region(o),
            Kind::ClassicWindow(w) => {
                let w = *w;
                self.classic_region(o, w).or(Some(Region::Box(rect)))
            }
            _ if self.objects[o].group.is_some() => {
                // its rectangle with rectrgn, else its background's opaque pixels stretched over it, else nothing
                if self.objects[o].base.rect_rgn {
                    return Some(Region::Box(rect));
                }
                let bg = self.objects[o]
                    .group
                    .as_ref()
                    .and_then(|g| g.background.clone())?;
                let bm = self.bitmap(&bg)?;
                // a background stretched to the group (its bitmap another size, in either axis)
                // gives a region one pixel bigger at the top and the left, right and bottom
                // exact; one at the group's size gives an exact region
                let stretched = self
                    .bitmap_size(&bg)
                    .is_some_and(|(w, h)| w != rect.width || h != rect.height);
                let over = if stretched {
                    Rect::new(rect.left - 1, rect.top - 1, rect.width + 1, rect.height + 1)
                } else {
                    rect
                };
                Some(Region::Pic(RegionImage {
                    bitmap: bm,
                    dst: over,
                    alpha: 255,
                    gamma: None,
                    smooth: false,
                }))
            }
            // its rectangle (Wasabi SDK basewnd, rectrgn on by default)
            _ => Some(Region::Box(rect)),
        }
    }

    /// The opaque pixels of what a layer draws; its rectangle only with a rectrgn written after
    /// the sysregion, as a sysregion turns a layer's rectrgn off.
    fn layer_own_region(&mut self, o: ObjId) -> Option<Region> {
        let b = &self.objects[o].base;
        let keys: Vec<&str> = b.attrs.iter().map(|(k, _)| k.as_str()).collect();
        let index = |k: &str| {
            keys.iter()
                .position(|x| *x == k)
                .map(|i| i as i64)
                .unwrap_or(-1)
        };
        let rect_after = b.rect_rgn && index("rectrgn") > index("sysregion").max(index("regionop"));
        if rect_after || (b.sysregion == 0 && b.rect_rgn) {
            return Some(Region::Box(b.rect));
        }
        let mut drawn = Vec::new();
        let tracking = std::mem::replace(&mut self.tracking_owners, false);
        self.draw_object(o, &mut drawn);
        self.tracking_owners = tracking;
        let mut pics: Vec<Region> = drawn
            .into_iter()
            .filter_map(|c| match c {
                DrawCmd::Image { bitmap, dst, .. } => Some(Region::Pic(RegionImage {
                    bitmap,
                    dst,
                    alpha: 255,
                    gamma: None,
                    smooth: false,
                })),
                _ => None,
            })
            .collect();
        match pics.len() {
            0 => None,
            1 => pics.pop(),
            _ => Some(Region::Op {
                base: None,
                add: pics,
                keep: Vec::new(),
                cut: Vec::new(),
            }),
        }
    }

    /// `o`'s region with its children's joined in by their region op; children's whole-window
    /// cuts go to `window_cuts`. `None` when it is empty.
    pub(crate) fn composed_region(
        &mut self,
        o: ObjId,
        window_cuts: &mut Vec<Region>,
    ) -> Option<Region> {
        if self.objects[o].group.is_none() {
            return self.own_region(o);
        }
        let mut add = Vec::new();
        let mut keep = Vec::new();
        let mut cut = Vec::new();
        for c in self.objects[o].children().to_vec() {
            // hidden or faded out, it is no part of the shape
            let b = &self.objects[c].base;
            if !b.visible || b.alpha() == 0 {
                continue;
            }
            let Some(r) = self.composed_region(c, window_cuts) else {
                continue;
            };
            match self.region_op(c) {
                1 => add.push(r),
                2 => keep.push(r),
                -1 => cut.push(r),
                -2 => window_cuts.push(r),
                _ => {}
            }
        }
        let base = self.own_region(o);
        if base.is_none() && add.is_empty() {
            return None;
        }
        Some(Region::Op {
            base: base.map(Box::new),
            add,
            keep,
            cut,
        })
    }

    // ---- behaviors ---------------------------------------------------------------------------

    /// A default-set group's behavior hears a xui parameter.
    pub(crate) fn behavior_xui_param(&mut self, g: ObjId, key: &str, value: &str) {
        let Some(behavior) = self.objects[g]
            .group
            .as_ref()
            .and_then(|d| d.behavior.clone())
        else {
            return;
        };
        match behavior {
            Behavior::StandardFrame { area } => match key {
                "content" => {
                    let Some(content) = self.instantiate_group(value) else {
                        self.host.log(&format!("standard frame: no group {value}"));
                        return;
                    };
                    self.set_param(content, "x", &area[0].to_string());
                    self.set_param(content, "y", &area[1].to_string());
                    self.set_param(content, "w", &area[2].to_string());
                    self.set_param(content, "h", &area[3].to_string());
                    self.set_param(content, "relatw", "1");
                    self.set_param(content, "relath", "1");
                    self.init_into(content, g);
                }
                "padtitleleft" | "padtitleright" => {
                    if let Some(bar) = self
                        .find_object(g, "wasabi.titlebar")
                        .filter(|t| self.objects[*t].group.is_some())
                    {
                        self.set_xml_param(bar, key, value);
                    }
                }
                "shade" => {
                    if let Some(trap) = self
                        .find_object(g, "wasabi.titlebar")
                        .filter(|t| self.objects[*t].group.is_some())
                        .and_then(|t| self.find_object(t, "mousetrap"))
                    {
                        self.set_param(trap, "dblclickaction", &format!("switch;{value}"));
                    }
                }
                _ => {}
            },
            Behavior::TitleBar {
                pad_left,
                pad_right,
            } => {
                let add = crate::gui::to_int_lenient(value).unwrap_or(0);
                let (pl, pr) = match key {
                    "padtitleleft" => (pad_left + add, pad_right),
                    "padtitleright" => (pad_left, pad_right + add),
                    _ => return,
                };
                if let Some(d) = self.objects[g].group.as_mut() {
                    d.behavior = Some(Behavior::TitleBar {
                        pad_left: pl,
                        pad_right: pr,
                    });
                }
                self.relayout();
            }
        }
    }

    /// A default-set group's behavior runs after every layout pass.
    fn behavior_after_layout(&mut self, g: ObjId) {
        let Some(Behavior::TitleBar {
            pad_left,
            pad_right,
        }) = self.objects[g]
            .group
            .as_ref()
            .and_then(|d| d.behavior.clone())
        else {
            return;
        };
        let Some(layout) = self.layout_object(g) else {
            return;
        };
        let Some(title) = self.embedded(g) else {
            return;
        };
        let Some((text_w, _)) = self.intrinsic_size(title) else {
            return;
        };
        // centered on the window, then counted from the bar's left edge
        let lr = self.objects[layout].base.rect;
        let gr = self.objects[g].base.rect;
        let x = (lr.width - text_w) / 2 + lr.left - gr.left;
        self.title_place(title, x, text_w, false);
        if let Some(s) = self.find_object(g, "wasabi.titlebar.streak.left") {
            self.title_place(s, pad_left, x - pad_left, false);
        }
        if let Some(s) = self.find_object(g, "wasabi.titlebar.streak.right") {
            self.title_place(s, x + text_w + 1, -(x + text_w + 1 + pad_right + 1), true);
        }
    }

    /// Sets x and w (relative to the bar's width when `relative`) only when they change: each change lays out again.
    fn title_place(&mut self, o: ObjId, x: i32, w: i32, relative: bool) {
        let rel = relative as i32;
        let b = &self.objects[o].base;
        if b.x == Some(x) && b.w == Some(w) && b.relat_x == 0 && b.relat_w == rel {
            return;
        }
        self.set_param(o, "x", &x.to_string());
        self.set_param(o, "relatx", "0");
        self.set_param(o, "w", &w.to_string());
        self.set_param(o, "relatw", &rel.to_string());
    }
}

/// A column of docked windows: they hang from `top`, one under the other, at `x`, `width` wide.
pub(crate) struct Column {
    pub(crate) x: i32,
    pub(crate) top: i32,
    pub(crate) width: i32,
    pub(crate) windows: Vec<ObjId>,
}
