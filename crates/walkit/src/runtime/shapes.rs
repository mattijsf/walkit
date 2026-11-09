// SPDX-License-Identifier: GPL-3.0-or-later
//! Text placement, window shapes and fitting pictures to their sheets.

use super::{
    BitmapDef, DrawCmd, Fit, FontDef, Gamma, ObjId, Rc, Rect, Region, SkinRuntime, DEFAULT_ASCENT,
    DEFAULT_DESCENT,
};

impl SkinRuntime {
    /// Ascent and descent in whole pixels as Windows lays the face out at `size`, read from
    /// its file; `None` when the file gives none (or there is no file).
    fn gdi_line(&mut self, font: Option<&FontDef>, size: i32, bold: bool) -> Option<(i32, i32)> {
        let face = crate::gdi::FontFace::of(font, bold);
        let key = match &face {
            crate::gdi::FontFace::StandIn { .. } => face.file()?,
            crate::gdi::FontFace::SkinFile { font, .. } => font.id.to_string(),
            crate::gdi::FontFace::Default { .. } => return None,
        };
        let m = if let Some(m) = self.gdi_lines.get(&key) {
            m.clone()
        } else {
            let bytes: Option<Rc<[u8]>> = match &face {
                crate::gdi::FontFace::StandIn { .. } => self.host.font_file(&key),
                crate::gdi::FontFace::SkinFile { font, .. } => self.files.read(&font.file),
                crate::gdi::FontFace::Default { .. } => None,
            };
            let m = bytes.and_then(|b| crate::gdi::GdiLineMetrics::read(&b));
            self.gdi_lines.insert(key, m.clone());
            m
        };
        m.map(|m| m.at(crate::gdi::em_size(size)))
    }

    /// Places `cmd` in its rectangle, as every renderer then draws it: across by the width the
    /// host measures, less the ticker's scroll; up and down by the font's line, Windows' where
    /// the file gives it, else the host's. Centered and right-aligned text starts on a whole
    /// pixel, as Windows places it: its width rounded, half the room left over dropping the odd
    /// pixel, so the letters stay sharp instead of falling between two pixels.
    pub(crate) fn place_text(&mut self, cmd: &mut crate::draw::TextCmd) {
        let r = cmd.rect;
        let width = self
            .host
            .text_advance(&cmd.text, cmd.font.as_ref(), cmd.size, cmd.bold)
            .round() as i32;
        let x = match cmd.align {
            crate::draw::Align::Left => r.left,
            crate::draw::Align::Center => r.left + (r.width - width) / 2,
            crate::draw::Align::Right => r.right() - width,
        };
        cmd.x = (x - cmd.scroll) as f32;
        let (asc, desc) = self
            .gdi_line(cmd.font.as_ref(), cmd.size, cmd.bold)
            .map(|(a, d)| (a as f32, d as f32))
            .or_else(|| {
                self.host
                    .line_metrics(cmd.font.as_ref(), cmd.size, cmd.bold)
            })
            .unwrap_or_else(|| {
                let em = crate::gdi::em_size(cmd.size);
                (em * DEFAULT_ASCENT, em * DEFAULT_DESCENT)
            });
        cmd.baseline = match cmd.valign {
            crate::draw::VAlign::Top => r.top as f32 + asc,
            crate::draw::VAlign::Center => {
                r.top as f32 + ((r.height as f32 - (asc + desc)) / 2.0).trunc() + asc
            }
            crate::draw::VAlign::Bottom => r.bottom() as f32 - desc,
        };
    }

    // ---- shapes and fitting ------------------------------------------------------------------

