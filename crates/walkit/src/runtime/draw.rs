// SPDX-License-Identifier: GPL-3.0-or-later

//! A window's drawing as a renderer sees it: the objects' commands with every picture fitted to
//! its sheet and tinted, bitmap text spelled out, and text placed.

use super::SkinRuntime;
use crate::draw::{Align, BitmapTextCmd, DrawCmd, Rect, VAlign};
use crate::gui::ObjId;

impl SkinRuntime {
    /// The drawing of the window `layout` shows.
    pub(crate) fn draw_layout(&mut self, layout: ObjId) -> Vec<DrawCmd> {
        if self.needs_layout {
            self.do_layout();
        }
        self.dirty = false;
        let mut drawn = Vec::new();
        self.draw_window(layout, &mut drawn);
        let mut out = Vec::with_capacity(drawn.len());
        for cmd in drawn {
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
                    }
                }
                DrawCmd::WindowStart(region) => {
                    let at = self.objects[layout]
                        .layout()
                        .map(|d| d.shape_at)
                        .unwrap_or(-1);
                    out.push(self.window_start(layout, &region, at));
                }
                DrawCmd::BitmapText(cmd) => self.spell(&cmd, &mut out),
                DrawCmd::Text(mut cmd) => {
                    self.place_text(&mut cmd);
                    out.push(DrawCmd::Text(cmd));
                }
                other => out.push(other),
            }
        }
        out
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
        let Some(shape) = shape else {
            self.draw_object(layout, out);
            return;
        };
        out.push(DrawCmd::WindowStart(shape));
        self.draw_object(layout, out);
        out.push(DrawCmd::WindowEnd);
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
