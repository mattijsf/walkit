// SPDX-License-Identifier: GPL-3.0-or-later

//! A window's drawing as a renderer sees it: the objects' commands with every picture fitted to
//! its sheet and tinted, bitmap text spelled out, and text placed.

use super::SkinRuntime;
use crate::draw::{Align, BitmapTextCmd, DrawCmd, Rect, Rgb, VAlign};
use crate::gui::ObjId;

impl SkinRuntime {
    /// The drawing of the window `layout` shows.
    pub(crate) fn draw_layout(&mut self, layout: ObjId) -> Vec<DrawCmd> {
        self.draw_layout_owned(layout).0
    }

    /// The drawing of the window `layout` shows, and the object that drew each command: the
    /// innermost object whose drawing it is, the layout for what no child drew, and nothing for
    /// the end of a shape or a clip.
    pub(crate) fn draw_layout_owned(
        &mut self,
        layout: ObjId,
    ) -> (Vec<DrawCmd>, Vec<Option<ObjId>>) {
        if self.needs_layout {
            self.do_layout();
        }
        self.dirty = false;
        let mut drawn = Vec::new();
        self.draw_window(layout, &mut drawn);
        let mut owners = std::mem::take(&mut self.drawn_owners);
        owners.resize(drawn.len(), None);
        for (cmd, owner) in drawn.iter().zip(owners.iter_mut()) {
            *owner = match cmd {
                DrawCmd::Unclip | DrawCmd::WindowEnd => None,
                _ => owner.or(Some(layout)),
            };
        }
        let mut out = Vec::with_capacity(drawn.len());
        let mut out_owners = Vec::with_capacity(drawn.len());
        for (cmd, owner) in drawn.into_iter().zip(owners) {
            match cmd {
                DrawCmd::Image {
                    bitmap,
                    dst,
                    alpha,
                    smooth,
                    ..
                } => {
                    if let Some((bitmap, dst)) = self.fit_part_to_sheet(&bitmap, dst) {
                        let gamma = self.gamma(bitmap.gamma_group.as_deref());
                        out.push(DrawCmd::Image {
                            bitmap,
                            dst,
                            alpha,
                            gamma,
                            smooth,
                        });
                        out_owners.push(owner);
                    }
                }
                DrawCmd::WindowStart(region) => {
                    let at = self.objects[layout]
                        .layout()
                        .map(|d| d.shape_at)
                        .unwrap_or(-1);
                    out.push(self.window_start(layout, &region, at));
                    out_owners.push(owner);
                }
                DrawCmd::BitmapText(cmd) => {
                    let from = out.len();
                    self.spell(&cmd, &mut out);
                    for c in &out[from..] {
                        out_owners.push(match c {
                            DrawCmd::Unclip => None,
                            _ => owner,
                        });
                    }
                }
                DrawCmd::Text(mut cmd) => {
                    self.place_text(&mut cmd);
                    out.push(DrawCmd::Text(cmd));
                    out_owners.push(owner);
                }
                other => {
                    out.push(other);
                    out_owners.push(owner);
                }
            }
        }
        (out, out_owners)
    }

    /// The layout's drawing: inside its shape when it has one.
    fn draw_window(&mut self, layout: ObjId, out: &mut Vec<DrawCmd>) {
        if !self.objects[layout].base.visible {
            return;
        }
        let desktop_alpha = self.objects[layout]
            .layout()
            .is_some_and(|d| d.desktop_alpha);
        let (shape, _) = if desktop_alpha {
            (None, -1)
        } else {
            self.window_shape(layout)
        };
        // the shape may have drawn objects to measure them: the owners start here
        self.drawn_owners.clear();
        self.tracking_owners = true;
        match shape {
            None => {
                if !desktop_alpha {
                    out.push(self.window_black(layout));
                }
                self.draw_object(layout, out);
            }
            Some(shape) => {
                out.push(DrawCmd::WindowStart(shape));
                out.push(self.window_black(layout));
                self.draw_object(layout, out);
                out.push(DrawCmd::WindowEnd);
            }
        }
        self.tracking_owners = false;
    }

    /// Black over the whole window, under its objects: Winamp fills a window's canvas with black
    /// before the objects draw, so a pixel inside the window that no object draws is black (the
    /// shape still cuts the window's outline). A `desktopalpha` window starts clear instead.
    fn window_black(&mut self, layout: ObjId) -> DrawCmd {
        let (w, h) = self.layout_size(layout);
        DrawCmd::Fill {
            rect: Rect::new(0, 0, w, h),
            color: Rgb(0),
            alpha: 255,
        }
    }

    /// Commands `from` to `to` of the window being drawn, as `owner` drew them unless a child of
    /// it did: the innermost object claims a command.
    pub(crate) fn claim_drawn(&mut self, from: usize, to: usize, owner: ObjId) {
        if !self.tracking_owners {
            return;
        }
        if self.drawn_owners.len() < to {
            self.drawn_owners.resize(to, None);
        }
        for o in &mut self.drawn_owners[from..to] {
            if o.is_none() {
                *o = Some(owner);
            }
        }
    }

    /// `cmd` as a renderer draws it, added to `out`: inside its rectangle, each character's cell
    /// of the font sheet at its place, aligned by the text's width and less its scroll, a colon
    /// `colon_width` wide where the skin narrows it, a space advancing only.
    fn spell(&mut self, cmd: &BitmapTextCmd, out: &mut Vec<DrawCmd>) {
        let f = &cmd.font;
        let r = cmd.rect;
        let advance = f.char_width + f.h_spacing;
        let colon = cmd.colon_width.unwrap_or(advance);
        let total: i32 = cmd
            .text
            .chars()
            .map(|c| if c == ':' { colon } else { advance })
            .sum();
        let mut x = match cmd.align {
            Align::Left => r.left,
            Align::Center => r.left + (r.width - total) / 2,
            Align::Right => r.right() - total,
        } - cmd.scroll;
        let y = match cmd.valign {
            VAlign::Top => r.top,
            VAlign::Center => r.top + (r.height - f.char_height) / 2,
            VAlign::Bottom => r.bottom() - f.char_height,
        };
        let gamma = self.gamma(f.gamma_group.as_deref());
        out.push(DrawCmd::Clip(r));
        for c in cmd.text.chars() {
            if let Some(cell) = f.cell(c) {
                let g = super::objects::glyph(f, cell);
                if let Some((bitmap, dst)) =
                    self.fit_part_to_sheet(&g, Rect::new(x, y, f.char_width, f.char_height))
                {
                    out.push(DrawCmd::Image {
                        bitmap,
                        dst,
                        alpha: cmd.alpha,
                        gamma,
                        smooth: false,
                    });
                }
            }
            x += if c == ':' { colon } else { advance };
        }
        out.push(DrawCmd::Unclip);
    }
}
