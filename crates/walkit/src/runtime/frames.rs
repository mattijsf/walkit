// SPDX-License-Identifier: GPL-3.0-or-later

//! The groups that arrange their children themselves: a frame's two panes, a bucket's strip of
//! entries, and a crossfade's pages.

use super::SkinRuntime;
use crate::draw::{DrawCmd, Rect};
use crate::gui::{to_bool, to_int_lenient, Kind, ObjId};
use crate::maki::Value;

const SIZER: i32 = 8;
const OPAQUE: i32 = 255;
const FILL: [(&str, &str); 6] = [
    ("x", "0"),
    ("y", "0"),
    ("w", "0"),
    ("h", "0"),
    ("relatw", "1"),
    ("relath", "1"),
];

impl SkinRuntime {
    fn frame(&self, o: ObjId) -> &crate::gui::FrameData {
        match &self.objects[o].kind {
            Kind::Frame(f) => f,
            _ => panic!("not a frame"),
        }
    }

    fn frame_mut(&mut self, o: ObjId) -> &mut crate::gui::FrameData {
        match &mut self.objects[o].kind {
            Kind::Frame(f) => f,
            _ => panic!("not a frame"),
        }
    }

    /// A frame's attributes beyond a group's.
    pub(crate) fn frame_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        match k {
            "orientation" => self.frame_mut(o).vertical = value.to_lowercase().starts_with('v'),
            "from" => {
                let v = value.to_lowercase();
                self.frame_mut(o).from_far = v.starts_with('r') || v.starts_with('b');
            }
            "width" | "height" => self.frame_mut(o).pos = to_int_lenient(value).unwrap_or(0),
            "minwidth" | "minheight" => {
                self.frame_mut(o).min_pos = to_int_lenient(value).unwrap_or(0)
            }
            "maxwidth" | "maxheight" => self.frame_mut(o).max_pos = to_int_lenient(value),
            "left" | "top" | "right" | "bottom" => {
                let pane = self.instantiate_group(value);
                if let Some(p) = pane {
                    self.add_child(o, p);
                }
                if k == "left" || k == "top" {
                    self.frame_mut(o).first = pane;
                } else {
                    self.frame_mut(o).second = pane;
                }
            }
            "vbitmap" | "hbitmap" => {
                self.frame_mut(o).divider = Some(if value.eq_ignore_ascii_case("empty") {
                    String::new()
                } else {
                    value.to_string()
                })
            }
            "vgrabber" | "hgrabber" => {
                self.frame_mut(o).grabber = Some(if value.eq_ignore_ascii_case("empty") {
                    String::new()
                } else {
                    value.to_string()
                })
            }
            "resizable" | "resizeable" => self.frame_mut(o).resizable = to_bool(value),
            "jump" => {}
            _ => return false,
        }
        true
    }

    /// Lays a frame's panes out: the divider `pos` pixels from the `from` side, kept between the
    /// minimum and maximum, taking 8 px; one child hidden, the other takes the whole frame.
    pub(crate) fn frame_lay_out_children(&mut self, o: ObjId) {
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        let f = self.frame(o).clone();
        let total = if f.vertical { rect.width } else { rect.height };
        let others: Vec<ObjId> = self.objects[o]
            .children()
            .iter()
            .copied()
            .filter(|c| Some(*c) != f.first && Some(*c) != f.second)
            .collect();
        if let (Some(a), Some(b)) = (f.first, f.second) {
            let (va, vb) = (self.objects[a].base.visible, self.objects[b].base.visible);
            if va != vb {
                self.layout_in(if va { a } else { b }, rect, alpha);
                for c in others {
                    self.lay_out(c, rect, alpha);
                }
                return;
            }
        }
        let sizer = if f.resizable { SIZER } else { 0 };
        let mut p = f.pos;
        if let Some(m) = f.max_pos {
            p = p.min(if m < 0 { total + m } else { m });
        }
        let top = (total - sizer).max(0);
        p = p.clamp(f.min_pos.min(top), top);
        // vertical: the divider comes out of the pane the position is not measured from; horizontal:
        // the top pane always gets the position's share, the bottom one what is left after the
        // divider (Wasabi SDK FrameWnd::onResize)
        let first_len = if !f.from_far {
            p
        } else if f.vertical {
            total - p - sizer
        } else {
            total - p
        };
        let second_start = first_len + sizer;
        let span = |start: i32, len: i32| -> Rect {
            if f.vertical {
                Rect::new(rect.left + start, rect.top, len, rect.height)
            } else {
                Rect::new(rect.left, rect.top + start, rect.width, len)
            }
        };
        self.frame_mut(o).sizer_rect = span(first_len, sizer);
        let second_len = (total - second_start).max(0);
        let a = span(0, first_len);
        let b = span(second_start, second_len);
        if let Some(first) = f.first {
            self.layout_in(first, a, alpha);
        }
        if let Some(second) = f.second {
            self.layout_in(second, b, alpha);
        }
        for c in others {
            self.lay_out(c, rect, alpha);
        }
    }

    /// The divider, stretched over the 8 px between the panes and 2 px past each end, its
    /// grabber centered on it when the divider is longer.
    pub(crate) fn frame_draw_divider(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let f = self.frame(o).clone();
        if !self.objects[o].base.visible || !f.resizable {
            return;
        }
        let r = f.sizer_rect;
        if r.width <= 0 || r.height <= 0 {
            return;
        }
        if let (Some(a), Some(b)) = (f.first, f.second) {
            if self.objects[a].base.visible != self.objects[b].base.visible {
                return;
            }
        }
        let alpha = self.objects[o].base.effective_alpha;
        let bar = f.divider.clone().unwrap_or_else(|| {
            if f.vertical {
                "wasabi.framewnd.verticaldivider"
            } else {
                "wasabi.framewnd.horizontaldivider"
            }
            .to_string()
        });
        let grip = f.grabber.clone().unwrap_or_else(|| {
            if f.vertical {
                "wasabi.framewnd.verticalgrabber"
            } else {
                "wasabi.framewnd.horizontalgrabber"
            }
            .to_string()
        });
        let long = if f.vertical {
            Rect::new(r.left, r.top - 2, r.width, r.height + 4)
        } else {
            Rect::new(r.left - 2, r.top, r.width + 4, r.height)
        };
        if !bar.is_empty() {
            if let Some(bm) = self.bitmap(&bar) {
                out.push(DrawCmd::image(bm, long, alpha));
            }
        }
        if grip.is_empty() {
            return;
        }
        let Some((gw, gh)) = self.bitmap_size(&grip) else {
            return;
        };
        let len = if f.vertical { long.height } else { long.width };
        let glen = if f.vertical { gh } else { gw };
        if len <= glen {
            return;
        }
        let cut = (len - glen) / 2;
        let g = if f.vertical {
            Rect::new(long.left, long.top + cut, long.width, len - 2 * cut)
        } else {
            Rect::new(long.left + cut, long.top, len - 2 * cut, long.height)
        };
        if let Some(bm) = self.bitmap(&grip) {
            out.push(DrawCmd::image(bm, g, alpha));
        }
    }

    /// A frame's divider position.
    pub(crate) fn frame_position(&self, o: ObjId) -> i32 {
        self.frame(o).pos
    }

    /// Moves a frame's divider.
    pub(crate) fn frame_set_position(&mut self, o: ObjId, pos: i32) {
        self.frame_mut(o).pos = pos;
    }

    // ---- buckets -----------------------------------------------------------------------------

    fn bucket(&self, o: ObjId) -> &crate::gui::BucketData {
        match &self.objects[o].kind {
            Kind::Bucket(b) => b,
            _ => panic!("not a bucket"),
        }
    }

    fn bucket_mut(&mut self, o: ObjId) -> &mut crate::gui::BucketData {
        match &mut self.objects[o].kind {
            Kind::Bucket(b) => b,
            _ => panic!("not a bucket"),
        }
    }

    /// A bucket's attributes beyond a group's.
    pub(crate) fn bucket_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        match k {
            "wndtype" => self.bucket_mut(o).window_type = Some(value.trim().to_lowercase()),
            "vertical" => self.bucket_mut(o).vertical = to_bool(value),
            "spacing" => self.bucket_mut(o).spacing = to_int_lenient(value).unwrap_or(0),
            "leftmargin" => self.bucket_mut(o).left_margin = to_int_lenient(value).unwrap_or(0),
            "rightmargin" => self.bucket_mut(o).right_margin = to_int_lenient(value).unwrap_or(0),
            _ => return false,
        }
        true
    }

    /// Puts the groups of the bucket's window type in the strip, from its start.
    pub(crate) fn bucket_fill(&mut self, o: ObjId) {
        let ids = self
            .bucket(o)
            .window_type
            .as_ref()
            .and_then(|t| self.window_types.get(t).cloned())
            .unwrap_or_default();
        for id in ids {
            if let Some(g) = self.instantiate_group(&id) {
                self.add_child(o, g);
            }
        }
        self.bucket_place(o);
    }

    fn bucket_length(&mut self, o: ObjId, c: ObjId) -> i32 {
        let vertical = self.bucket(o).vertical;
        let natural = self.intrinsic_size(c);
        let b = &self.objects[c].base;
        if vertical {
            b.h.or(natural.map(|s| s.1))
        } else {
            b.w.or(natural.map(|s| s.0))
        }
        .unwrap_or(0)
    }

    fn bucket_content_length(&mut self, o: ObjId) -> i32 {
        let children = self.objects[o].children().to_vec();
        let b = self.bucket(o).clone();
        let mut sum = 0;
        for c in &children {
            sum += self.bucket_length(o, *c);
        }
        b.left_margin + sum + b.spacing * (children.len() as i32 - 1).max(0) + b.right_margin
    }

    fn bucket_view_length(&mut self, o: ObjId) -> i32 {
        let r = self.laid_out_rect(o);
        if self.bucket(o).vertical {
            r.height
        } else {
            r.width
        }
    }

    fn bucket_place(&mut self, o: ObjId) {
        let b = self.bucket(o).clone();
        let mut at = b.left_margin - b.scroll;
        for c in self.objects[o].children().to_vec() {
            let len = self.bucket_length(o, c);
            let base = &mut self.objects[c].base;
            if b.vertical {
                base.x = Some(0);
                base.y = Some(at);
            } else {
                base.x = Some(at);
                base.y = Some(0);
            }
            at += len + b.spacing;
        }
        self.relayout();
    }

    /// How far a bucket's entries are scrolled.
    pub(crate) fn bucket_scroll(&self, o: ObjId) -> i32 {
        self.bucket(o).scroll
    }

    /// Scrolls a bucket's entries to `to`, within what fits.
    pub(crate) fn bucket_scroll_to(&mut self, o: ObjId, to: i32) {
        let max = (self.bucket_content_length(o) - self.bucket_view_length(o)).max(0);
        let s = to.clamp(0, max);
        if s == self.bucket(o).scroll {
            return;
        }
        self.bucket_mut(o).scroll = s;
        self.bucket_place(o);
    }

    /// One page on (`direction` 1) or back (-1): as many whole entries as the strip shows.
    pub(crate) fn bucket_page(&mut self, o: ObjId, direction: i32) {
        let Some(first) = self.objects[o].children().first().copied() else {
            return;
        };
        let one = self.bucket_length(o, first) + self.bucket(o).spacing;
        if one <= 0 {
            return;
        }
        let shown = ((self.bucket_view_length(o) as f64 / one as f64).ceil() as i32).max(1);
        let to = self.bucket(o).scroll + direction * shown * one;
        self.bucket_scroll_to(o, to);
    }

    /// A bucket's content length along the strip, or its size across it.
    pub(crate) fn bucket_max(&mut self, o: ObjId, width: bool) -> i32 {
        let vertical = self.bucket(o).vertical;
        if width == vertical {
            let r = self.laid_out_rect(o);
            if width {
                r.width
            } else {
                r.height
            }
        } else {
            self.bucket_content_length(o)
        }
    }

    /// The bucket a page button's action targets, else one beside the button.
    pub(crate) fn bucket_for(&mut self, source: ObjId, target: Option<&str>) -> Option<ObjId> {
        if let Some(t) = target {
            let windows = self.windows();
            if let Some(found) = windows.iter().find_map(|l| self.find_object(*l, t)) {
                if matches!(self.objects[found].kind, Kind::Bucket(_)) {
                    return Some(found);
                }
                return None;
            }
        }
        let parent = self.objects[source].base.parent?;
        self.objects[parent]
            .children()
            .iter()
            .copied()
            .find(|c| matches!(self.objects[*c].kind, Kind::Bucket(_)))
    }

    /// Tells each waiting bucket onStartup once a script listens to it: a bucket's script picks
    /// its first entry from here (Winamp Modern's options menu), and that script may load after
    /// the bucket is made, with the group it sits in.
    pub(crate) fn start_buckets(&mut self) {
        if self.starting_buckets.is_empty() {
            return;
        }
        let ready: Vec<ObjId> = self
            .starting_buckets
            .iter()
            .copied()
            .filter(|b| {
                self.scripts
                    .iter()
                    .any(|s| s.listens_to(crate::maki::Handle::Gui(*b)))
            })
            .collect();
        if ready.is_empty() {
            return;
        }
        self.starting_buckets.retain(|b| !ready.contains(b));
        for b in ready {
            self.dispatch(crate::maki::Handle::Gui(b), "onstartup", &[]);
        }
    }

    // ---- crossfades --------------------------------------------------------------------------

    /// A crossfade's attributes beyond a group's; `groupid` shows a page.
    pub(crate) fn xfade_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        match k {
            "groupid" | "group" => self.xfade_show(o, value.trim()),
            "speed" => {
                if let Ok(v) = value.parse::<f64>() {
                    if let Kind::XFade(x) = &mut self.objects[o].kind {
                        x.speed = v;
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn xfade_show(&mut self, o: ObjId, id: &str) {
        let key = id.to_lowercase();
        let Kind::XFade(x) = &self.objects[o].kind else {
            return;
        };
        let before = x.current;
        let known = x.pages.iter().find(|(k, _)| *k == key).map(|(_, p)| *p);
        if before.is_some() && known == before {
            return;
        }
        let page = if key.is_empty() {
            None
        } else {
            match known {
                Some(p) => Some(p),
                None => {
                    let made = self.instantiate_group(id);
                    if let Some(p) = made {
                        let fill: Vec<(String, String)> = FILL
                            .iter()
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .collect();
                        self.set_params(p, &fill);
                        if let Kind::XFade(x) = &mut self.objects[o].kind {
                            x.pages.push((key.clone(), p));
                        }
                    }
                    made
                }
            }
        };
        // only the page going and the page coming are drawn
        let pages: Vec<ObjId> = match &self.objects[o].kind {
            Kind::XFade(x) => x.pages.iter().map(|(_, p)| *p).collect(),
            _ => Vec::new(),
        };
        for p in pages {
            if Some(p) != before && Some(p) != page {
                self.objects[p].base.visible = false;
            }
        }
        if let Some(b) = before {
            self.xfade_fade(o, b, 0);
        }
        if let Some(p) = page {
            self.objects[p].base.set_alpha(0);
            self.objects[p].base.visible = true;
            // on top of the page going
            self.add_child(o, p);
            self.xfade_fade(o, p, OPAQUE);
        }
        if let Kind::XFade(x) = &mut self.objects[o].kind {
            x.current = page;
        }
        self.relayout();
    }

    fn xfade_fade(&mut self, o: ObjId, g: ObjId, to: i32) {
        let speed = match &self.objects[o].kind {
            Kind::XFade(x) => x.speed,
            _ => 0.25,
        };
        self.gui_maki_call(g, "settargeta", &[Value::Int(to)]);
        self.gui_maki_call(g, "settargetspeed", &[Value::Num(speed)]);
        self.gui_maki_call(g, "gototarget", &[]);
    }

    /// The page a crossfade shows now.
    pub(crate) fn xfade_current(&self, o: ObjId) -> Option<ObjId> {
        match &self.objects[o].kind {
            Kind::XFade(x) => x.current,
            _ => None,
        }
    }
}
