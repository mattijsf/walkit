// SPDX-License-Identifier: GPL-3.0-or-later
//! The drawing of each kind of object into the window's draw list.

use super::objects::{CENTER_16, PAN_REACH};
use super::SkinRuntime;
use crate::draw::{DrawCmd, Rect};
use crate::gui::{Kind, ObjId};
use crate::player::Status;

impl SkinRuntime {
    /// What `o` draws, before the sheet fitting and the text placement of a frame.
    pub(crate) fn draw_object(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        if !self.objects[o].base.visible {
            return;
        }
        let rect = self.objects[o].base.rect;
        let alpha = self.objects[o].base.effective_alpha;
        if self.objects[o].group.is_some() {
            self.draw_group(o, out);
            match self.objects[o].kind {
                Kind::Frame(_) => self.frame_draw_divider(o, out),
                // the buttons lie over the field
                Kind::LayoutStatus(_) => self.draw_status_bar(o, out),
                _ => {}
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
            Kind::ClassicWindow(_) => self.draw_classic(o, out),
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
}
