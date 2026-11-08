// SPDX-License-Identifier: GPL-3.0-or-later

//! What each kind of object does: the attributes it takes, its natural size, its drawing, its
//! touches and the script methods it answers.

use super::SkinRuntime;
use crate::draw::{BitmapDef, FontDef, Rect};
use crate::gui::{parse_rgb, to_bool, to_int_lenient, ButtonKind, Kind, ObjId};

pub(super) const GEOMETRY: [&str; 13] = [
    "x",
    "y",
    "w",
    "h",
    "default_x",
    "default_y",
    "default_w",
    "default_h",
    "relatx",
    "relaty",
    "relatw",
    "relath",
    "fitparent",
];
pub(super) const LABEL_SIZE: i32 = 15;
/// The size of a group that writes none and has no background (Wasabi SDK basewnd).
pub(super) const UNSIZED_W: i32 = 320;
pub(super) const UNSIZED_H: i32 = 200;
pub(super) const LABEL_PAD: i32 = 8;
/// An equalizer slider's lowest position, flat at 0.
pub(super) const EQ_LOW: i32 = -128;
/// The span of a slider's positions unless the skin gives one.
pub(super) const FULL: i32 = 255;
pub(super) const GRID_PARTS: [&str; 9] = [
    "topleft",
    "top",
    "topright",
    "left",
    "middle",
    "right",
    "bottomleft",
    "bottom",
    "bottomright",
];
pub(super) const CENTER_16: i32 = 0x8000;
/// How far the player's balance reaches either way from the middle.
pub(crate) const PAN_REACH: i32 = 127;

pub(super) fn label_font() -> FontDef {
    FontDef::new("arial", "")
}

/// `relat*` as written: "%..." is a percentage.
pub(super) fn relat(value: &str) -> i32 {
    if value.trim_start().starts_with('%') {
        2
    } else {
        to_int_lenient(value).unwrap_or(0)
    }
}

impl SkinRuntime {
    /// Sets every attribute of `map` on `o`; the ones no object handles are counted.
    pub(crate) fn set_params(&mut self, o: ObjId, map: &[(String, String)]) {
        for (k, v) in map {
            if !self.set_param(o, k, v) {
                let tag = self.objects[o].base.tag.clone();
                *self
                    .unhandled_params
                    .entry(format!("{tag} {}", k.to_lowercase()))
                    .or_insert(0) += 1;
            }
        }
    }

    /// Sets attribute `key` on `o`: true when the object took it.
    pub(crate) fn set_param(&mut self, o: ObjId, key: &str, value: &str) -> bool {
        let k = key.to_lowercase();
        self.objects[o].base.set_attr(&k, value);
        if k == "resize" && !self.scripts_running() {
            self.objects[o].base.declared_resize = Some(value.to_string());
        }
        let geometry =
            GEOMETRY.contains(&k.as_str()) || k == "padtitleleft" || k == "padtitleright";
        let placed_before = if geometry {
            Some(self.objects[o].base.placement())
        } else {
            None
        };
        let taken = {
            let b = &mut self.objects[o].base;
            match k.as_str() {
                "id" => b.id = value.to_lowercase(),
                "x" => b.x = to_int_lenient(value),
                "y" => b.y = to_int_lenient(value),
                "w" => b.w = to_int_lenient(value),
                "h" => b.h = to_int_lenient(value),
                "default_w" => b.default_w = to_int_lenient(value),
                "default_h" => b.default_h = to_int_lenient(value),
                "default_x" | "default_y" => {}
                "relatx" => b.relat_x = relat(value),
                "relaty" => b.relat_y = relat(value),
                "relatw" => b.relat_w = relat(value),
                "relath" => b.relat_h = relat(value),
                // fill the parent, a negative value inset by that much on every side
                "fitparent" => {
                    if let Some(v) = to_int_lenient(value).filter(|v| *v != 0) {
                        b.relat_w = 1;
                        b.relat_h = 1;
                        b.x = Some(if v < 0 { -v } else { 0 });
                        b.y = b.x;
                        b.w = Some(if v < 0 { 2 * v } else { 0 });
                        b.h = b.w;
                    }
                }
                "visible" => b.visible = to_bool(value),
                "alpha" => b.set_alpha(
                    to_int_lenient(value)
                        .map(|a| a.clamp(0, 255))
                        .unwrap_or(255),
                ),
                "activealpha" => {
                    b.active_alpha = to_int_lenient(value)
                        .map(|a| a.clamp(0, 255))
                        .unwrap_or(255);
                }
                "inactivealpha" => {
                    b.inactive_alpha = to_int_lenient(value)
                        .map(|a| a.clamp(0, 255))
                        .unwrap_or(255);
                }
                "ghost" => b.ghost = to_bool(value),
                "rectrgn" => b.rect_rgn = to_bool(value),
                "sysregion" | "regionop" => {
                    b.sysregion = to_int_lenient(value).unwrap_or(0);
                    b.sysregion_set = true;
                }
                "tooltip" => b.tooltip = Some(value.to_string()),
                "dblclickaction" => b.dbl_click_action = Some(value.to_string()),
                "enabled" => b.enabled = to_bool(value),
                "padtitleleft" => b.pad_left = to_int_lenient(value).unwrap_or(0),
                "padtitleright" => b.pad_right = to_int_lenient(value).unwrap_or(0),
                "move" => b.move_window = Some(to_bool(value)),
                "name" | "renderbasetexture" | "cursor" | "dropdownlist" | "droptarget"
                | "notify" | "focusonclick" => {}
                _ => return self.set_kind_param(o, &k, value),
            }
            true
        };
        // a new position or size needs a new layout (a script setting the same one again does not),
        // anything else only a redraw
        match placed_before {
            None => self.invalidate(),
            Some(before) => {
                if self.objects[o].base.placement() != before {
                    self.relayout();
                }
            }
        }
        taken
    }