    /// The start of `layout`'s shaped window with its pictures fitted to their sheets and tinted
    /// for the color theme. A window keeps its shape until the skin changes, and a new theme
    /// changes it, so the same shape gives back the same command from frame to frame.
    pub(crate) fn window_start(
        &mut self,
        layout: ObjId,
        region: &Region,
        shape_at: i64,
    ) -> DrawCmd {
        if let Some((at, cmd)) = self.shaped.get(&layout) {
            if *at == shape_at {
                return cmd.clone();
            }
        }
        let mut fitted = region
            .map_pics(&mut |img| self.fit_region_image(img))
            .unwrap_or(Region::Op {
                base: None,
                add: Vec::new(),
                keep: Vec::new(),
                cut: Vec::new(),
            });
        let mut gammas: Vec<Option<Gamma>> = Vec::new();
        fitted.pics_mut(&mut |img| {
            gammas.push(Some(img.gamma.unwrap_or(Gamma {
                r: 0,
                g: 0,
                b: 0,
                boost: 0,
                gray: 0,
            })));
        });
        // tints are looked up through the runtime, so apply them in a second pass
        let mut groups: Vec<Option<Rc<str>>> = Vec::new();
        fitted.pics_mut(&mut |img| groups.push(img.bitmap.gamma_group.clone()));
        let tints: Vec<Option<Gamma>> = groups.iter().map(|g| self.gamma(g.as_deref())).collect();
        let mut i = 0;
        fitted.pics_mut(&mut |img| {
            img.gamma = tints[i];
            i += 1;
        });
        let cmd = DrawCmd::WindowStart(fitted);
        self.shaped.insert(layout, (shape_at, cmd.clone()));
        cmd
    }

    fn fit_region_image(
        &mut self,
        img: &crate::draw::RegionImage,
    ) -> Option<crate::draw::RegionImage> {
        let (bitmap, dst) = self.fit_part_to_sheet(&img.bitmap, img.dst)?;
        Some(crate::draw::RegionImage {
            bitmap,
            dst,
            alpha: img.alpha,
            gamma: img.gamma,
            smooth: img.smooth,
        })
    }

    /// The part of an image command that lies on its sheet, with the destination cut the same
    /// way: Winamp draws nothing for rows or columns a bitmap element asks for past the edge of
    /// its file. `None` when nothing is left.
    pub(crate) fn fit_part_to_sheet(
        &mut self,
        b: &BitmapDef,
        dst: Rect,
    ) -> Option<(BitmapDef, Rect)> {
        if !self.fits.contains_key(b) {
            if let Some(fit) = self.fit_part(b) {
                self.fits.insert(b.clone(), fit);
            }
        }
        match self.fits.get(b) {
            None | Some(Fit::Whole) => Some((b.clone(), dst)),
            Some(Fit::None) => None,
            Some(Fit::Cut {
                part,
                asked_w,
                asked_h,
            }) => {
                let fw = part.w.unwrap_or(*asked_w);
                let fh = part.h.unwrap_or(*asked_h);
                // a stretched picture keeps its scale: the destination shrinks by the same share
                let dw = if fw == *asked_w {
                    dst.width
                } else {
                    dst.width * fw / *asked_w
                };
                let dh = if fh == *asked_h {
                    dst.height
                } else {
                    dst.height * fh / *asked_h
                };
                if dw <= 0 || dh <= 0 {
                    None
                } else {
                    Some((part.clone(), Rect::new(dst.left, dst.top, dw, dh)))
                }
            }
        }
    }

    /// Works out once how much of the part `b` names lies on its sheet; a sheet whose size is
    /// not known yet leaves it to the next frame.
    fn fit_part(&mut self, b: &BitmapDef) -> Option<Fit> {
        let (sheet_w, sheet_h) = self.image_size(&b.file)?;
        let sw = b.w.unwrap_or(sheet_w - b.x);
        let sh = b.h.unwrap_or(sheet_h - b.y);
        let fw = sw.min(sheet_w - b.x);
        let fh = sh.min(sheet_h - b.y);
        let empty = fw <= 0 || fh <= 0;
        let outside = sw <= 0 || sh <= 0;
        Some(if empty || outside {
            Fit::None
        } else if fw == sw && fh == sh && b.w.is_some() && b.h.is_some() {
            Fit::Whole
        } else {
            Fit::Cut {
                part: b.part(b.x, b.y, Some(fw), Some(fh)),
                asked_w: sw,
                asked_h: sh,
            }
        })
    }
}
