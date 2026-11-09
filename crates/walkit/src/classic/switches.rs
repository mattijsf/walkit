// SPDX-License-Identifier: GPL-3.0-or-later
//! The switches of a classic skin a host keeps in step with the host's settings: the time
//! counting down, the equalizer's auto, the vis mode, the playlist's selection and scroll.

use crate::classic::vis::VisMode;
use crate::runtime::SkinRuntime;

impl SkinRuntime {
    /// Whether the time displays count down, as a click on them switches it.
    pub fn classic_time_remaining(&self) -> bool {
        self.classic_remaining
    }

    /// Makes the time displays count down, or up.
    pub fn set_classic_time_remaining(&mut self, on: bool) {
        if self.classic_remaining != on {
            self.classic_remaining = on;
            self.repaint();
        }
    }

    /// Whether the equalizer's auto button is on.
    pub fn classic_eq_auto(&self) -> bool {
        self.classic_eq_auto
    }

    /// Turns the equalizer's auto button on or off.
    pub fn set_classic_eq_auto(&mut self, on: bool) {
        if self.classic_eq_auto != on {
            self.classic_eq_auto = on;
            self.repaint();
        }
    }

    /// What the main window's vis shows, as a press on it steps it.
    pub fn classic_vis_mode(&self) -> VisMode {
        self.classic_vis_mode
    }

    /// Makes the main window's vis show `mode`.
    pub fn set_classic_vis_mode(&mut self, mode: VisMode) {
        if self.classic_vis_mode != mode {
            self.classic_vis_mode = mode;
            self.repaint();
        }
    }

    /// Shows `rows` as the classic playlist's selection, as the host's playlist has it after its
    /// select all or invert; rows past the list are left out, and the host is not told back.
    pub fn set_classic_playlist_selection(&mut self, rows: &[i32]) {
        let len = self.player.tracks().len() as i32;
        let mut rows: Vec<i32> = rows
            .iter()
            .copied()
            .filter(|&r| r >= 0 && r < len)
            .collect();
        rows.sort_unstable();
        rows.dedup();
        let first = rows.first().copied();
        let more = rows.into_iter().skip(1).collect::<Vec<_>>();
        if self.classic_selected_row != first || self.classic_selected_more != more {
            self.classic_selected_row = first;
            self.classic_selected_more = more;
            self.repaint();
        }
    }

    /// Whether the classic windows switch to their shade bars, from their shade buttons and a
    /// double tap on their title bars.
    pub fn classic_shades(&self) -> bool {
        self.classic_shades
    }

    /// Lets the classic windows switch to their shade bars, or not; turning it off puts every
    /// shaded one back in its normal layout.
    pub fn set_classic_shades(&mut self, on: bool) {
        if self.classic_shades == on {
            return;
        }
        self.classic_shades = on;
        if on {
            return;
        }
        for id in ["main", "equalizer", "pledit"] {
            let Some(c) = self.container(id) else {
                continue;
            };
            let shade = self.container_layout(c, "shade");
            if shade.is_none() || self.containers[c].current != shade {
                continue;
            }
            if self.is_open(c) {
                self.switch_to(c, "normal");
            } else {
                // a closed window opens again in its normal layout
                self.containers[c].current = self.container_layout(c, "normal");
            }
        }
        self.repaint();
    }

    /// Puts `marks` on the classic playlist's rows, as the host's playlist marks them; the host
    /// sets them again when its playlist changes. Empty marks take them off.
    pub fn set_classic_row_marks(&mut self, marks: crate::classic::marks::RowMarks) {
        if *self.classic_row_marks != marks {
            self.classic_row_marks = std::rc::Rc::new(marks);
            self.repaint();
        }
    }

    /// Scrolls the playlist by `rows`, down for more than 0, within its songs.
    pub fn scroll_classic_playlist(&mut self, rows: i32) {
        let Some(h) = self.object_info("classic.pl").map(|o| o.rect.height) else {
            return;
        };
        let past = self.player.tracks().len() as i32 - self.classic_pl_rows(h);
        let to = (self.classic_pl_scroll(h) + rows).clamp(0, past.max(0));
        if to != self.classic_pl_scroll {
            self.classic_pl_scroll = to;
            self.repaint();
        }
    }
}
