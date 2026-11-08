// SPDX-License-Identifier: GPL-3.0-or-later

//! Winamp's built-in lists as a skin shows them: the color themes list, the playlist editor's
//! list, and a window's status bar.

use super::SkinRuntime;
use crate::draw::{Align, BitmapDef, DrawCmd, FontDef, Rect, Rgb, TextCmd, VAlign};
use crate::gui::{to_bool, Kind, ObjId};
use crate::maki::{Handle, Value};
use crate::text::time;

const ROW: i32 = 18;
const ROW_OLD: i32 = 14;
const HEADER: i32 = 12;
/// Text starts 4 px in from the list's left edge, in rows and header alike.
const TEXT_LEFT: i32 = 4;
/// Wasabi lists draw in the skin's default font at size 16 (12 px).
const LIST_FONT_SIZE: i32 = 16;
const DRAG_SLOP: i32 = 6;
const BACKGROUND: &str = "wasabi.list.background";
const UP: &str = "wasabi.scrollbar.vertical.left";
const DOWN: &str = "wasabi.scrollbar.vertical.right";
const THUMB: &str = "wasabi.scrollbar.vertical.button";
const TRACK: &str = "wasabi.scrollbar.vertical.background.middle";
const LEFT: &str = "wasabi.scrollbar.horizontal.left";
const RIGHT: &str = "wasabi.scrollbar.horizontal.right";
const HTHUMB: &str = "wasabi.scrollbar.horizontal.button";
const HTRACK: &str = "wasabi.scrollbar.horizontal.background.middle";

const PL_ROW: i32 = 15;
const PL_TOP: i32 = 3;
const PL_TEXT_RIGHT: i32 = 18;
const PL_TEXT_LEFT: i32 = 2;
/// Space kept between a cut title and its time.
const PL_TITLE_GAP: i32 = 1;
const PL_LEFT_BORDER: i32 = 2;
/// Right of the rows: 5 px gray, a light line, the 6 px channel, a light line, 2 px of border.
const PL_BAR: i32 = 15;
const PL_CHANNEL_X: i32 = 6;
const PL_CHANNEL_W: i32 = 6;
const PL_THUMB_MIN: i32 = 17;
const PL_GRIP: &str = "wasabi.scrollbar.vertical.grip";
const PL_BORDER: Rgb = Rgb(0x7F8795);
const PL_LIGHT: Rgb = Rgb(0xCCD0D9);

const STATUS_PLAYLIST: &str = "45f3f7c1-a6f3-4ee6-a15e-125e92fc3f8d";
const STATUS_LEFT: [&str; 4] = ["Add", "Rem", "Sel", "Misc"];
const STATUS_RIGHT: [&str; 1] = ["List"];
/// Winamp's own buttons: Arial at text size 14, which draws at 11 px (Wasabi SDK buttwnd).
const STATUS_FONT_SIZE: i32 = 14;
/// Buttons 13 px tall, 1 px into the bar; a cell at least 8 wide, its last 2 px the bar's.
const STATUS_BUTTON_H: i32 = 13;
const STATUS_MIN_W: i32 = 8;
const STATUS_CELL_MARGIN: i32 = 2;

#[allow(clippy::too_many_arguments)]
fn text_cmd(
    text: String,
    font: Option<FontDef>,
    size: i32,
    color: Rgb,
    rect: Rect,
    align: Align,
    alpha: i32,
    bold: bool,
) -> DrawCmd {
    DrawCmd::Text(Box::new(TextCmd {
        text,
        font,
        size,
        color,
        rect,
        align,
        scroll: 0,
        alpha,
        valign: VAlign::Center,
        bold,
        shadow: None,
        shadow_dx: 1,
        shadow_dy: 1,
        x: 0.0,
        baseline: 0.0,
    }))
}

impl SkinRuntime {
    // ---- the color themes list ---------------------------------------------------------------

    fn themes(&self, o: ObjId) -> &crate::gui::ThemeListData {
        match &self.objects[o].kind {
            Kind::ColorThemeList(t) => t,
            _ => panic!("not a themes list"),
        }
    }

    fn themes_mut(&mut self, o: ObjId) -> &mut crate::gui::ThemeListData {
        match &mut self.objects[o].kind {
            Kind::ColorThemeList(t) => t,
            _ => panic!("not a themes list"),
        }
    }