    /// The attributes a kind takes beyond the base ones.
    fn set_kind_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        if matches!(self.objects[o].kind, Kind::XFade(_)) && self.xfade_set_param(o, k, value) {
            return true;
        }
        if self.objects[o].group.is_some() {
            let taken = {
                let g = self.objects[o].group.as_mut().expect("a group");
                match k {
                    "background" => {
                        g.background = Some(value.to_string());
                        g.draw_background = true;
                        true
                    }
                    "drawbackground" => {
                        g.draw_background = to_bool(value);
                        true
                    }
                    "instance_id" | "instanceid" => {
                        g.instance_id = Some(value.to_lowercase());
                        true
                    }
                    "autowidthsource" => {
                        g.auto_width_source = Some(value.to_string());
                        true
                    }
                    "autoheightsource" => {
                        g.auto_height_source = Some(value.to_string());
                        true
                    }
                    "embed_xui" => {
                        g.embed_xui = Some(value.to_lowercase());
                        true
                    }
                    "inherit_group" | "xuitag" | "allowzerosize" | "lockminmax"
                    | "propagatesize" | "design" | "sysregion_offset" => true,
                    _ => false,
                }
            };
            if taken {
                return true;
            }
        }
        match &mut self.objects[o].kind {
            Kind::Unknown { .. } => true,
            Kind::Group => false,
            Kind::Layout(d) => {
                match k {
                    "minimum_w" => d.min_w = to_int_lenient(value).unwrap_or(0),
                    "minimum_h" => d.min_h = to_int_lenient(value).unwrap_or(0),
                    "maximum_w" => d.max_w = to_int_lenient(value).unwrap_or(0),
                    "maximum_h" => d.max_h = to_int_lenient(value).unwrap_or(0),
                    "desktopalpha" => d.desktop_alpha = value == "1",
                    "linkwidth" => d.link_width = Some(value.to_lowercase()),
                    "linkheight" => d.link_height = Some(value.to_lowercase()),
                    "nofocusapponclick" | "owner" | "nodock" | "lockminmax" | "scalable"
                    | "alphabackground" => {}
                    "snapadjustleft" => d.snap_left = to_int_lenient(value).unwrap_or(0),
                    "snapadjusttop" => d.snap_top = to_int_lenient(value).unwrap_or(0),
                    "snapadjustright" => d.snap_right = to_int_lenient(value).unwrap_or(0),
                    "snapadjustbottom" => d.snap_bottom = to_int_lenient(value).unwrap_or(0),
                    _ => return false,
                }
                true
            }
            Kind::Layer(l) => layer_param(l, k, value),
            Kind::AnimatedLayer(l, a) => {
                if layer_param(l, k, value) {
                    return true;
                }
                match k {
                    "framewidth" => {
                        a.frame_w = to_int_lenient(value);
                        a.vertical_set = Some(false);
                    }
                    "frameheight" => {
                        a.frame_h = to_int_lenient(value);
                        a.vertical_set = Some(true);
                    }
                    "start" => a.start_frame = to_int_lenient(value).unwrap_or(0),
                    "end" => a.end_frame = to_int_lenient(value),
                    "speed" => a.speed = to_int_lenient(value).unwrap_or(200),
                    "autoplay" => a.auto_play = to_bool(value),
                    "autoreplay" => a.auto_replay = to_bool(value),
                    "debug" | "realtime" | "elementframes" => {}
                    _ => return false,
                }
                true
            }
            Kind::Button(b) => {
                match k {
                    "image" => b.image = Some(value.to_string()),
                    "downimage" => b.down_image = Some(value.to_string()),
                    "hoverimage" => b.hover_image = Some(value.to_string()),
                    "activeimage" => b.active_image = Some(value.to_string()),
                    "action" => b.action = Some(value.to_string()),
                    "param" => b.param = Some(value.to_string()),
                    "action_target" | "cbtarget" => b.action_target = Some(value.to_string()),
                    "borders" => b.borders = to_bool(value),
                    "text" => b.label = Some(value.to_string()),
                    "center_image" => b.center_image = to_bool(value),
                    "retcode" | "lefttext" | "righttext" => {}
                    _ => {
                        return match &mut b.kind {
                            ButtonKind::Plain => false,
                            ButtonKind::Toggle {
                                cfg_attrib,
                                cfg_val,
                                auto_toggle,
                            } => match k {
                                "cfgattrib" => {
                                    *cfg_attrib = Some(cfg_attrib_of(value));
                                    true
                                }
                                "cfgval" => {
                                    *cfg_val = to_int_lenient(value).unwrap_or(1);
                                    true
                                }
                                "autotoggle" => {
                                    *auto_toggle = to_bool(value);
                                    true
                                }
                                _ => false,
                            },
                            ButtonKind::NStates {
                                states,
                                values,
                                cfg_attrib,
                                auto_elements,
                                ..
                            } => match k {
                                "nstates" => {
                                    *states = to_int_lenient(value).unwrap_or(2).max(1);
                                    true
                                }
                                "cfgvals" => {
                                    *values = Some(
                                        value
                                            .split(';')
                                            .map(|v| to_int_lenient(v).unwrap_or(0))
                                            .collect(),
                                    );
                                    true
                                }
                                "cfgattrib" => {
                                    *cfg_attrib = Some(cfg_attrib_of(value));
                                    true
                                }
                                "autoelements" => {
                                    *auto_elements = to_bool(value);
                                    true
                                }
                                _ => false,
                            },
                        }
                    }
                }
                true
            }
            Kind::Text(_) => self.text_set_param(o, k, value),
            Kind::Slider(s) => {
                match k {
                    "action" => {
                        let action = value.to_lowercase();
                        if action == "eq_band" || action == "eq_preamp" {
                            if !s.low_set {
                                s.low = EQ_LOW;
                            }
                            if !s.high_set {
                                s.high = EQ_LOW + FULL;
                            }
                        }
                        s.action = Some(action);
                    }
                    "param" => s.param = Some(value.to_string()),
                    "thumb" => s.thumb = Some(value.to_string()),
                    "downthumb" => s.down_thumb = Some(value.to_string()),
                    "hoverthumb" => {}
                    "barleft" => s.bar_left = Some(value.to_string()),
                    "barmiddle" => s.bar_middle = Some(value.to_string()),
                    "barright" => s.bar_right = Some(value.to_string()),
                    "orientation" => s.vertical = value.to_lowercase().starts_with('v'),
                    "low" => {
                        s.low = to_int_lenient(value).unwrap_or(0);
                        s.low_set = true;
                    }
                    "high" => {
                        s.high = to_int_lenient(value).unwrap_or(255);
                        s.high_set = true;
                    }
                    // Winamp 5.666 has no thumboffset: a skin's is ignored
                    "cfgattrib" | "cfgval" | "thumboffset" => {}
                    _ => return false,
                }
                true
            }
            Kind::Status(s) => {
                match k {
                    "stopbitmap" => s.stop = Some(value.to_string()),
                    "playbitmap" => s.play = Some(value.to_string()),
                    "pausebitmap" => s.pause = Some(value.to_string()),
                    _ => return false,
                }
                true
            }
            Kind::Grid(g) => grid_param(g, k, value),
            Kind::ProgressGrid(g, p) => {
                if k == "orientation" {
                    p.orientation = value.to_lowercase();
                    return true;
                }
                grid_param(g, k, value)
            }
            Kind::AlbumArt { not_found } => match k {
                "notfoundimage" => {
                    *not_found = Some(value.to_string());
                    true
                }
                "image" | "stretch" | "keepaspect" => true,
                _ => false,
            },
            Kind::RectObj(r) => match k {
                // a skin color by name, else one written out
                "color" => {
                    let named = value.to_string();
                    if let Some(c) = parse_rgb(value) {
                        r.color = c;
                    }
                    let known = self.color(&named).is_some();
                    if let Kind::RectObj(r) = &mut self.objects[o].kind {
                        r.color_id = if known { Some(named) } else { None };
                    }
                    true
                }
                "filled" => {
                    r.filled = to_bool(value);
                    true
                }
                "cursor" => true,
                _ => false,
            },
            Kind::Images(i) => {
                match k {
                    "images" => i.image = Some(value.to_string()),
                    "imagesspacing" => i.spacing = to_int_lenient(value).unwrap_or(0),
                    "source" => i.source = value.to_lowercase(),
                    _ => return false,
                }
                true
            }
            Kind::Menu(m) => {
                match k {
                    "menu" => m.menu = Some(value.to_string()),
                    "normal" => m.normal = Some(value.to_string()),
                    "down" => m.down = Some(value.to_string()),
                    // a mouse moving along the bar; a finger has no hover and no keyboard
                    "hover" | "menugroup" | "next" | "prev" => {}
                    _ => return false,
                }
                true
            }
            Kind::Frame(_) => self.frame_set_param(o, k, value),
            Kind::Bucket(_) => self.bucket_set_param(o, k, value),
            Kind::XFade(_) => false,
            Kind::Vis(_) => self.vis_set_param(o, k, value),
            Kind::ClassicWindow(_) => {
                if k == "window" {
                    if let Some(w) = crate::classic::ClassicWindow::parse(value) {
                        self.objects[o].kind = Kind::ClassicWindow(w);
                    }
                    true
                } else {
                    false
                }
            }
            Kind::EqVis(_) => self.eqvis_set_param(o, k, value),
            Kind::ColorThemeList(_) => self.themes_set_param(o, k, value),
            Kind::Gradient(_) => self.gradient_set_param(o, k, value),
            Kind::PlaylistView(_) => {
                matches!(k, "param" | "noshowcmdbar" | "autoopen" | "autoclose")
            }
            Kind::LayoutStatus(_) => self.status_bar_set_param(o, k, value),
        }
    }

    /// `setXmlParam(key, value)` from a script; a group keeps what it cannot use for its scripts.
    pub(crate) fn set_xml_param(&mut self, o: ObjId, key: &str, value: &str) {
        let k = key.to_lowercase();
        if matches!(self.objects[o].kind, Kind::Text(_)) && self.text_set_xml_param(o, &k, value) {
            return;
        }
        let before = self.objects[o].base.attr(&k).map(|s| s.to_string());
        let paint = self.objects[o].base.paint_key();
        let was_zero = self.objects[o].base.alpha() == 0;
        if self.objects[o].group.is_some() {
            // every attribute lands in attrs, a xui parameter too
            if before.as_deref() != Some(value) {
                self.invalidate();
            }
            let known = self.set_param(o, &k, value);
            if self.objects[o].base.paint_key() != paint {
                self.alpha_changed(o, was_zero);
            }
            if known {
                return;
            }
            // a xui parameter: for the group's scripts, and for the child it exposes
            self.objects[o]
                .group
                .as_mut()
                .expect("a group")
                .xui_params
                .push((k.clone(), value.to_string()));
            self.deliver_xui_params(o);
            if let Some(e) = self.embedded(o) {
                self.set_param(e, &k, value);
            }
            return;
        }
        self.set_param(o, &k, value);
        // what an object keeps for itself (a layer's image) shows on the next frame too
        if before.as_deref() != Some(value) {
            self.invalidate();
        }
        if self.objects[o].base.paint_key() != paint {
            self.alpha_changed(o, was_zero);
        }
    }
}

