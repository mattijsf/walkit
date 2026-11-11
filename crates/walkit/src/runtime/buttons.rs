// SPDX-License-Identifier: GPL-3.0-or-later
//! Buttons: their pictures per state, their actions, and what a press does.

use super::objects::{label_font, LABEL_SIZE};
use super::SkinRuntime;
use crate::draw::{Align, BitmapDef, DrawCmd, Rect, Rgb, TextCmd, VAlign};
use crate::gui::{ButtonKind, Kind, ObjId};
use crate::maki::{Handle, Value};

impl SkinRuntime {
    fn button(&self, o: ObjId) -> &crate::gui::ButtonData {
        match &self.objects[o].kind {
            Kind::Button(b) => b,
            _ => panic!("not a button"),
        }
    }

    pub(super) fn button_mut(&mut self, o: ObjId) -> &mut crate::gui::ButtonData {
        match &mut self.objects[o].kind {
            Kind::Button(b) => b,
            _ => panic!("not a button"),
        }
    }

    /// The bitmap id a picture attribute draws: itself, or for a multi-state button its state's.
    pub(super) fn button_picture(&mut self, o: ObjId, base: Option<&str>) -> Option<String> {
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
        if !Self::has_action_state(action.as_deref()) {
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

    pub(super) fn draw_button(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let b = self.button(o).clone();
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if b.borders {
            self.draw_framed_button(out, rect, alpha, b.pressed);
        }
        if let Some(t) = &b.label {
            // pressed, the label sits 1 px right and down (Wasabi SDK buttwnd)
            let shift = i32::from(b.pressed && b.borders);
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
                smooth: false,
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
        let shift = i32::from(b.pressed && self.objects[o].base.enabled);
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

    /// Winamp's button frame, as it draws the buttons it makes itself and `borders="1"`
    /// buttons: the nine `wasabi.button` pictures (or the pressed ones) over the button's box.
    pub(crate) fn draw_framed_button(
        &mut self,
        out: &mut Vec<DrawCmd>,
        r: Rect,
        alpha: i32,
        pressed: bool,
    ) {
        let base = if pressed {
            "wasabi.button.pressed"
        } else {
            "wasabi.button"
        };
        let part = |rt: &mut SkinRuntime, part: &str| {
            let name = format!("{base}.{part}");
            let bm = rt.bitmap(&name)?;
            let size = rt.bitmap_size(&name).unwrap_or((0, 0));
            Some((bm, size))
        };
        // nine pictures over the whole box: the corners at their own sizes, the edges and the
        // fill (the picture named "middle") stretched between them
        let parts: Vec<Option<(BitmapDef, (i32, i32))>> = [
            "top.left",
            "top",
            "top.right",
            "left",
            "middle",
            "right",
            "bottom.left",
            "bottom",
            "bottom.right",
        ]
        .iter()
        .map(|p| part(self, p))
        .collect();
        let size = |i: usize| parts[i].as_ref().map_or((0, 0), |p| p.1);
        let (tl, tr, l_, r_, bl, b_, br) = (
            size(0),
            size(2),
            size(3),
            size(5),
            size(6),
            size(7),
            size(8),
        );
        let (top, bottom, left, right) = (r.top, r.bottom(), r.left, r.right());
        let boxes = [
            (left, top, left + tl.0, top + tl.1),
            (left + tl.0, top, right - tr.0, top + tl.1),
            (right - tr.0, top, right, top + tl.1),
            (left, top + tl.1, left + l_.0, bottom - bl.1),
            (left + tl.0, top + tl.1, right - tr.0, bottom - bl.1),
            (right - r_.0, top + tr.1, right, bottom - br.1),
            (left, bottom - bl.1, left + bl.0, bottom),
            (left + bl.0, bottom - b_.1, right - br.0, bottom),
            (right - br.0, bottom - br.1, right, bottom),
        ];
        for (p, (x0, y0, x1, y1)) in parts.into_iter().zip(boxes) {
            if let Some((bm, _)) = p {
                if x1 > x0 && y1 > y0 {
                    out.push(DrawCmd::image(
                        bm,
                        Rect::new(x0, y0, x1 - x0, y1 - y0),
                        alpha,
                    ));
                }
            }
        }
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
                        if !Self::has_action_state(b.action.as_deref()) {
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
}