    /// The list's attributes beyond the base ones.
    pub(crate) fn themes_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        match k {
            "nocolheader" => self.themes_mut(o).header = value != "1",
            "nohscroll" => self.themes_mut(o).hscroll = value != "1",
            "nocols" => {}
            _ => return false,
        }
        true
    }

    /// The themes, sorted by name ignoring case.
    fn theme_names(&self) -> Vec<String> {
        let mut names = self.color_themes();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    fn themes_old(&self) -> bool {
        self.wal_version < 1.0
    }

    fn themes_row_h(&self) -> i32 {
        if self.themes_old() {
            ROW_OLD
        } else {
            ROW
        }
    }

    fn themes_header_h(&self, o: ObjId) -> i32 {
        if self.themes(o).header {
            HEADER
        } else {
            0
        }
    }

    fn themes_bar_w(&mut self) -> i32 {
        self.bitmap_size(UP).map(|s| s.0).unwrap_or(0)
    }

    fn themes_hbar_h(&mut self, o: ObjId) -> i32 {
        if self.themes(o).hscroll {
            self.bitmap_size(LEFT).map(|s| s.1).unwrap_or(0)
        } else {
            0
        }
    }

    /// Where the rows show: under the header, left of the vertical bar, above the horizontal one.
    fn themes_rows_area(&mut self, o: ObjId) -> Rect {
        let rect = self.objects[o].base.rect;
        let inset = if self.themes_old() { 2 } else { 1 };
        let header_h = self.themes_header_h(o);
        let bar_w = self.themes_bar_w();
        let hbar_h = self.themes_hbar_h(o);
        Rect::new(
            rect.left + inset,
            rect.top + header_h,
            rect.width - inset - bar_w,
            rect.height - header_h - hbar_h,
        )
    }

    fn themes_max_scroll(&mut self, o: ObjId) -> i32 {
        let n = self.theme_names().len() as i32;
        let rows = self.themes_rows_area(o);
        (n * self.themes_row_h() - rows.height).max(0)
    }

    /// Selects the theme in use and scrolls it into the middle, as Winamp does when the list appears.
    fn themes_follow_current(&mut self, o: ObjId) {
        let current = self.color_theme().map(|s| s.to_string());
        if current == self.themes(o).shown_for {
            return;
        }
        self.themes_mut(o).shown_for.clone_from(&current);
        self.themes_mut(o).selected.clone_from(&current);
        let names = self.theme_names();
        let Some(index) = current.and_then(|c| names.iter().position(|n| *n == c)) else {
            return;
        };
        let rect = self.objects[o].base.rect;
        let row_h = self.themes_row_h();
        // where Winamp 5.666 puts the row (measured on both skins; the horizontal bar does not count)
        let middle = if self.themes(o).header {
            (rect.height - HEADER - row_h) / 2 - 2
        } else {
            (rect.height - 6 - row_h) / 2
        };
        let max = self.themes_max_scroll(o);
        self.themes_mut(o).scroll_px = (index as i32 * row_h - middle).clamp(0, max.max(0));
    }

    /// The list's drawing.
    pub(crate) fn draw_theme_list(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let alpha = self.objects[o].base.effective_alpha;
        self.themes_follow_current(o);
        let text = self.color("wasabi.list.text").unwrap_or(Rgb(0xFFFFFF));
        let sel_text = self
            .color("wasabi.list.text.selected")
            .or_else(|| self.color("wasabi.list.item.selected.fg"))
            .unwrap_or(text);
        let sel_back = self
            .color("wasabi.list.text.selected.background")
            .or_else(|| self.color("wasabi.list.item.selected"))
            .unwrap_or(Rgb(0x6788C9));
        let rows = self.themes_rows_area(o);
        let scroll = self.themes(o).scroll_px;
        // the background tile under the rows only: the scrollbars sit on whatever is behind the list
        // (MMD3's black group background), and the header paints its own color. The tile
        // scrolls with the rows: its origin is where row 0 would be (measured on MMD3's dot pattern)
        if let (Some(tile), Some((tw, th))) =
            (self.bitmap(BACKGROUND), self.bitmap_size(BACKGROUND))
        {
            out.push(DrawCmd::Clip(Rect::new(
                rect.left,
                rows.top,
                rows.right() - rect.left,
                rows.height,
            )));
            let phase = scroll.rem_euclid(th.max(1));
            tile_over(
                out,
                &tile,
                tw,
                th,
                Rect::new(
                    rect.left,
                    rows.top - phase,
                    rows.right() - rect.left,
                    rows.height + phase,
                ),
                alpha,
            );
            out.push(DrawCmd::Unclip);
        }
        out.push(DrawCmd::Clip(rows));
        let all = self.theme_names();
        let row_h = self.themes_row_h();
        let selected = self.themes(o).selected.clone();
        let current = self.color_theme().map(|s| s.to_string());
        let first = (scroll / row_h) as usize;
        for (i, name) in all.iter().enumerate().skip(first) {
            let y = rows.top + i as i32 * row_h - scroll;
            if y >= rows.bottom() {
                break;
            }
            let row = Rect::new(rows.left, y, rows.width, row_h);
            let is_selected = selected
                .as_ref()
                .is_some_and(|s| s.eq_ignore_ascii_case(name));
            if is_selected {
                out.push(DrawCmd::Fill {
                    rect: row,
                    color: sel_back,
                    alpha,
                });
            }
            let bold = current
                .as_ref()
                .is_some_and(|c| c.eq_ignore_ascii_case(name));
            out.push(text_cmd(
                name.clone(),
                None,
                LIST_FONT_SIZE,
                if is_selected { sel_text } else { text },
                Rect::new(
                    rect.left + TEXT_LEFT,
                    row.top,
                    row.right() - rect.left - TEXT_LEFT,
                    row.height,
                ),
                Align::Left,
                alpha,
                bold,
            ));
        }
        out.push(DrawCmd::Unclip);
        if self.themes(o).header {
            let bg = self
                .color("wasabi.list.column.background")
                .unwrap_or(Rgb(0x666666));
            let fg = self
                .color("wasabi.list.column.text")
                .unwrap_or(Rgb(0xFFFFFF));
            out.push(DrawCmd::Fill {
                rect: Rect::new(rect.left, rect.top, rect.width, HEADER),
                color: bg,
                alpha,
            });
            out.push(text_cmd(
                "Theme".into(),
                None,
                LIST_FONT_SIZE,
                fg,
                Rect::new(
                    rect.left + TEXT_LEFT,
                    rect.top,
                    rect.width - TEXT_LEFT,
                    HEADER,
                ),
                Align::Left,
                alpha,
                false,
            ));
        }
        self.draw_theme_scrollbars(o, out, alpha);
    }

    /// Winamp's list draws its own scrollbars: arrows at the ends, the thumb along the track by
    /// how far the list is scrolled, all tinted like the arrows. The vertical bar stops above
    /// the horizontal one; the square between them is black.
    fn draw_theme_scrollbars(&mut self, o: ObjId, out: &mut Vec<DrawCmd>, alpha: i32) {
        let bw = self.themes_bar_w();
        let hh = self.themes_hbar_h(o);
        // a vertical bar with a picture missing leaves out the horizontal bar too
        if bw > 0 && !self.draw_theme_vertical_bar(o, out, bw, hh, alpha) {
            return;
        }
        if hh > 0 {
            self.draw_theme_horizontal_bar(o, out, bw, hh, alpha);
        }
    }

    fn draw_theme_vertical_bar(
        &mut self,
        o: ObjId,
        out: &mut Vec<DrawCmd>,
        bw: i32,
        hh: i32,
        alpha: i32,
    ) -> bool {
        let rect = self.objects[o].base.rect;
        let (Some(up), Some(down), Some(thumb)) =
            (self.bitmap(UP), self.bitmap(DOWN), self.bitmap(THUMB))
        else {
            return false;
        };
        let group = up.gamma_group.clone();
        let x = rect.right() - bw;
        let bar_h = rect.height - hh;
        let up_h = self.bitmap_size(UP).map(|s| s.1).unwrap_or(0);
        let down_h = self.bitmap_size(DOWN).map(|s| s.1).unwrap_or(0);
        let thumb_h = self.bitmap_size(THUMB).map(|s| s.1).unwrap_or(0);
        // the track runs under the arrows too, so their rounded corners show it
        if let Some(track) = self.bitmap(TRACK) {
            out.push(DrawCmd::image(
                tinted(track, group.as_ref()),
                Rect::new(x, rect.top, bw, bar_h),
                alpha,
            ));
        }
        out.push(DrawCmd::image(up, Rect::new(x, rect.top, bw, up_h), alpha));
        out.push(DrawCmd::image(
            down,
            Rect::new(x, rect.top + bar_h - down_h, bw, down_h),
            alpha,
        ));
        let travel = (bar_h - up_h - down_h - thumb_h).max(0);
        let max = self.themes_max_scroll(o);
        let scroll = self.themes(o).scroll_px;
        let thumb_top = rect.top + up_h + if max == 0 { 0 } else { travel * scroll / max };
        out.push(DrawCmd::image(
            thumb,
            Rect::new(x, thumb_top, bw, thumb_h),
            alpha,
        ));
        true
    }

    fn draw_theme_horizontal_bar(
        &mut self,
        o: ObjId,
        out: &mut Vec<DrawCmd>,
        bw: i32,
        hh: i32,
        alpha: i32,
    ) {
        let rect = self.objects[o].base.rect;
        let (Some(left), Some(right), Some(thumb)) =
            (self.bitmap(LEFT), self.bitmap(RIGHT), self.bitmap(HTHUMB))
        else {
            return;
        };
        let group = left.gamma_group.clone();
        let y = rect.bottom() - hh;
        let bar_w = rect.width - bw;
        let left_w = self.bitmap_size(LEFT).map(|s| s.0).unwrap_or(0);
        let right_w = self.bitmap_size(RIGHT).map(|s| s.0).unwrap_or(0);
        let thumb_w = self.bitmap_size(HTHUMB).map(|s| s.0).unwrap_or(0);
        if let Some(track) = self.bitmap(HTRACK) {
            out.push(DrawCmd::image(
                tinted(track, group.as_ref()),
                Rect::new(rect.left, y, bar_w, hh),
                alpha,
            ));
        }
        out.push(DrawCmd::image(
            left,
            Rect::new(rect.left, y, left_w, hh),
            alpha,
        ));
        out.push(DrawCmd::image(
            right,
            Rect::new(rect.left + bar_w - right_w, y, right_w, hh),
            alpha,
        ));
        // nothing scrolls sideways here: the thumb sits at the start
        out.push(DrawCmd::image(
            thumb,
            Rect::new(rect.left + left_w, y, thumb_w, hh),
            alpha,
        ));
        if bw > 0 {
            out.push(DrawCmd::Fill {
                rect: Rect::new(rect.right() - bw, y, bw, hh),
                color: Rgb(0),
                alpha,
            });
        }
    }

    pub(crate) fn themes_pointer_down(&mut self, o: ObjId, py: i32) {
        let t = self.themes_mut(o);
        t.down_y = py;
        t.down_scroll = t.scroll_px;
        t.dragged = false;
    }

    pub(crate) fn themes_pointer_move(&mut self, o: ObjId, py: i32) {
        let t = self.themes(o);
        if !t.dragged && (py - t.down_y).abs() < DRAG_SLOP {
            return;
        }
        let max = self.themes_max_scroll(o);
        let t = self.themes_mut(o);
        t.dragged = true;
        t.scroll_px = (t.down_scroll - (py - t.down_y)).clamp(0, max.max(0));
        self.invalidate();
    }

    pub(crate) fn themes_pointer_up(&mut self, o: ObjId, py: i32, inside: bool) {
        if self.themes(o).dragged || !inside {
            return;
        }
        let rows = self.themes_rows_area(o);
        if py < rows.top || py >= rows.bottom() {
            return;
        }
        let row_h = self.themes_row_h();
        let index = ((py - rows.top + self.themes(o).scroll_px) / row_h) as usize;
        let Some(name) = self.theme_names().get(index).cloned() else {
            return;
        };
        if self
            .themes(o)
            .selected
            .as_ref()
            .is_some_and(|s| s.eq_ignore_ascii_case(&name))
        {
            self.themes_switch_to_selected(o);
        } else {
            self.themes_mut(o).selected = Some(name);
        }
        self.invalidate();
    }

    /// Switches to the highlighted theme.
    pub(crate) fn themes_switch_to_selected(&mut self, o: ObjId) {
        if let Some(s) = self.themes(o).selected.clone() {
            self.select_color_theme(&s);
        }
        self.themes_mut(o).shown_for = self.color_theme().map(|s| s.to_string());
    }

    /// Previous and next switch straight to the neighboring theme, with wraparound.
    pub(crate) fn themes_step(&mut self, o: ObjId, delta: i32) {
        let all = self.theme_names();
        if all.is_empty() {
            return;
        }
        let current = self.color_theme().map(|s| s.to_string());
        let index = all
            .iter()
            .position(|n| current.as_ref().is_some_and(|c| c.eq_ignore_ascii_case(n)))
            .map(|i| i as i32)
            .unwrap_or(-1);
        let next = all[(index + delta).rem_euclid(all.len() as i32) as usize].clone();
        self.select_color_theme(&next);
        self.themes_mut(o).shown_for = None;
    }

    // ---- the playlist editor's list ----------------------------------------------------------

    fn pl(&self, o: ObjId) -> &crate::gui::PlaylistData {
        match &self.objects[o].kind {
            Kind::PlaylistView(p) => p,
            _ => panic!("not a playlist view"),
        }
    }

    fn pl_mut(&mut self, o: ObjId) -> &mut crate::gui::PlaylistData {
        match &mut self.objects[o].kind {
            Kind::PlaylistView(p) => p,
            _ => panic!("not a playlist view"),
        }
    }

    /// The rows' area, as Winamp 5.666 frames it.
    fn pl_list_rect(&self, o: ObjId) -> Rect {
        let rect = self.objects[o].base.rect;
        Rect::new(
            rect.left + PL_LEFT_BORDER,
            rect.top,
            rect.width - PL_LEFT_BORDER - PL_BAR,
            rect.height,
        )
    }

    fn pl_max_scroll(&self, o: ObjId) -> i32 {
        (PL_TOP + self.player.tracks().len() as i32 * PL_ROW - self.pl_list_rect(o).height).max(0)
    }

    /// The list's drawing: rows in the skin's list colors, the current entry in its color, a
    /// narrow scrollbar at the right edge.
    pub(crate) fn draw_playlist(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        if rect.width <= PL_BAR || rect.height <= 0 {
            return;
        }
        let alpha = self.objects[o].base.effective_alpha;
        let tracks = self.player.tracks();
        let border = self.color("wasabi.window.background").unwrap_or(PL_BORDER);
        // a skin without list colors (Winamp3) gets black behind the rows, the current one in gray
        let back = self.color("wasabi.list.background").unwrap_or(Rgb(0));
        let text = self.color("wasabi.list.text").unwrap_or(Rgb(0xFFFFFF));
        let current = self
            .color("wasabi.list.text.current")
            .unwrap_or(Rgb(0x7F7F7F));
        let sel_text = self.color("wasabi.list.text.selected").unwrap_or(text);
        let sel_back = self
            .color("wasabi.list.text.selected.background")
            .unwrap_or(Rgb(0x6788C9));
        let list = self.pl_list_rect(o);
        out.push(DrawCmd::Fill {
            rect,
            color: border,
            alpha,
        });
        out.push(DrawCmd::Fill {
            rect: list,
            color: back,
            alpha,
        });
        out.push(DrawCmd::Clip(list));
        let scroll = self.pl(o).scroll_px;
        let selected = self.pl(o).selected;
        let track_index = self.player.track_index();
        let rows_top = rect.top + PL_TOP;
        let digits = tracks.len().to_string().len();
        let first = (scroll / PL_ROW) as usize;
        for (i, t) in tracks.iter().enumerate().skip(first) {
            let y = rows_top + i as i32 * PL_ROW - scroll;
            // only rows that fit whole are drawn (WinampModern-pllong)
            if y + PL_ROW > list.bottom() {
                break;
            }
            let row = Rect::new(list.left, y, list.width, PL_ROW);
            let color = if i as i32 == selected {
                sel_text
            } else if i as i32 == track_index {
                current
            } else {
                text
            };
            if i as i32 == selected {
                out.push(DrawCmd::Fill {
                    rect: row,
                    color: sel_back,
                    alpha,
                });
            }
            // text from 1 px inside the component's left edge to 18 px inside its right edge
            let inner = Rect::new(
                rect.left + PL_TEXT_LEFT,
                row.top,
                rect.width - PL_TEXT_LEFT - PL_TEXT_RIGHT,
                row.height,
            );
            let length = time(t.length_ms);
            // a title that does not fit before the time ends in "..." a few px short of it
            let room = inner.width
                - self.host.text_width(&length, None, LIST_FONT_SIZE, false)
                - PL_TITLE_GAP;
            // the numbers padded to the longest one's digits, with spaces (" 1." in a list of 20)
            let number = format!("{:>width$}", i + 1, width = digits);
            let title = self.fitted(&format!("{number}. {}", t.title), room);
            out.push(text_cmd(
                title,
                None,
                LIST_FONT_SIZE,
                color,
                inner,
                Align::Left,
                alpha,
                false,
            ));
            out.push(text_cmd(
                length,
                None,
                LIST_FONT_SIZE,
                color,
                inner,
                Align::Right,
                alpha,
                false,
            ));
        }
        out.push(DrawCmd::Unclip);
        self.draw_playlist_scrollbar(o, out, border, alpha);
    }

    /// Winamp's list scrollbar, as 5.666 draws it: 5 px of the frame's gray, a light line, a
    /// 6 px channel in the skin's `wasabi.scrollbar.background` color, a light line and 2 px of
    /// border; no arrows. The skin's thumb picture is shrunk, with filtering, to 8 px over the
    /// channel and its two light lines, 17 px tall when nothing scrolls.
    fn draw_playlist_scrollbar(
        &mut self,
        o: ObjId,
        out: &mut Vec<DrawCmd>,
        border: Rgb,
        alpha: i32,
    ) {
        let rect = self.objects[o].base.rect;
        let light = self.color("wasabi.border.sunken").unwrap_or(PL_LIGHT);
        let x = rect.right() - PL_BAR;
        // without a skin color the channel is dark gray (Winamp3)
        let channel = self
            .color("wasabi.scrollbar.background")
            .unwrap_or(Rgb(0x333333));
        out.push(DrawCmd::Fill {
            rect: Rect::new(x, rect.top, PL_CHANNEL_X - 1, rect.height),
            color: border,
            alpha,
        });
        out.push(DrawCmd::Fill {
            rect: Rect::new(x + PL_CHANNEL_X - 1, rect.top, 1, rect.height),
            color: light,
            alpha,
        });
        out.push(DrawCmd::Fill {
            rect: Rect::new(x + PL_CHANNEL_X, rect.top, PL_CHANNEL_W, rect.height),
            color: channel,
            alpha,
        });
        out.push(DrawCmd::Fill {
            rect: Rect::new(x + PL_CHANNEL_X + PL_CHANNEL_W, rect.top, 1, rect.height),
            color: light,
            alpha,
        });
        // the thumb keeps its size however long the list (Wasabi SDK scrollbar; WinampModern-pllong)
        let thumb_h = PL_THUMB_MIN;
        let travel = (rect.height - 2 - thumb_h).max(0);
        let max = self.pl_max_scroll(o);
        let scroll = self.pl(o).scroll_px;
        let top = rect.top + 1 + if max == 0 { 0 } else { travel * scroll / max };
        match self.bitmap(THUMB) {
            Some(thumb) => {
                out.push(DrawCmd::Image {
                    bitmap: thumb,
                    dst: Rect::new(x + PL_CHANNEL_X - 1, top, PL_CHANNEL_W + 2, thumb_h),
                    alpha,
                    gamma: None,
                    smooth: true,
                });
                // the skin's grip picture at its own size, centered on the thumb
                if let Some(g) = self.bitmap(PL_GRIP) {
                    let (gw, gh) = self.bitmap_size(PL_GRIP).unwrap_or((0, 0));
                    let gx = x + PL_CHANNEL_X - 1 + (PL_CHANNEL_W + 2 - gw) / 2;
                    let gy = top + (thumb_h - gh) / 2;
                    out.push(DrawCmd::image(g, Rect::new(gx, gy, gw, gh), alpha));
                }
            }
            None => out.push(DrawCmd::Fill {
                rect: Rect::new(x + PL_CHANNEL_X, top, PL_CHANNEL_W, thumb_h),
                color: light,
                alpha,
            }),
        }
    }

    /// `text` as it fits in `width` px: whole, or cut with "..." after it.
    fn fitted(&mut self, text: &str, width: i32) -> String {
        if self.host.text_width(text, None, LIST_FONT_SIZE, false) <= width {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let mut end = chars.len();
        while end > 0 {
            let cut: String = chars[..end].iter().collect();
            let cut = format!("{}...", cut.trim_end());
            if self.host.text_width(&cut, None, LIST_FONT_SIZE, false) <= width {
                break;
            }
            end -= 1;
        }
        let cut: String = chars[..end].iter().collect();
        format!("{}...", cut.trim_end())
    }

    pub(crate) fn pl_pointer_down(&mut self, o: ObjId, py: i32) {
        let p = self.pl_mut(o);
        p.down_y = py;
        p.down_scroll = p.scroll_px;
        p.dragged = false;
    }

    pub(crate) fn pl_pointer_move(&mut self, o: ObjId, py: i32) {
        let p = self.pl(o);
        if !p.dragged && (py - p.down_y).abs() < DRAG_SLOP {
            return;
        }
        let max = self.pl_max_scroll(o);
        let p = self.pl_mut(o);
        p.dragged = true;
        p.scroll_px = (p.down_scroll - (py - p.down_y)).clamp(0, max.max(0));
        self.invalidate();
    }

    pub(crate) fn pl_pointer_up(&mut self, o: ObjId, py: i32, inside: bool) {
        if self.pl(o).dragged || !inside {
            return;
        }
        let Some(index) = self.pl_row_at(o, py) else {
            return;
        };
        if index == self.pl(o).selected {
            self.player.play_track(index);
        } else {
            self.pl_select(o, index);
        }
        self.pl_selection_changed(o);
    }

    /// The second tap of a double tap: the row under it is selected and plays.
    pub(crate) fn pl_double_tap(&mut self, o: ObjId, px: i32, py: i32) {
        self.dispatch(
            Handle::Gui(o),
            "onleftbuttondblclk",
            &[Value::Int(px), Value::Int(py)],
        );
        if let Some(a) = self.objects[o].base.dbl_click_action.clone() {
            self.run_action(o, &a);
        }
        let Some(index) = self.pl_row_at(o, py) else {
            return;
        };
        self.pl_select(o, index);
        self.player.play_track(index);
        self.pl_selection_changed(o);
    }

    /// Selects entry `index` and tells the host, which keeps the selection its playlist menus act
    /// on, as a classic skin's playlist does (`PLAYLIST_SELECT` with the entry).
    fn pl_select(&mut self, o: ObjId, index: i32) {
        if self.pl(o).selected == index {
            return;
        }
        self.pl_mut(o).selected = index;
        self.action(o, "PLAYLIST_SELECT", Some(&index.to_string()), None);
    }

    /// The entry whose row is at `py`, a point of the window, or `None` past the last one.
    fn pl_row_at(&self, o: ObjId, py: i32) -> Option<i32> {
        let rows_top = self.objects[o].base.rect.top + PL_TOP;
        let index = (py - rows_top + self.pl(o).scroll_px) / PL_ROW;
        if index >= 0 && (index as usize) < self.player.tracks().len() {
            Some(index)
        } else {
            None
        }
    }

    fn pl_selection_changed(&mut self, o: ObjId) {
        let selected = self.pl(o).selected;
        self.playlist_selected_ms = usize::try_from(selected)
            .ok()
            .and_then(|i| self.player.tracks().get(i).map(|t| t.length_ms))
            .unwrap_or(0);
        self.invalidate();
    }

    // ---- the status bar ----------------------------------------------------------------------

    /// The status bar's attributes beyond the base ones.
    pub(crate) fn status_bar_set_param(&mut self, o: ObjId, k: &str, value: &str) -> bool {
        let ids = |v: &str| -> Vec<String> {
            v.split([';', ','])
                .map(|s| s.trim().trim_matches(['{', '}']).to_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        };
        let Kind::LayoutStatus(d) = &mut self.objects[o].kind else {
            return false;
        };
        match k {
            "includeonly" => d.include_only = Some(ids(value)),
            "exclude" => d.exclude = ids(value),
            "bg" => d.background = to_bool(value),
            _ => return false,
        }
        true
    }

    /// The commands of the components in this window: left-hand ones, right-hand ones.
    fn status_commands(&self, o: ObjId) -> (Vec<&'static str>, Vec<&'static str>) {
        fn has_playlist(rt: &SkinRuntime, o: ObjId) -> bool {
            matches!(rt.objects[o].kind, Kind::PlaylistView(_))
                || rt.objects[o]
                    .children()
                    .iter()
                    .any(|c| has_playlist(rt, *c))
        }
        let Kind::LayoutStatus(d) = &self.objects[o].kind else {
            return (Vec::new(), Vec::new());
        };
        let shows = d
            .include_only
            .as_ref()
            .is_none_or(|i| i.iter().any(|s| s == STATUS_PLAYLIST))
            && !d.exclude.iter().any(|s| s == STATUS_PLAYLIST);
        let Some(window) = self.layout_object(o) else {
            return (Vec::new(), Vec::new());
        };
        if has_playlist(self, window) && shows {
            (STATUS_LEFT.to_vec(), STATUS_RIGHT.to_vec())
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// A command button's cell: its name between two "j"s, 11/10 of that, at least 8 (Wasabi SDK buttwnd).
    fn status_button_width(&mut self, text: &str) -> i32 {
        (self.host.text_width(
            &format!("j{text}j"),
            Some(&FontDef::new("arial", "")),
            STATUS_FONT_SIZE,
            false,
        ) * 11
            / 10)
            .max(STATUS_MIN_W)
    }

    /// The status bar's drawing: the command buttons and the field between them.
    pub(crate) fn draw_status_bar(&mut self, o: ObjId, out: &mut Vec<DrawCmd>) {
        let rect = self.objects[o].base.rect;
        if rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let alpha = self.objects[o].base.effective_alpha;
        let (left, right) = self.status_commands(o);
        let background = match &self.objects[o].kind {
            Kind::LayoutStatus(d) => d.background,
            _ => true,
        };
        let top = rect.top + 1;
        let h = STATUS_BUTTON_H;
        // the cells sit edge to edge; each button leaves the last 2 px of its cell to the bar (Wasabi SDK buttbar: no spacer)
        let mut x = rect.left + 1;
        for c in &left {
            let w = self.status_button_width(c);
            self.status_button(out, c, Rect::new(x, top, w - STATUS_CELL_MARGIN, h), alpha);
            x += w;
        }
        let mut r = rect.right() - 1;
        let mut right_rects = Vec::new();
        for c in right.iter().rev() {
            let w = self.status_button_width(c);
            r -= w;
            right_rects.push(Rect::new(r, top, w - STATUS_CELL_MARGIN, h));
        }
        right_rects.reverse();
        // the status text field between the two groups of buttons
        if background {
            out.push(DrawCmd::Fill {
                rect: Rect::new(x, top, (r - x).max(0), h),
                color: Rgb(0),
                alpha,
            });
        }
        for (c, rr) in right.iter().zip(right_rects) {
            self.status_button(out, c, rr, alpha);
        }
    }

    fn status_button(&mut self, out: &mut Vec<DrawCmd>, text: &str, r: Rect, alpha: i32) {
        self.draw_framed_button(out, r, alpha, false);
        out.push(text_cmd(
            text.to_string(),
            Some(FontDef::new("arial", "")),
            STATUS_FONT_SIZE,
            Rgb(0),
            Rect::new(r.left + 1, r.top + 1, r.width - 2, r.height - 2),
            Align::Center,
            alpha,
            false,
        ));
    }
}

/// `tile` repeated over `area`, each tile `tw` x `th`, the last column and row cut to fit.
fn tile_over(out: &mut Vec<DrawCmd>, tile: &BitmapDef, tw: i32, th: i32, area: Rect, alpha: i32) {
    if tw <= 0 || th <= 0 {
        return;
    }
    let mut y = area.top;
    while y < area.bottom() {
        let mut x = area.left;
        while x < area.right() {
            let w = tw.min(area.right() - x);
            let h = th.min(area.bottom() - y);
            let part = if w == tw && h == th {
                tile.clone()
            } else {
                tile.part(tile.x, tile.y, Some(w), Some(h))
            };
            out.push(DrawCmd::image(part, Rect::new(x, y, w, h), alpha));
            x += tw;
        }
        y += th;
    }
}

/// A scrollbar part tinted like the arrows: a track picture names no gammagroup, and Winamp
/// still tints it "Scrollbar Buttons".
fn tinted(b: BitmapDef, group: Option<&std::rc::Rc<str>>) -> BitmapDef {
    if b.gamma_group.is_some() {
        b
    } else {
        BitmapDef {
            gamma_group: group.cloned(),
            ..b
        }
    }
}