/// A size from an autowidthsource: its natural size plus its offset, or less its relative w
/// (the room it leaves); none when it is placed from the far side.
pub(super) fn auto_size(
    natural: Option<i32>,
    pos: Option<i32>,
    size: Option<i32>,
    relat_pos: i32,
    relat_size: i32,
) -> Option<i32> {
    let rs = if size.is_none() { 0 } else { relat_size };
    let natural = natural?;
    if relat_pos != 0 || rs > 1 {
        return None;
    }
    Some(if rs == 0 {
        natural + pos.unwrap_or(0)
    } else {
        natural - size.unwrap_or(0)
    })
}

/// `value` out of `whole`, as a share of `range`, rounded to the nearest.
pub(super) fn scaled(value: i32, whole: i32, range: i32) -> i32 {
    (value as f64 * range as f64 / whole as f64).round() as i32
}

pub(super) fn cfg_attrib_of(value: &str) -> String {
    let after = value
        .split_once(';')
        .map(|(_, a)| a.trim().to_lowercase())
        .unwrap_or_default();
    if after.is_empty() {
        value.to_string()
    } else {
        after
    }
}

pub(super) fn layer_param(l: &mut crate::gui::LayerData, k: &str, value: &str) -> bool {
    match k {
        "image" => l.image = Some(value.to_string()),
        "tile" => l.tile = to_int_lenient(value).unwrap_or(0) != 0,
        "inactiveimage" => l.inactive_image = Some(value.to_string()),
        "region" | "scale" | "resize" | "dblclickaction" => {}
        _ => return false,
    }
    true
}

