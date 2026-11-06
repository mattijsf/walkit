// SPDX-License-Identifier: GPL-3.0-or-later
//! Touches on each kind of object: what a press, a drag and a release do.

use super::SkinRuntime;
use crate::gui::{Kind, ObjId};
use crate::maki::{Handle, Value};

impl SkinRuntime {
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
