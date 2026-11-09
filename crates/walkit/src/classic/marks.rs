// SPDX-License-Identifier: GPL-3.0-or-later
//! Marks a host puts on classic playlist rows: a small picture, or a few letters where it has
//! none, in the row's own color, before the title or ahead of the length. Each mark sits in a cell
//! as wide as the widest mark the host set, so the titles beside marks of different widths line up.

/// One mark: 1-bit pixel art, one string per row of pixels with `#` for ink, drawn a pixel to the
/// skin's pixel; or, without any, [`RowMark::text`] in the row's letters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowMark {
    /// The picture's rows of pixels, `#` for ink and anything else for none.
    pub pixels: Vec<String>,
    /// What stands in for a picture: a few letters drawn as the row's text.
    pub text: String,
}

impl RowMark {
    /// How wide the picture is; 0 without one.
    #[must_use]
    pub fn pixel_width(&self) -> i32 {
        self.pixels
            .iter()
            .map(|r| r.chars().count() as i32)
            .max()
            .unwrap_or(0)
    }

    /// How tall the picture is; 0 without one.
    #[must_use]
    pub fn pixel_height(&self) -> i32 {
        self.pixels.len() as i32
    }

    /// The picture in `argb`, transparent around its ink, `pixel_width` by `pixel_height`.
    #[must_use]
    pub fn picture(&self, argb: u32) -> Vec<u32> {
        let w = self.pixel_width();
        let mut out = vec![0u32; (w * self.pixel_height()).max(0) as usize];
        for (y, row) in self.pixels.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    out[y * w as usize + x] = argb;
                }
            }
        }
        out
    }
}

/// Where a row's mark goes: between its number and its title, or right ahead of its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkPlace {
    /// After the number, before the title.
    Before,
    /// After the title, ahead of the length.
    #[default]
    After,
}

/// The marks a host set on a classic playlist's rows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RowMarks {
    /// Every mark there is; [`RowMarks::rows`] point into it.
    pub marks: Vec<RowMark>,
    /// Each row's mark, by the row's place in the playlist: an index into
    /// [`RowMarks::marks`], or `None` for a row without one.
    pub rows: Vec<Option<u16>>,
    /// Where the marks go.
    pub place: MarkPlace,
}

impl RowMarks {
    /// Row `row`'s mark, when it has one.
    #[must_use]
    pub fn of(&self, row: usize) -> Option<&RowMark> {
        self.rows
            .get(row)
            .copied()
            .flatten()
            .and_then(|m| self.marks.get(m as usize))
    }
}

/// The room between a mark and the words beside it, in skin pixels.
pub(crate) const MARK_GAP: i32 = 3;