pub(super) fn grid_param(g: &mut crate::gui::GridData, k: &str, value: &str) -> bool {
    if GRID_PARTS.contains(&k) {
        match g.parts.iter_mut().find(|(key, _)| key == k) {
            Some(e) => e.1 = value.to_string(),
            None => g.parts.push((k.to_string(), value.to_string())),
        }
        true
    } else {
        false
    }
}

/// The id of a part of a bitmap font sheet, one cell.
pub(crate) fn glyph(f: &crate::draw::BitmapFontDef, cell: (i32, i32)) -> BitmapDef {
    BitmapDef {
        id: std::rc::Rc::from(format!("{}#{},{}", f.id, cell.0, cell.1).as_str()),
        file: f.file.clone(),
        x: f.ox + cell.0 * f.char_width,
        y: f.oy + cell.1 * f.char_height,
        w: Some(f.char_width),
        h: Some(f.char_height),
        gamma_group: f.gamma_group.clone(),
    }
}

impl SkinRuntime {
    /// Where object `o` is in the stack, in layout pixels: its rectangle in its window, moved by
    /// where the window is. `None` for an object in no window.
    pub(crate) fn stack_rect_of(&mut self, o: ObjId) -> Option<Rect> {
        let l = self.layout_object(o)?;
        let r = self.laid_out_rect(o);
        let at = self.window_rect_of(l);
        Some(Rect::new(
            at.left + r.left,
            at.top + r.top,
            r.width,
            r.height,
        ))
    }

