// SPDX-License-Identifier: GPL-3.0-or-later
//! The skin's resources as the runtime reads them: bitmaps through their aliases, bitmap fonts,
//! fonts, gamma groups and colors.

use super::{
    names_for, BitmapDef, BitmapFontDef, FontDef, Gamma, Rc, Rect, Rgb, SkinRuntime,
    MAX_ALIAS_DEPTH, MAX_OPAQUE_SCAN,
};

impl SkinRuntime {
    /// The bitmap element `id` names, through aliases; a plain file name used as an image.
    pub(crate) fn bitmap(&mut self, id: &str) -> Option<BitmapDef> {
        if let Some(found) = self.bitmap_lookups.get(id) {
            return found.clone();
        }
        let found = self.find_bitmap(id);
        self.bitmap_lookups.insert(id.to_string(), found.clone());
        found
    }

    /// The size of picture `file`, as the host decoded it; asked once.
    pub(crate) fn image_size(&mut self, file: &str) -> Option<(i32, i32)> {
        if let Some(size) = self.image_sizes.get(file) {
            return Some(*size);
        }
        let size = self.host.image_size(file)?;
        self.image_sizes.insert(file.to_string(), size);
        Some(size)
    }

    /// [`SkinRuntime::bitmap`] worked out: the element `id` names, through its aliases and
    /// old-style twin, a skin's before the base's, else a file of that name.
    fn find_bitmap(&mut self, id: &str) -> Option<BitmapDef> {
        let mut from_base: Option<BitmapDef> = None;
        for name in names_for(id) {
            let Some((key, def)) = self.aliased_bitmap(&name) else {
                continue;
            };
            if !self.base_bitmaps.contains(&key) {
                return Some(def);
            }
            if from_base.is_none() {
                from_base = Some(def);
            }
        }
        if let Some(def) = from_base {
            self.base_usage.insert(format!("bitmap:{}", def.id));
            return Some(def);
        }
        // a plain file name used as an image
        if self.files.exists(id) {
            let key = id.to_lowercase();
            Some(
                self.bitmaps
                    .entry(key.clone())
                    .or_insert_with(|| BitmapDef::new(&key, id, 0, 0, None, None, None))
                    .clone(),
            )
        } else {
            None
        }
    }

    /// The bitmap element `name` defines, or names through element aliases (at most 8 hops),
    /// with the id it is defined under; `None` when the chain ends without one.
    fn aliased_bitmap(&self, name: &str) -> Option<(String, BitmapDef)> {
        let mut key: &str = name;
        for _ in 0..8 {
            if let Some(def) = self.bitmaps.get(key) {
                return Some((key.to_string(), def.clone()));
            }
            key = self.aliases.get(key)?;
        }
        None
    }

    /// The size of bitmap `id`: as its element says, else from its file.
    pub(crate) fn bitmap_size(&mut self, id: &str) -> Option<(i32, i32)> {
        let def = self.bitmap(id)?;
        if let (Some(w), Some(h)) = (def.w, def.h) {
            return Some((w, h));
        }
        let full = self.image_size(&def.file)?;
        Some((
            def.w.unwrap_or(full.0 - def.x),
            def.h.unwrap_or(full.1 - def.y),
        ))
    }

    /// Whether the bitmap drawn into `rect` is not fully transparent at a point. Unknown
    /// pixels count as opaque, so a host that cannot read pixels keeps rectangle hit testing.
    pub(crate) fn opaque_at(&mut self, image: Option<&str>, rect: Rect, px: i32, py: i32) -> bool {
        let Some(def) = image.and_then(|i| self.bitmap(i)) else {
            return true;
        };
        let Some((w, h)) = self.bitmap_size(&def.id) else {
            return true;
        };
        if rect.width <= 0 || rect.height <= 0 {
            return true;
        }
        let sx = def.x + (px - rect.left) * w / rect.width;
        let sy = def.y + (py - rect.top) * h / rect.height;
        let Some(p) = self.host.pixel(&def.file, sx, sy) else {
            return true;
        };
        (p >> 24) != 0
    }

    /// Where `image` drawn into `dst` stops showing on the right: the right edge of its last
    /// column with a pixel that is not fully transparent, in layout pixels, or `dst`'s left
    /// edge when none shows. The column is looked up once per bitmap part.
    pub(crate) fn opaque_right(&mut self, image: &BitmapDef, dst: Rect) -> i32 {
        if dst.width <= 0 {
            return dst.left;
        }
        let Some(full) = self.image_size(&image.file) else {
            return dst.right();
        };
        let sw = image.w.unwrap_or(full.0 - image.x);
        let sh = image.h.unwrap_or(full.1 - image.y);
        // only the part the sheet holds is drawn
        let cw = sw.min(full.0 - image.x);
        let ch = sh.min(full.1 - image.y);
        let no_source = cw <= 0 || ch <= 0;
        if image.x < 0 || image.y < 0 || no_source {
            return dst.left;
        }
        let key = format!(
            "{}|{}|{}|{}|{}",
            image.file.to_lowercase(),
            image.x,
            image.y,
            sw,
            sh
        );
        let column = if let Some(c) = self.opaque_columns.get(&key) {
            *c
        } else {
            let c = if cw as i64 * ch as i64 > MAX_OPAQUE_SCAN {
                sw - 1
            } else {
                match self.host.pixels(&image.file, image.x, image.y, cw, ch) {
                    None => sw - 1,
                    Some(px) => (0..cw)
                        .rev()
                        .find(|c| (0..ch).any(|r| (px[(r * cw + c) as usize] >> 24) != 0))
                        .unwrap_or(-1),
                }
            };
            self.opaque_columns.insert(key, c);
            c
        };
        if column < 0 {
            return dst.left;
        }
        // stretched to dst, source column c covers dst pixels up to (c + 1) * dst / source, rounded up
        dst.right()
            .min(dst.left + ((column + 1) * dst.width + sw - 1) / sw)
    }

