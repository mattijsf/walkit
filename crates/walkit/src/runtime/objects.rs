// SPDX-License-Identifier: GPL-3.0-or-later

//! What each kind of object does: the attributes it takes, its natural size, its drawing, its
//! touches and the script methods it answers.

use super::SkinRuntime;
use crate::draw::{Align, BitmapDef, DrawCmd, FontDef, Rect, Rgb, TextCmd, VAlign};
use crate::gui::{parse_rgb, to_bool, to_int_lenient, ButtonKind, Kind, ObjId};
use crate::maki::{Handle, Value};
use crate::player::Status;

const GEOMETRY: [&str; 13] = [
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
const LABEL_SIZE: i32 = 15;
/// The size of a group that writes none and has no background (Wasabi SDK basewnd).
const UNSIZED_W: i32 = 320;
const UNSIZED_H: i32 = 200;
const LABEL_PAD: i32 = 8;
/// An equalizer slider's lowest position, flat at 0.
const EQ_LOW: i32 = -128;
/// The span of a slider's positions unless the skin gives one.
const FULL: i32 = 255;
const GRID_PARTS: [&str; 9] = [
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
const CENTER_16: i32 = 0x8000;
/// How far the player's balance reaches either way from the middle.
pub(crate) const PAN_REACH: i32 = 127;

fn label_font() -> FontDef {
    FontDef::new("arial", "")
}

/// `relat*` as written: "%..." is a percentage.
fn relat(value: &str) -> i32 {
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
                        .unwrap_or(255)
                }
                "inactivealpha" => {
                    b.inactive_alpha = to_int_lenient(value)
                        .map(|a| a.clamp(0, 255))
                        .unwrap_or(255)
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

    // ---- natural sizes -----------------------------------------------------------------------

    /// The natural size when the XML gives none: an image's size, mostly.
    pub(crate) fn intrinsic_size(&mut self, o: ObjId) -> Option<(i32, i32)> {
        if self.objects[o].group.is_some() {
            return self.group_intrinsic_size(o);
        }
        match self.objects[o].kind.clone() {
            Kind::Layer(l) => l.image.and_then(|i| self.bitmap_size(&i)),
            Kind::AnimatedLayer(l, _) => {
                if l.image.as_ref().and_then(|i| self.bitmap_size(i)).is_none() {
                    None
                } else {
                    Some(self.anim_frame_size(o))
                }
            }
            Kind::Button(b) => {
                let picture = self.button_picture(o, b.image.as_deref());
                if let Some(s) = picture.and_then(|p| self.bitmap_size(&p)) {
                    return Some(s);
                }
                if matches!(b.kind, ButtonKind::NStates { .. }) {
                    return None;
                }
                b.label.map(|t| {
                    (
                        self.host
                            .text_width(&t, Some(&label_font()), LABEL_SIZE, false)
                            + LABEL_PAD,
                        0,
                    )
                })
            }
            Kind::Text(_) => Some(self.text_intrinsic_size(o)),
            Kind::Slider(s) => s
                .bar_middle
                .and_then(|b| self.bitmap_size(&b))
                .or_else(|| s.thumb.and_then(|t| self.bitmap_size(&t))),
            Kind::Status(s) => Some(
                s.play
                    .and_then(|p| self.bitmap_size(&p))
                    .unwrap_or((16, 16)),
            ),
            Kind::Grid(g) | Kind::ProgressGrid(g, _) => {
                let mid = self.grid_part_size(&g, "middle");
                if mid == (0, 0) {
                    None
                } else {
                    Some((
                        self.grid_part_size(&g, "left").0
                            + mid.0
                            + self.grid_part_size(&g, "right").0,
                        mid.1,
                    ))
                }
            }
            _ => None,
        }
    }

    fn group_intrinsic_size(&mut self, g: ObjId) -> Option<(i32, i32)> {
        let data = self.objects[g].group.as_ref()?.clone();
        let bg = data.background.as_ref().and_then(|b| self.bitmap_size(b));
        // the source's own size plus what its relative w/h leaves around it
        let aw = data
            .auto_width_source
            .as_ref()
            .and_then(|s| self.find_object(g, s))
            .and_then(|src| {
                let natural = self.intrinsic_size(src).map(|s| s.0);
                let b = &self.objects[src].base;
                auto_size(natural, b.x, b.w, b.relat_x, b.relat_w)
            });
        let ah = data
            .auto_height_source
            .as_ref()
            .and_then(|s| self.find_object(g, s))
            .and_then(|src| {
                let natural = self.intrinsic_size(src).map(|s| s.1);
                let b = &self.objects[src].base;
                auto_size(natural, b.y, b.h, b.relat_y, b.relat_h)
            });
        let b = &self.objects[g].base;
        // then its background's size, then Wasabi's 320 x 200
        Some((
            aw.or(b.default_w).or(bg.map(|s| s.0)).unwrap_or(UNSIZED_W),
            ah.or(b.default_h).or(bg.map(|s| s.1)).unwrap_or(UNSIZED_H),
        ))
    }

    fn grid_part_size(&mut self, g: &crate::gui::GridData, part: &str) -> (i32, i32) {
        g.parts
            .iter()
            .find(|(k, _)| k == part)
            .and_then(|(_, v)| self.bitmap_size(v))
            .unwrap_or((0, 0))
    }

    // ---- animated layers ---------------------------------------------------------------------

    fn anim(&self, o: ObjId) -> (&crate::gui::LayerData, &crate::gui::AnimData) {
        match &self.objects[o].kind {
            Kind::AnimatedLayer(l, a) => (l, a),
            _ => panic!("not an animated layer"),
        }
    }

    fn anim_mut(&mut self, o: ObjId) -> &mut crate::gui::AnimData {
        match &mut self.objects[o].kind {
            Kind::AnimatedLayer(_, a) => a,
            _ => panic!("not an animated layer"),
        }
    }

    fn anim_full_size(&mut self, o: ObjId) -> Option<(i32, i32)> {
        let image = self.anim(o).0.image.clone()?;
        self.bitmap_size(&image)
    }

    /// Whether the frames run down the picture; with neither framewidth nor frameheight written,
    /// a picture as wide as the object runs down and any other across. `None` for a picture the
    /// object's size: one frame.
    pub(crate) fn anim_vertical(&mut self, o: ObjId) -> Option<bool> {
        if let Some(v) = self.anim(o).1.vertical_set {
            return Some(v);
        }
        let Some(full) = self.anim_full_size(o) else {
            return Some(true);
        };
        let b = &self.objects[o].base;
        let ow = b.w.unwrap_or(full.0);
        let oh = b.h.unwrap_or(full.1);
        if full.0 == ow && full.1 != oh {
            Some(true)
        } else if full.0 != ow {
            Some(false)
        } else {
            None
        }
    }

    pub(crate) fn anim_frame_size(&mut self, o: ObjId) -> (i32, i32) {
        let Some(full) = self.anim_full_size(o) else {
            return (0, 0);
        };
        let vertical = self.anim_vertical(o);
        let b = &self.objects[o].base;
        let ow = b.w.unwrap_or(full.0);
        let oh = b.h.unwrap_or(full.1);
        let a = self.anim(o).1;
        // across the strip a frame is as large as the picture
        match vertical {
            Some(true) => (full.0, a.frame_h.unwrap_or(oh)),
            Some(false) => (a.frame_w.unwrap_or(ow), full.1),
            None => (ow, oh),
        }
    }

    /// How many frames the strip holds.
    pub(crate) fn anim_length(&mut self, o: ObjId) -> i32 {
        let Some(full) = self.anim_full_size(o) else {
            return 0;
        };
        let (fw, fh) = self.anim_frame_size(o);
        match self.anim_vertical(o) {
            Some(true) => full.1 / fh.max(1),
            Some(false) => full.0 / fw.max(1),
            None => 1,
        }
    }

    /// Shows frame `n`, and scripts hear `onFrame`. Going to the frame it already shows asks for no new picture.
    pub(crate) fn anim_goto_frame(&mut self, o: ObjId, n: i32) {
        let moved = n != self.anim(o).1.frame;
        self.anim_mut(o).frame = n;
        if moved {
            self.anim_frame_changed(o);
        }
        self.dispatch(Handle::Gui(o), "onframe", &[Value::Int(n)]);
    }

    /// Asks for a frame for a new picture of the strip: only while it shows, and with its
    /// window's shape worked out again only when the strip is part of it.
    fn anim_frame_changed(&mut self, o: ObjId) {
        if self.region_op(o) != 0 {
            self.invalidate();
        } else if self.shows(o) {
            self.repaint();
        }
    }

    pub(crate) fn anim_play(&mut self, o: ObjId) {
        let length = self.anim_length(o);
        let a = self.anim_mut(o);
        let end = a.end_frame.unwrap_or(length - 1);
        a.step = if a.start_frame < end { 1 } else { -1 };
        a.paused = false;
        let start = a.start_frame;
        let auto_replay = a.auto_replay;
        self.anim_goto_frame(o, start);
        self.dispatch(Handle::Gui(o), "onplay", &[]);
        if start == end && !auto_replay {
            self.anim_stop(o);
            return;
        }
        let now = self.now();
        let a = self.anim_mut(o);
        a.playing = true;
        a.next_at = now + a.speed as i64;
        if !self.animations.contains(&o) {
            self.animations.push(o);
        }
    }

    pub(crate) fn anim_stop(&mut self, o: ObjId) {
        if self.anim(o).1.playing {
            self.anim_mut(o).playing = false;
            self.animations.retain(|a| *a != o);
        }
        self.dispatch(Handle::Gui(o), "onstop", &[]);
    }

    /// One step of a playing strip; called by the runtime when `next_at` is due.
    pub(crate) fn anim_advance(&mut self, o: ObjId) {
        if self.anim(o).1.paused {
            return;
        }
        let length = self.anim_length(o);
        let a = self.anim(o).1;
        let end = a.end_frame.unwrap_or(length - 1);
        if a.frame == end {
            if !a.auto_replay {
                self.anim_stop(o);
                return;
            }
            // back to the start without onFrame; a strip that starts where it ends asks for no new picture
            let moved = a.frame != a.start_frame;
            let start = a.start_frame;
            self.anim_mut(o).frame = start;
            if moved {
                self.anim_frame_changed(o);
            }
        } else {
            let next = a.frame + a.step;
            self.anim_goto_frame(o, next);
        }
        // on the beat, catching up ticks that came late
        let now = self.now();
        let a = self.anim_mut(o);
        let speed = a.speed.max(1) as i64;
        a.next_at += speed;
        if a.next_at < now - speed * 8 {
            a.next_at = now;
        }
    }

    pub(crate) fn anim_start_if_auto(&mut self, o: ObjId) {
        let a = self.anim(o).1;
        if a.auto_play && !a.playing {
            self.anim_play(o);
        }
    }

    // ---- buttons -----------------------------------------------------------------------------

    fn button(&self, o: ObjId) -> &crate::gui::ButtonData {
        match &self.objects[o].kind {
            Kind::Button(b) => b,
            _ => panic!("not a button"),
        }
    }

    fn button_mut(&mut self, o: ObjId) -> &mut crate::gui::ButtonData {
        match &mut self.objects[o].kind {
            Kind::Button(b) => b,
            _ => panic!("not a button"),
        }
    }

    /// The bitmap id a picture attribute draws: itself, or for a multi-state button its state's.
    fn button_picture(&mut self, o: ObjId, base: Option<&str>) -> Option<String> {
        let base = base?;
        match &self.button(o).kind {
            ButtonKind::NStates {
                auto_elements,
                states,
                ..
            } if *auto_elements && *states >= 2 => {
                let state = self.button_state(o);
                Some(format!("{base}{state}"))
            }
            _ => Some(base.to_string()),
        }
    }

    /// An n-states button's current state.
    fn button_state(&mut self, o: ObjId) -> i32 {
        let ButtonKind::NStates {
            states,
            values,
            cfg_attrib,
            local_state,
            ..
        } = self.button(o).kind.clone()
        else {
            return 0;
        };
        let value_of = |state: i32| {
            values
                .as_ref()
                .and_then(|v| v.get(state as usize).copied())
                .unwrap_or(state)
        };
        if let Some(attrib) = cfg_attrib {
            let v = self.config_value(&attrib);
            return (0..states).find(|s| value_of(*s) == v).unwrap_or(0);
        }
        let action = self.button(o).action.clone();
        if !self.has_action_state(action.as_deref()) {
            return local_state;
        }
        let param = self.button(o).param.clone();
        let open = self.action_state(action.as_deref(), param.as_deref());
        (0..states)
            .find(|s| (value_of(*s) != 0) == open)
            .unwrap_or(0)
    }

    /// An n-states button's config value for its current state.
    pub(crate) fn button_cfg_value(&mut self, o: ObjId) -> i32 {
        let state = self.button_state(o);
        match &self.button(o).kind {
            ButtonKind::NStates { values, .. } => values
                .as_ref()
                .and_then(|v| v.get(state as usize).copied())
                .unwrap_or(state),
            _ => 0,
        }
    }

    /// Whether the thing this button toggles is on: an open window, a player switch.
    pub(crate) fn button_is_active(&mut self, o: ObjId) -> bool {
        let b = self.button(o).clone();
        match &b.kind {
            ButtonKind::Toggle {
                cfg_attrib: Some(attrib),
                cfg_val,
                ..
            } => {
                if *cfg_val == 1 {
                    self.config_flag(attrib)
                } else {
                    self.config_value(attrib) == *cfg_val
                }
            }
            ButtonKind::NStates { values, .. } => {
                let state = self.button_state(o);
                values
                    .as_ref()
                    .and_then(|v| v.get(state as usize).copied())
                    .unwrap_or(state)
                    != 0
            }
            _ => b.activated || self.action_state(b.action.as_deref(), b.param.as_deref()),
        }
    }

    /// The state its picture shows: active (1) or not (0), or an n-states button's state.
    fn button_shown_state(&mut self, o: ObjId) -> i32 {
        if matches!(self.button(o).kind, ButtonKind::NStates { .. }) {
            self.button_state(o)
        } else {
            self.button_is_active(o) as i32
        }
    }

    /// Whether the shown state moved, for a window of the app, since the last frame drew the
    /// button or this last answered. Each change is told once.
    pub(crate) fn button_state_changed(&mut self, o: ObjId) -> bool {
        let follows_host = self
            .button(o)
            .action
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case("TOGGLE"));
        if !follows_host {
            return false;
        }
        let Some(was) = self.button(o).drawn_state else {
            return false;
        };
        let now = self.button_shown_state(o);
        if now == was {
            return false;
        }
        self.button_mut(o).drawn_state = Some(now);
        true
    }

    fn draw_button(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let b = self.button(o).clone();
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if b.borders {
            self.draw_framed_button(out, rect, alpha, b.pressed);
        }
        if let Some(t) = &b.label {
            // pressed, the label sits 1 px right and down (Wasabi SDK buttwnd)
            let shift = if b.pressed && b.borders { 1 } else { 0 };
            out.push(DrawCmd::Text(Box::new(TextCmd {
                text: t.clone(),
                font: Some(label_font()),
                size: LABEL_SIZE,
                color: Rgb(0),
                rect: Rect::new(
                    rect.left + 1 + shift,
                    rect.top + 1 + shift,
                    rect.width - 2,
                    rect.height - 2,
                ),
                align: Align::Center,
                scroll: 0,
                alpha,
                valign: VAlign::Center,
                bold: false,
                shadow: None,
                shadow_dx: 1,
                shadow_dy: 1,
                x: 0.0,
                baseline: 0.0,
            })));
        }
        let shown = self.button_shown_state(o);
        self.button_mut(o).drawn_state = Some(shown);
        let active = self.button_is_active(o);
        let img = if b.pressed {
            self.button_picture(o, b.down_image.as_deref())
        } else {
            None
        }
        .or(if active { b.active_image.clone() } else { None })
        .or_else(|| self.button_picture(o, b.image.as_deref()));
        let Some(img) = img else { return };
        let Some(bmp) = self.bitmap(&img) else { return };
        // a centered picture sits in a box the normal picture's size, 1 px right and down while pressed
        let size = self
            .button_picture(o, b.image.as_deref())
            .and_then(|p| self.bitmap_size(&p))
            .or_else(|| self.bitmap_size(&img));
        let shift = if b.pressed && self.objects[o].base.enabled {
            1
        } else {
            0
        };
        let dst = match size {
            Some((w, h)) if b.center_image => Rect::new(
                rect.left + (rect.width - w) / 2 + shift,
                rect.top + (rect.height - h) / 2 + shift,
                w,
                h,
            ),
            _ => rect,
        };
        out.push(DrawCmd::image(bmp, dst, alpha));
    }

    /// Winamp's own button frame, as it draws the buttons it makes itself and `borders="1"`
    /// buttons: a black outline round the default set's `wasabi.button` picture (or its pressed
    /// one), stretched from 2 px corners.
    pub(crate) fn draw_framed_button(
        &mut self,
        out: &mut Vec<DrawCmd>,
        r: Rect,
        alpha: i32,
        pressed: bool,
    ) {
        out.push(DrawCmd::Fill {
            rect: r,
            color: Rgb(0),
            alpha,
        });
        let base = if pressed {
            "wasabi.button.pressed"
        } else {
            "wasabi.button"
        };
        let corner = 2;
        let l = r.left + 1;
        let t = r.top + 1;
        let mw = r.width - 2 - 2 * corner;
        let mh = r.height - 2 - 2 * corner;
        let mut put = |rt: &mut SkinRuntime, part: &str, x: i32, y: i32, w: i32, h: i32| {
            if w <= 0 || h <= 0 {
                return;
            }
            if let Some(bm) = rt
                .bitmap(&format!("{base}.{part}"))
                .or_else(|| rt.bitmap(&format!("wasabi.button.{part}")))
            {
                out.push(DrawCmd::image(bm, Rect::new(x, y, w, h), alpha));
            }
        };
        put(self, "top.left", l, t, corner, corner);
        put(self, "top", l + corner, t, mw, corner);
        put(self, "top.right", l + corner + mw, t, corner, corner);
        put(self, "left", l, t + corner, corner, mh);
        put(self, "middle", l + corner, t + corner, mw, mh);
        put(self, "right", l + corner + mw, t + corner, corner, mh);
        put(self, "bottom.left", l, t + corner + mh, corner, corner);
        put(self, "bottom", l + corner, t + corner + mh, mw, corner);
        put(
            self,
            "bottom.right",
            l + corner + mw,
            t + corner + mh,
            corner,
            corner,
        );
    }

    /// A click on button `o`: onLeftClick, its action, and a toggle's flip.
    pub(crate) fn button_click(&mut self, o: ObjId) {
        let b = self.button(o).clone();
        match b.kind {
            ButtonKind::NStates {
                states,
                values,
                cfg_attrib,
                ..
            } => {
                let state = self.button_state(o);
                let next = (state + 1) % states;
                let value_of = |s: i32| {
                    values
                        .as_ref()
                        .and_then(|v| v.get(s as usize).copied())
                        .unwrap_or(s)
                };
                match cfg_attrib {
                    Some(attrib) => self.set_config_value(&attrib, value_of(next)),
                    None => {
                        if let ButtonKind::NStates { local_state, .. } =
                            &mut self.button_mut(o).kind
                        {
                            *local_state = next;
                        }
                    }
                }
                self.invalidate();
                self.dispatch(
                    Handle::Gui(o),
                    "ontoggle",
                    &[Value::Int((value_of(next) != 0) as i32)],
                );
                self.plain_click(o);
            }
            ButtonKind::Toggle {
                cfg_attrib,
                cfg_val,
                auto_toggle,
            } => {
                // the push first (onLeftClick, the action), then the flip and onToggle
                self.plain_click(o);
                if !auto_toggle {
                    return;
                }
                match cfg_attrib {
                    // without a config attribute, a button for a window, EQ on or EQ auto lights from that
                    // state (see isActive), whatever changes it
                    None => {
                        if !self.has_action_state(b.action.as_deref()) {
                            let b = self.button_mut(o);
                            b.activated = !b.activated;
                        }
                    }
                    Some(attrib) if cfg_val == 1 => {
                        let on = self.config_flag(&attrib);
                        self.set_config_flag(&attrib, !on);
                    }
                    Some(attrib) => {
                        let v = if self.button_is_active(o) { 0 } else { cfg_val };
                        self.set_config_value(&attrib, v);
                    }
                }
                self.invalidate();
                let active = self.button_is_active(o) as i32;
                self.dispatch(Handle::Gui(o), "ontoggle", &[Value::Int(active)]);
                self.dispatch(Handle::Gui(o), "onactivate", &[Value::Int(active)]);
            }
            ButtonKind::Plain => self.plain_click(o),
        }
    }

    fn plain_click(&mut self, o: ObjId) {
        self.dispatch(Handle::Gui(o), "onleftclick", &[]);
        let b = self.button(o).clone();
        if let Some(action) = b.action {
            self.action(o, &action, b.param.as_deref(), b.action_target.as_deref());
        }
    }

    // ---- sliders -----------------------------------------------------------------------------

    fn slider(&self, o: ObjId) -> &crate::gui::SliderData {
        match &self.objects[o].kind {
            Kind::Slider(s) => s,
            _ => panic!("not a slider"),
        }
    }

    fn slider_mut(&mut self, o: ObjId) -> &mut crate::gui::SliderData {
        match &mut self.objects[o].kind {
            Kind::Slider(s) => s,
            _ => panic!("not a slider"),
        }
    }

    fn slider_range(&self, o: ObjId) -> i32 {
        let s = self.slider(o);
        (s.high - s.low).max(1)
    }

    fn thumb_size(&mut self, o: ObjId) -> (i32, i32) {
        let thumb = self.slider(o).thumb.clone();
        thumb.and_then(|t| self.bitmap_size(&t)).unwrap_or((0, 0))
    }

    /// The thumb's box for the current position.
    fn thumb_rect(&mut self, o: ObjId) -> Rect {
        let (tw, th) = self.thumb_size(o);
        let rect = self.objects[o].base.rect;
        let s = self.slider(o);
        let frac = (s.position - s.low) as f64 / self.slider_range(o) as f64;
        if s.vertical {
            let travel = (rect.height - th).max(0);
            Rect::new(
                rect.left + (rect.width - tw) / 2,
                rect.top + ((1.0 - frac) * travel as f64) as i32,
                tw,
                th,
            )
        } else {
            let travel = (rect.width - tw).max(0);
            Rect::new(
                rect.left + (frac * travel as f64) as i32,
                rect.top + (rect.height - th) / 2,
                tw,
                th,
            )
        }
    }

    fn draw_slider(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let s = self.slider(o).clone();
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        let bl = s.bar_left.as_ref().and_then(|b| self.bitmap(b));
        let bm = s.bar_middle.as_ref().and_then(|b| self.bitmap(b));
        let br = s.bar_right.as_ref().and_then(|b| self.bitmap(b));
        if let Some(bm) = bm {
            let lw = bl
                .as_ref()
                .and_then(|b| self.bitmap_size(&b.id))
                .map(|s| s.0)
                .unwrap_or(0);
            let rw = br
                .as_ref()
                .and_then(|b| self.bitmap_size(&b.id))
                .map(|s| s.0)
                .unwrap_or(0);
            if let Some(bl) = bl {
                out.push(DrawCmd::image(
                    bl,
                    Rect::new(rect.left, rect.top, lw, rect.height),
                    alpha,
                ));
            }
            out.push(DrawCmd::image(
                bm,
                Rect::new(rect.left + lw, rect.top, rect.width - lw - rw, rect.height),
                alpha,
            ));
            if let Some(br) = br {
                out.push(DrawCmd::image(
                    br,
                    Rect::new(rect.right() - rw, rect.top, rw, rect.height),
                    alpha,
                ));
            }
        }
        // a seek bar has no thumb while nothing plays
        if s.action.as_deref() == Some("seek")
            && self.player.status() == Status::Stopped
            && !s.dragging
        {
            return;
        }
        let img = if s.dragging {
            s.down_thumb.clone()
        } else {
            None
        }
        .or(s.thumb.clone());
        let Some(img) = img else { return };
        if let Some(bm) = self.bitmap(&img) {
            let tr = self.thumb_rect(o);
            out.push(DrawCmd::image(bm, tr, alpha));
        }
    }

    fn slider_position_from(&mut self, o: ObjId, px: i32, py: i32) -> i32 {
        let (tw, th) = self.thumb_size(o);
        let rect = self.objects[o].base.rect;
        let s = self.slider(o);
        let frac = if s.vertical {
            let travel = (rect.height - th).max(1);
            1.0 - (py - rect.top - s.grab) as f64 / travel as f64
        } else {
            let travel = (rect.width - tw).max(1);
            (px - rect.left - s.grab) as f64 / travel as f64
        };
        s.low + (frac.clamp(0.0, 1.0) * self.slider_range(o) as f64) as i32
    }

    /// Sets the position and tells the script; `from_user` also drives the player first, and
    /// the thumb goes where the player snapped to, so the script shows the snapped value.
    pub(crate) fn slider_set_position(&mut self, o: ObjId, p: i32, from_user: bool) {
        let (low, high) = (self.slider(o).low, self.slider(o).high);
        let clamped = p.clamp(low, high.max(low));
        if clamped == self.slider(o).position && from_user {
            return;
        }
        self.slider_mut(o).position = clamped;
        if from_user {
            if let Some(snapped) = self.slider_moved(o, false) {
                self.slider_mut(o).position = snapped.clamp(low, high.max(low));
            }
        }
        self.repaint();
        let pos = self.slider(o).position;
        self.dispatch(Handle::Gui(o), "onsetposition", &[Value::Int(pos)]);
    }

    pub(crate) fn slider_pointer_down(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).locked {
            return;
        }
        self.slider_mut(o).dragging = true;
        // held where it was grabbed; a tap beside the thumb lands it just right of (below) the finger
        let (tw, th) = self.thumb_size(o);
        let t = self.thumb_rect(o);
        let vertical = self.slider(o).vertical;
        self.slider_mut(o).grab = if vertical {
            th / 2 - 2
        } else if px >= t.left && px < t.right() {
            px - t.left - 1
        } else {
            tw / 2 - 2
        };
        let p = self.slider_position_from(o, px, py);
        self.slider_set_position(o, p, true);
        self.dispatch(
            Handle::Gui(o),
            "onleftbuttondown",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn slider_pointer_move(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).dragging {
            let p = self.slider_position_from(o, px, py);
            self.slider_set_position(o, p, true);
        }
        self.dispatch(
            Handle::Gui(o),
            "onmousemove",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn slider_pointer_up(&mut self, o: ObjId, px: i32, py: i32) {
        if self.slider(o).dragging {
            self.slider_mut(o).dragging = false;
            self.repaint();
            self.dispatch(
                Handle::Gui(o),
                "onleftbuttonup",
                &[Value::Int(px), Value::Int(py)],
            );
            // a menu the release handler opened took the touch
            if self.pointer.press_ended {
                return;
            }
            let pos = self.slider(o).position;
            self.dispatch(Handle::Gui(o), "onsetfinalposition", &[Value::Int(pos)]);
            self.dispatch(Handle::Gui(o), "onpostedposition", &[Value::Int(pos)]);
            self.slider_moved(o, true);
        }
    }

    pub(crate) fn slider_pointer_cancel(&mut self, o: ObjId) {
        if self.slider(o).dragging {
            self.slider_mut(o).dragging = false;
            self.repaint();
            let (px, py) = self.last_pointer;
            self.dispatch(
                Handle::Gui(o),
                "onleftbuttonup",
                &[Value::Int(px), Value::Int(py)],
            );
        }
    }

    /// Puts the thumb where the player is until its first sync.
    pub(crate) fn slider_place_if_unsynced(&mut self, o: ObjId) {
        let s = self.slider(o);
        if s.dragging || s.synced {
            return;
        }
        if let Some(t) = self.slider_target(o) {
            let s = self.slider_mut(o);
            s.position = t.clamp(s.low, s.high.max(s.low));
        }
    }

    /// Follows the player when the user is not holding the thumb.
    pub(crate) fn slider_sync(&mut self, o: ObjId) {
        if self.slider(o).dragging {
            return;
        }
        let Some(target) = self.slider_target(o) else {
            return;
        };
        let s = self.slider(o);
        if target != s.position || !s.synced {
            let synced = s.synced;
            let s = self.slider_mut(o);
            s.position = target.clamp(s.low, s.high.max(s.low));
            // a thumb out of sight draws nothing; it shows where it is when it shows again
            if self.shows(o) {
                self.repaint();
            }
            // the first placement is the skin starting up, not a change scripts react to
            if synced {
                let pos = self.slider(o).position;
                self.dispatch(Handle::Gui(o), "onsetposition", &[Value::Int(pos)]);
                self.dispatch(Handle::Gui(o), "onpostedposition", &[Value::Int(pos)]);
            }
            self.slider_mut(o).synced = true;
        }
    }

    /// Where the player says a slider should sit, or `None` for one the player does not drive.
    pub(crate) fn slider_target(&mut self, o: ObjId) -> Option<i32> {
        let s = self.slider(o).clone();
        let span = s.high - s.low;
        let p = &*self.player;
        Some(match s.action.as_deref() {
            Some("seek") => {
                if p.length_ms() > 0 {
                    s.low + scaled(p.position_ms(), p.length_ms(), span)
                } else {
                    s.low
                }
            }
            Some("volume") => s.low + scaled(p.volume(), 255, span),
            Some("eq_band") => s.low + scaled(self.eq_value(s.param.as_deref())?, 255, span),
            Some("eq_preamp") => s.low + scaled(p.eq_preamp(), 255, span),
            // the middle at 127 of 0..255, which Winamp Modern's display reads as the center
            Some("pan") => s.low + (p.pan() + PAN_REACH) * span / (2 * PAN_REACH),
            _ => return None,
        })
    }

    /// An EQ slider's value: `param` is a band counted from 1, or "preamp".
    fn eq_value(&self, param: Option<&str>) -> Option<i32> {
        let param = param?;
        if param.eq_ignore_ascii_case("preamp") {
            Some(self.player.eq_preamp())
        } else {
            let i = param.parse::<i32>().ok()? - 1;
            usize::try_from(i)
                .ok()
                .and_then(|i| self.player.eq_bands().get(i).copied())
        }
    }

    /// Drives the player from a slider the listener moved; the values round to the nearest.
    /// Returns where the thumb belongs when the player took another value than the slider's,
    /// or `None` when it took the slider's.
    pub(crate) fn slider_moved(&mut self, o: ObjId, final_pos: bool) -> Option<i32> {
        let s = self.slider(o).clone();
        let frac = (s.position - s.low) as f64 / (s.high - s.low).max(1) as f64;
        let level = (frac * 255.0).round() as i32;
        let taken = match s.action.as_deref() {
            Some("seek") => {
                if final_pos {
                    let to = (frac * self.player.length_ms() as f64).round() as i32;
                    self.player.seek_to(to);
                    let pos = self.player.position_ms();
                    self.dispatch_system("onseek", &[Value::Int(pos)]);
                }
                true
            }
            Some("volume") => {
                self.player.set_volume(level);
                true
            }
            Some("eq_band") => {
                if s.param
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case("preamp"))
                {
                    self.player.set_eq_preamp(level);
                    self.player.eq_preamp() == level
                } else {
                    if let Some(band) = s.param.as_deref().and_then(|p| p.parse::<i32>().ok()) {
                        self.player.set_eq_band(band - 1, level);
                    }
                    true
                }
            }
            Some("eq_preamp") => {
                self.player.set_eq_preamp(level);
                self.player.eq_preamp() == level
            }
            Some("pan") => {
                let pan = (frac * 2.0 * PAN_REACH as f64).round() as i32 - PAN_REACH;
                self.player.set_pan(pan);
                self.player.pan() == pan
            }
            _ => true,
        };
        if taken {
            None
        } else {
            self.slider_target(o)
        }
    }

    // ---- drawing per kind --------------------------------------------------------------------

    /// What `o` draws, before the sheet fitting and the text placement of a frame.
    pub(crate) fn draw_object(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        if !self.objects[o].base.visible {
            return;
        }
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if self.objects[o].group.is_some() {
            self.draw_group(o, out);
            if matches!(self.objects[o].kind, Kind::Frame(_)) {
                self.frame_draw_divider(o, out);
            }
            return;
        }
        match self.objects[o].kind.clone() {
            Kind::Unknown { .. }
            | Kind::Group
            | Kind::Layout(_)
            | Kind::Frame(_)
            | Kind::Bucket(_)
            | Kind::XFade(_) => {}
            Kind::Vis(_) => self.draw_vis(o, out),
            Kind::EqVis(_) => self.draw_eqvis(o, out),
            Kind::Gradient(_) => self.draw_gradient(o, out),
            Kind::ColorThemeList(_) => self.draw_theme_list(o, out),
            Kind::PlaylistView(_) => self.draw_playlist(o, out),
            Kind::LayoutStatus(_) => self.draw_status_bar(o, out),
            Kind::Layer(l) => self.draw_layer(o, &l, out),
            Kind::AnimatedLayer(l, a) => {
                let Some(img) = l.image else { return };
                let Some(def) = self.bitmap(&img) else { return };
                let (fw, fh) = self.anim_frame_size(o);
                if fw <= 0 || fh <= 0 {
                    return;
                }
                // a frame past the end shows the first
                let length = self.anim_length(o);
                let f = if a.frame >= 0 && a.frame < length {
                    a.frame
                } else {
                    0
                };
                let vertical = self.anim_vertical(o);
                let sx = if vertical == Some(false) {
                    def.x + f * fw
                } else {
                    def.x
                };
                let sy = if vertical == Some(true) {
                    def.y + f * fh
                } else {
                    def.y
                };
                out.push(DrawCmd::image(
                    def.part(sx, sy, Some(fw), Some(fh)),
                    rect,
                    alpha,
                ));
            }
            Kind::Button(_) => self.draw_button(o, out),
            Kind::Text(_) => self.draw_text(o, out),
            Kind::Slider(_) => self.draw_slider(o, out),
            Kind::Status(s) => {
                let status = self.player.status();
                if let Some(bm) = s.image(status).and_then(|i| self.bitmap(i)) {
                    out.push(DrawCmd::image(bm, rect, alpha));
                }
            }
            Kind::Grid(g) => self.draw_grid(&g, rect, alpha, out),
            Kind::ProgressGrid(g, p) => {
                let n = self.progress_filled(o);
                if let Kind::ProgressGrid(_, pd) = &mut self.objects[o].kind {
                    pd.drawn_filled = n;
                }
                if n < 0 {
                    return;
                }
                let part = match p.orientation.as_str() {
                    "left" => Rect::new(rect.right() - n, rect.top, n, rect.height),
                    "top" => Rect::new(rect.left, rect.top, rect.width, n),
                    "bottom" => Rect::new(rect.left, rect.bottom() - n, rect.width, n),
                    _ => Rect::new(rect.left, rect.top, n, rect.height),
                };
                self.draw_grid(&g, part, alpha, out);
            }
            Kind::AlbumArt { not_found } => {
                if rect.width <= 0 || rect.height <= 0 {
                    return;
                }
                let Some(id) = not_found else { return };
                let Some(bm) = self.bitmap(&id) else { return };
                let Some((w, h)) = self.bitmap_size(&id) else {
                    return;
                };
                out.push(DrawCmd::Clip(rect));
                out.push(DrawCmd::image(
                    bm,
                    Rect::new(
                        rect.left + (rect.width - w) / 2,
                        rect.top + (rect.height - h) / 2,
                        w,
                        h,
                    ),
                    alpha,
                ));
                out.push(DrawCmd::Unclip);
            }
            Kind::RectObj(r) => {
                if rect.width <= 0 || rect.height <= 0 {
                    return;
                }
                let color = r
                    .color_id
                    .as_ref()
                    .and_then(|c| self.color(c))
                    .unwrap_or(r.color);
                if r.filled {
                    out.push(DrawCmd::Fill { rect, color, alpha });
                } else {
                    out.push(DrawCmd::Fill {
                        rect: Rect::new(rect.left, rect.top, rect.width, 1),
                        color,
                        alpha,
                    });
                    out.push(DrawCmd::Fill {
                        rect: Rect::new(rect.left, rect.bottom() - 1, rect.width, 1),
                        color,
                        alpha,
                    });
                    out.push(DrawCmd::Fill {
                        rect: Rect::new(rect.left, rect.top, 1, rect.height),
                        color,
                        alpha,
                    });
                    out.push(DrawCmd::Fill {
                        rect: Rect::new(rect.right() - 1, rect.top, 1, rect.height),
                        color,
                        alpha,
                    });
                }
            }
            Kind::Images(i) => self.draw_images(&i, rect, alpha, out),
            Kind::Menu(_) => {}
        }
    }

    fn draw_group(&mut self, g: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[g].base.rect;
        let alpha = self.objects[g].base.effective_alpha;
        let data = self.objects[g].group.as_ref().expect("a group");
        let bg = if data.draw_background {
            data.background.clone()
        } else {
            None
        };
        // a shaped group draws like any other: only the window's shape cuts
        if let Some(bm) = bg.and_then(|b| self.bitmap(&b)) {
            out.push(DrawCmd::image(bm, rect, alpha));
        }
        // children paint only inside their group (Bento's title bar cuts its 20 px menu pictures at 18)
        out.push(DrawCmd::Clip(rect));
        for c in self.objects[g].children().to_vec() {
            let from = out.len();
            self.draw_object(c, out);
            self.claim_drawn(from, out.len(), c);
        }
        out.push(DrawCmd::Unclip);
    }

    fn draw_layer(&mut self, o: ObjId, l: &crate::gui::LayerData, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        // a region picture draws too: the shape it cuts hides it
        let layout = self.layout_object(o);
        let focused = self.has_focus(layout);
        let img = l
            .inactive_image
            .as_ref()
            .filter(|_| !focused)
            .or(l.image.as_ref())
            .cloned();
        let Some(img) = img else { return };
        let Some(bm) = self.bitmap(&img) else { return };
        let size = self.bitmap_size(&img).filter(|s| s.0 > 0 && s.1 > 0);
        let Some((tw, th)) = size.filter(|_| l.tile) else {
            out.push(DrawCmd::image(bm, rect, alpha));
            return;
        };
        // one command per tile; tiles at the right and bottom edges take only the part that fits
        let mut ty = rect.top;
        while ty < rect.bottom() {
            let mut tx = rect.left;
            while tx < rect.right() {
                let w = tw.min(rect.right() - tx);
                let h = th.min(rect.bottom() - ty);
                let part = if w == tw && h == th {
                    bm.clone()
                } else {
                    bm.part(bm.x, bm.y, Some(w), Some(h))
                };
                out.push(DrawCmd::image(part, Rect::new(tx, ty, w, h), alpha));
                tx += tw;
            }
            ty += th;
        }
    }

    /// The grid stretched over `area`.
    fn draw_grid(
        &mut self,
        g: &crate::gui::GridData,
        area: Rect,
        alpha: i32,
        out: &mut Vec<DrawCmd>,
    ) {
        if area.width <= 0 || area.height <= 0 {
            return;
        }
        let size = |rt: &mut SkinRuntime, part: &str| rt.grid_part_size(g, part);
        let (l, t, r, b) = (area.left, area.top, area.right(), area.bottom());
        let tl = size(self, "topleft");
        let tr = size(self, "topright");
        let bl = size(self, "bottomleft");
        let br = size(self, "bottomright");
        let top = size(self, "top");
        let bottom = size(self, "bottom");
        let left = size(self, "left");
        let right = size(self, "right");
        let mut put = |rt: &mut SkinRuntime, part: &str, x: i32, y: i32, w: i32, h: i32| {
            if w <= 0 || h <= 0 {
                return;
            }
            if let Some(bm) = g
                .parts
                .iter()
                .find(|(k, _)| k == part)
                .and_then(|(_, v)| rt.bitmap(v))
            {
                out.push(DrawCmd::image(bm, Rect::new(x, y, w, h), alpha));
            }
        };
        // corners at their size; each edge and the middle span between their neighbors
        put(self, "topleft", l, t, tl.0, tl.1);
        put(self, "top", l + tl.0, t, area.width - tl.0 - tr.0, top.1);
        put(self, "topright", r - tr.0, t, tr.0, tr.1);
        put(self, "left", l, t + tl.1, left.0, area.height - tl.1 - bl.1);
        put(
            self,
            "middle",
            l + left.0,
            t + top.1,
            area.width - left.0 - right.0,
            area.height - top.1 - bottom.1,
        );
        put(
            self,
            "right",
            r - right.0,
            t + tr.1,
            right.0,
            area.height - tr.1 - br.1,
        );
        put(self, "bottomleft", l, b - bl.1, bl.0, bl.1);
        put(
            self,
            "bottom",
            l + bl.0,
            b - bottom.1,
            area.width - bl.0 - br.0,
            bottom.1,
        );
        put(self, "bottomright", r - br.0, b - br.1, br.0, br.1);
    }

    /// The length of a progress grid's filled part, in layout pixels; -1 while it shows none.
    pub(crate) fn progress_filled(&self, o: ObjId) -> i32 {
        let Kind::ProgressGrid(_, p) = &self.objects[o].kind else {
            return -1;
        };
        let upright = p.orientation == "top" || p.orientation == "bottom";
        let pl = &*self.player;
        if pl.status() == Status::Stopped || pl.length_ms() <= 0 {
            return -1;
        }
        let f = (pl.position_ms() as f32 / pl.length_ms() as f32).clamp(0.0, 1.0);
        let rect = self.objects[o].base.rect;
        ((if upright { rect.height } else { rect.width }) as f32 * f) as i32
    }

    /// Whether a progress grid's filled part is not what the last frame drew.
    pub(crate) fn progress_filled_changed(&self, o: ObjId) -> bool {
        let Kind::ProgressGrid(_, p) = &self.objects[o].kind else {
            return false;
        };
        self.progress_filled(o) != p.drawn_filled
    }

    /// When, in the runtime's clock at `now`, a progress grid's filled part next grows while
    /// the song plays on. `None` while it does not play or is full.
    pub(crate) fn progress_next_fill_at(&mut self, o: ObjId, now: i64) -> Option<i64> {
        let Kind::ProgressGrid(_, p) = &self.objects[o].kind else {
            return None;
        };
        let upright = p.orientation == "top" || p.orientation == "bottom";
        let rect = self.objects[o].base.rect;
        let length = if upright { rect.height } else { rect.width };
        let pl = &*self.player;
        if pl.status() != Status::Playing || pl.length_ms() <= 0 || length <= 0 {
            return None;
        }
        let f = self.progress_filled(o);
        if f >= length {
            return None;
        }
        let position = pl.position_ms();
        let length_ms = pl.length_ms();
        let Kind::ProgressGrid(_, p) = &mut self.objects[o].kind else {
            return None;
        };
        if position != p.asked_position || f != p.asked_filled {
            // the first position that fills one pixel more, never the one playing now, at a whole second
            let next = ((f + 1) as f64 * length_ms as f64 / length as f64).ceil() as i64;
            let second = (next + 999) / 1000 * 1000;
            p.asked_position = position;
            p.asked_filled = f;
            p.asked_due = now + (second - position as i64).max(1);
        } else if now >= p.asked_due {
            p.asked_due = now + 1000;
        }
        Some(p.asked_due)
    }

    fn draw_images(
        &mut self,
        i: &crate::gui::ImagesData,
        rect: Rect,
        alpha: i32,
        out: &mut Vec<DrawCmd>,
    ) {
        if rect.width <= 0 || rect.height <= 0 || i.spacing <= 0 {
            return;
        }
        let Some(id) = &i.image else { return };
        let Some(def) = self.bitmap(id) else { return };
        let Some(sheet_h) = def.h.or_else(|| self.bitmap_size(id).map(|s| s.1)) else {
            return;
        };
        let last = sheet_h / i.spacing - 1;
        let p = &*self.player;
        // the source as Winamp's 16-bit position: 0..65535, the balance's middle at 32768
        let position: Option<i32> = match i.source.as_str() {
            "volume" => Some(p.volume().clamp(0, 255) * 65535 / 255),
            "balance" => {
                Some(CENTER_16 + p.pan().clamp(-PAN_REACH, PAN_REACH) * (CENTER_16 - 1) / PAN_REACH)
            }
            "seek" => {
                if p.status() == Status::Stopped || p.length_ms() <= 0 {
                    None
                } else {
                    Some((p.position_ms() as i64 * 65535 / p.length_ms() as i64) as i32)
                }
            }
            _ => None,
        };
        let frame = if i.source == "balance" {
            let off = (position.unwrap_or(CENTER_16) - CENTER_16).abs();
            (off * last) >> 15
        } else {
            (position.unwrap_or(0) * last) >> 16
        };
        out.push(DrawCmd::image(
            def.part(
                def.x,
                def.y + frame * i.spacing,
                Some(rect.width),
                Some(rect.height),
            ),
            rect,
            alpha,
        ));
    }

    // ---- touches -----------------------------------------------------------------------------

    /// Whether a touch at the point lands on `o`: not a ghost, enabled, and inside its non-empty rectangle.
    fn takes_touch_at(&self, o: ObjId, px: i32, py: i32) -> bool {
        let b = &self.objects[o].base;
        !b.ghost && b.enabled && b.rect.width > 0 && b.rect.height > 0 && b.rect.contains(px, py)
    }

    /// The topmost object under a point of `o`'s window, or `None`. Ghost objects let touches through.
    pub(crate) fn hit_test(&mut self, o: ObjId, px: i32, py: i32) -> Option<ObjId> {
        if !self.objects[o].base.visible {
            return None;
        }
        if self.objects[o].group.is_some() {
            match &self.objects[o].kind {
                // entries scrolled out of the strip are not drawn there, and take no touch
                Kind::Bucket(_) if !self.objects[o].base.rect.contains(px, py) => return None,
                // only the page shown takes a touch, not the one fading out
                Kind::XFade(_) => {
                    return self.xfade_current(o).and_then(|c| self.hit_test(c, px, py))
                }
                _ => {}
            }
            for c in self.objects[o].children().to_vec().into_iter().rev() {
                if let Some(h) = self.hit_test(c, px, py) {
                    return Some(h);
                }
            }
            if self.objects[o].is_layout() {
                return if self.objects[o].base.rect.contains(px, py) {
                    Some(o)
                } else {
                    None
                };
            }
            // a group whose region is its rectangle takes what its children leave, and blocks what lies under it
            return if self.objects[o].base.rect_rgn && self.takes_touch_at(o, px, py) {
                Some(o)
            } else {
                None
            };
        }
        match &self.objects[o].kind {
            Kind::Grid(_)
            | Kind::ProgressGrid(..)
            | Kind::Unknown { .. }
            | Kind::Gradient(_)
            | Kind::LayoutStatus(_) => None,
            Kind::Layer(l) | Kind::AnimatedLayer(l, _) => {
                // clicks pass through transparent pixels, as in Winamp
                let image = l.image.clone();
                let rect = self.objects[o].base.rect;
                if self.takes_touch_at(o, px, py) && self.opaque_at(image.as_deref(), rect, px, py)
                {
                    Some(o)
                } else {
                    None
                }
            }
            _ => {
                if self.takes_touch_at(o, px, py) {
                    Some(o)
                } else {
                    None
                }
            }
        }
    }

    /// `o` hears the button go down at a point of its window.
    pub(crate) fn object_pointer_down(&mut self, o: ObjId, px: i32, py: i32) {
        match &self.objects[o].kind {
            Kind::ColorThemeList(_) => return self.themes_pointer_down(o, py),
            Kind::PlaylistView(_) => return self.pl_pointer_down(o, py),
            Kind::Button(_) => {
                self.button_mut(o).pressed = true;
                self.repaint();
            }
            Kind::Slider(_) => {
                self.slider_pointer_down(o, px, py);
                return;
            }
            Kind::Menu(_) => self.menu_show_pressed(o, true),
            _ => {}
        }
        self.dispatch(
            Handle::Gui(o),
            "onleftbuttondown",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn object_pointer_move(&mut self, o: ObjId, px: i32, py: i32) {
        match self.objects[o].kind {
            Kind::Slider(_) => return self.slider_pointer_move(o, px, py),
            Kind::ColorThemeList(_) => return self.themes_pointer_move(o, py),
            Kind::PlaylistView(_) => return self.pl_pointer_move(o, py),
            _ => {}
        }
        self.dispatch(
            Handle::Gui(o),
            "onmousemove",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn object_pointer_up(&mut self, o: ObjId, px: i32, py: i32, inside: bool) {
        match self.objects[o].kind {
            Kind::ColorThemeList(_) => self.themes_pointer_up(o, py, inside),
            Kind::PlaylistView(_) => self.pl_pointer_up(o, py, inside),
            Kind::Button(_) => {
                self.button_mut(o).pressed = false;
                self.repaint();
                self.dispatch(
                    Handle::Gui(o),
                    "onleftbuttonup",
                    &[Value::Int(px), Value::Int(py)],
                );
                // a menu the release handler opened took the touch
                if inside && !self.pointer.press_ended {
                    self.button_click(o);
                }
            }
            Kind::Slider(_) => self.slider_pointer_up(o, px, py),
            Kind::Menu(_) => {
                self.menu_show_pressed(o, false);
                self.dispatch(
                    Handle::Gui(o),
                    "onleftbuttonup",
                    &[Value::Int(px), Value::Int(py)],
                );
                if inside && !self.pointer.press_ended {
                    let menu = match &self.objects[o].kind {
                        Kind::Menu(m) => m.menu.clone(),
                        _ => None,
                    };
                    self.action(o, "MENU", menu.as_deref(), None);
                }
            }
            _ => {
                self.dispatch(
                    Handle::Gui(o),
                    "onleftbuttonup",
                    &[Value::Int(px), Value::Int(py)],
                );
            }
        }
    }

    /// The press ends without a release: the object lets go with no click or final position.
    pub(crate) fn object_pointer_cancel(&mut self, o: ObjId) {
        match self.objects[o].kind {
            Kind::Button(_) => {
                self.button_mut(o).pressed = false;
                self.repaint();
            }
            Kind::Slider(_) => self.slider_pointer_cancel(o),
            Kind::Menu(_) => self.menu_show_pressed(o, false),
            _ => {}
        }
    }

    /// A right click at a point of `o`'s window: the right button goes down and up.
    pub(crate) fn object_right_click(&mut self, o: ObjId, px: i32, py: i32) {
        self.dispatch(
            Handle::Gui(o),
            "onrightbuttondown",
            &[Value::Int(px), Value::Int(py)],
        );
        self.dispatch(
            Handle::Gui(o),
            "onrightbuttonup",
            &[Value::Int(px), Value::Int(py)],
        );
    }

    pub(crate) fn object_double_tap(&mut self, o: ObjId, px: i32, py: i32) {
        if matches!(self.objects[o].kind, Kind::PlaylistView(_)) {
            return self.pl_double_tap(o, px, py);
        }
        self.dispatch(
            Handle::Gui(o),
            "onleftbuttondblclk",
            &[Value::Int(px), Value::Int(py)],
        );
        if let Some(a) = self.objects[o].base.dbl_click_action.clone() {
            self.run_action(o, &a);
        }
    }

    fn menu_show_pressed(&mut self, o: ObjId, pressed: bool) {
        let Kind::Menu(m) = &self.objects[o].kind else {
            return;
        };
        let parent = self.objects[o].base.parent;
        let up = m
            .normal
            .clone()
            .and_then(|n| parent.and_then(|p| self.find_object(p, &n)));
        let dn = m
            .down
            .clone()
            .and_then(|n| parent.and_then(|p| self.find_object(p, &n)));
        let Some(dn) = dn else { return };
        if pressed {
            self.show_object(dn);
            if let Some(up) = up {
                self.hide_object(up);
            }
        } else {
            if let Some(up) = up {
                self.show_object(up);
            }
            self.hide_object(dn);
        }
        self.repaint();
    }
}

/// A size from an autowidthsource: its natural size plus its offset, or less its relative w
/// (the room it leaves); none when it is placed from the far side.
fn auto_size(
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
fn scaled(value: i32, whole: i32, range: i32) -> i32 {
    (value as f64 * range as f64 / whole as f64).round() as i32
}

fn cfg_attrib_of(value: &str) -> String {
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

fn layer_param(l: &mut crate::gui::LayerData, k: &str, value: &str) -> bool {
    match k {
        "image" => l.image = Some(value.to_string()),
        "tile" => l.tile = to_int_lenient(value).unwrap_or(0) != 0,
        "inactiveimage" => l.inactive_image = Some(value.to_string()),
        "region" | "scale" | "resize" | "dblclickaction" => {}
        _ => return false,
    }
    true
}

fn grid_param(g: &mut crate::gui::GridData, k: &str, value: &str) -> bool {
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
        if self.needs_layout {
            self.do_layout();
        }
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