    /// The rectangle after any pending layout.
    pub(crate) fn laid_out_rect(&mut self, o: ObjId) -> Rect {
        self.ensure_layout();
        self.objects[o].base.rect
    }

    /// Where the visualizer shows in the stack, in layout pixels: a shown window holder for
    /// Winamp's visualizer (`hold="guid:avs"`, Winamp Modern's drawer in its visualizer mode) or
    /// for its video, which the host does not play, cut to the groups around it, as they cut
    /// what is drawn inside them. `None` when none shows, or none of it is left uncut (a drawer
    /// shut). The host draws its visualizer there.
    pub fn vis_rect(&mut self) -> Option<Rect> {
        fn find(rt: &SkinRuntime, o: ObjId) -> Option<ObjId> {
            let obj = &rt.objects[o];
            if !obj.base.visible {
                return None;
            }
            if obj.group.is_some() {
                if super::HOLDER_TAGS.contains(&obj.base.tag.as_str())
                    && super::VIS_HOLDS
                        .iter()
                        .any(|g| SkinRuntime::holds(&obj.base.attrs, g))
                {
                    return Some(o);
                }
                return obj.children().iter().find_map(|c| find(rt, *c));
            }
            None
        }
        if self.needs_layout {
            self.do_layout();
        }
        for l in self.windows() {
            if !self.objects[l].base.visible {
                continue;
            }
            let Some(holder) = self.objects[l]
                .children()
                .iter()
                .find_map(|c| find(self, *c))
            else {
                continue;
            };
            let mut r = self.objects[holder].base.rect;
            let mut p = self.objects[holder].base.parent;
            while let Some(g) = p {
                r = r.cut_to(&self.objects[g].base.rect);
                p = self.objects[g].base.parent;
            }
            let (w, h) = self.layout_size(l);
            r = r.cut_to(&Rect::new(0, 0, w, h));
            if r.width <= 0 || r.height <= 0 {
                continue;
            }
            let at = self.window_rect_of(l);
            return Some(Rect::new(
                at.left + r.left,
                at.top + r.top,
                r.width,
                r.height,
            ));
        }
        None
    }
}