    /// A bitmap font; its `file` may name a bitmap element, which then gives the file and the offset.
    pub(crate) fn bitmap_font(&mut self, id: &str) -> Option<Rc<BitmapFontDef>> {
        let key = id.to_lowercase();
        if let Some(f) = self.resolved_fonts.get(&key) {
            return f.clone();
        }
        let resolved = self.bitmap_fonts.get(&key).map(|def| {
            let element = self.bitmaps.get(&def.file.to_lowercase());
            // the color theme reaches a bitmap font only through the bitmap element it draws from;
            // Winamp 5.666 ignores gammagroup on <bitmapfont> itself
            match element {
                Some(e) => Rc::new(BitmapFontDef {
                    id: def.id.clone(),
                    file: e.file.clone(),
                    char_width: def.char_width,
                    char_height: def.char_height,
                    h_spacing: def.h_spacing,
                    v_spacing: def.v_spacing,
                    ox: e.x,
                    oy: e.y,
                    gamma_group: e.gamma_group.clone(),
                }),
                None => Rc::new(BitmapFontDef {
                    ox: 0,
                    oy: 0,
                    gamma_group: None,
                    ..def.clone()
                }),
            }
        });
        self.resolved_fonts.insert(key, resolved.clone());
        resolved
    }

    /// The TrueType font `id`.
    pub(crate) fn font(&self, id: &str) -> Option<FontDef> {
        self.fonts.get(&id.to_lowercase()).cloned()
    }

    /// The tint of gamma group `group` in the color theme worn now.
    pub(crate) fn gamma(&mut self, group: Option<&str>) -> Option<Gamma> {
        let group = group?;
        if let Some(g) = self.gammas.get(group) {
            return *g;
        }
        let g = self
            .current_gamma_set
            .as_ref()
            .and_then(|set| self.gamma_sets.iter().find(|(n, _)| n == set))
            .and_then(|(_, groups)| groups.get(&group.to_lowercase()).copied())
            .filter(|g| !g.is_identity());
        self.gammas.insert(group.to_string(), g);
        g
    }

    /// The color `id`, tinted by the theme.
    pub(crate) fn color(&mut self, id: &str) -> Option<Rgb> {
        self.color_at_depth(id, 0)
    }

    fn color_at_depth(&mut self, id: &str, depth: i32) -> Option<Rgb> {
        let key = self.color_key(id)?;
        if self.base_colors.contains(&key) {
            self.base_usage.insert(format!("color:{key}"));
        }
        // an alias takes the other color's RGB, tinted by the alias's group (or the other's when it has none)
        let c = if let Some(c) = self.colors.get(&key) {
            *c
        } else {
            let alias = self
                .color_aliases
                .get(&key)
                .filter(|_| depth < MAX_ALIAS_DEPTH)?
                .clone();
            self.plain_color(&alias, depth + 1)?
        };
        // a color that names no gammagroup takes the theme's "Text" group (Wasabi SDK skinclr)
        let group = self
            .color_groups
            .get(&key)
            .cloned()
            .or_else(|| {
                self.color_aliases
                    .get(&key)
                    .and_then(|a| self.color_groups.get(a))
                    .cloned()
            })
            .unwrap_or_else(|| "text".to_string());
        let Some(g) = self.gamma(Some(&group)) else {
            return Some(c);
        };
        Some(Rgb(g.apply(c.0 | 0xff00_0000) & 0xffffff))
    }

    /// A color's RGB before any tint, following aliases.
    fn plain_color(&self, id: &str, depth: i32) -> Option<Rgb> {
        let key = self.color_key(id)?;
        if let Some(c) = self.colors.get(&key) {
            Some(*c)
        } else {
            let alias = self
                .color_aliases
                .get(&key)
                .filter(|_| depth < MAX_ALIAS_DEPTH)?;
            self.plain_color(alias, depth + 1)
        }
    }

    /// The name a color is defined under, as an RGB or an alias: `id` or its old-style twin, a
    /// skin's definition before the default set's. `None` when neither is defined.
    fn color_key(&self, id: &str) -> Option<String> {
        let names: Vec<String> = names_for(id)
            .into_iter()
            .filter(|n| self.colors.contains_key(n) || self.color_aliases.contains_key(n))
            .collect();
        names
            .iter()
            .find(|n| !self.base_colors.contains(*n))
            .or(names.first())
            .cloned()
    }
}
