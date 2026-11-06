// SPDX-License-Identifier: GPL-3.0-or-later
//! The natural size of each kind of object: what it takes when the skin gives it none.

use super::objects::{auto_size, label_font, LABEL_PAD, LABEL_SIZE, UNSIZED_H, UNSIZED_W};
use super::SkinRuntime;
use crate::gui::{ButtonKind, Kind, ObjId};

impl SkinRuntime {
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
        // the source's size plus what its relative w/h leaves around it
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

    pub(super) fn grid_part_size(&mut self, g: &crate::gui::GridData, part: &str) -> (i32, i32) {
        g.parts
            .iter()
            .find(|(k, _)| k == part)
            .and_then(|(_, v)| self.bitmap_size(v))
            .unwrap_or((0, 0))
    }
}
