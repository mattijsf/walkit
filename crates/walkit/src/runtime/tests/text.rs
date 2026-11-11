// SPDX-License-Identifier: GPL-3.0-or-later

//! A modern skin's TrueType text: smoothed unless it sets `antialias="0"`, and then measured
//! and laid out by the host's measures for smoothed text.

use std::rc::Rc;

use super::{SkinRuntime, SKIN_XML};
use crate::draw::{DrawCmd, FontDef, TextCmd};
use crate::files::MemoryFiles;
use crate::host::{Clock, SkinHost, SkinSetup};
use crate::player::FakePlayer;
use crate::text::TextData;

/// A host whose smoothed text is 50 pixels wide on a line of 11 up and 1 down, and whose other
/// text is 30 pixels wide.
struct SmoothHost;

impl SkinHost for SmoothHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(&mut self, _text: &str, _font: Option<&FontDef>, _size: i32, _bold: bool) -> i32 {
        30
    }

    fn smooth_text_advance(
        &mut self,
        _text: &str,
        _font: Option<&FontDef>,
        _size: i32,
        _bold: bool,
    ) -> Option<f32> {
        Some(50.0)
    }

    fn smooth_line_metrics(
        &mut self,
        _font: Option<&FontDef>,
        _size: i32,
        _bold: bool,
    ) -> Option<(f32, f32)> {
        Some((11.0, 1.0))
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}
}

/// The texts drawn by a skin of right-aligned texts in boxes 100 wide and 23 high at y 0.
fn texts(attributes: &[&str]) -> Vec<TextCmd> {
    let objects: String = attributes
        .iter()
        .enumerate()
        .map(|(i, a)| {
            format!(
                r#"<text id="t{i}" x="0" y="0" w="100" h="23" text="Song" font="Arial" fontsize="14" align="right" {a}/>"#
            )
        })
        .collect();
    let player = format!(
        r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="23">{objects}</layout></container>"#
    );
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", SKIN_XML.as_bytes());
    files.insert("player.xml", player.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(SmoothHost),
        Box::new(FakePlayer::new(clock.clone())),
        SkinSetup::default(),
        clock,
    );
    rt.load();
    rt.tick();
    rt.draw_windows()
        .into_iter()
        .flat_map(|(_, cmds)| cmds)
        .filter_map(|c| match c {
            DrawCmd::Text(t) => Some(*t),
            _ => None,
        })
        .collect()
}

#[test]
fn modern_text_is_smoothed_unless_it_sets_antialias_off() {
    let t = texts(&["", r#"antialias="1""#, r#"antialias="0""#]);
    let smooth: Vec<bool> = t.iter().map(|t| t.smooth).collect();
    assert_eq!(smooth, vec![true, true, false]);
}

#[test]
fn smoothed_text_is_measured_and_centered_by_the_hosts_smoothed_measures() {
    let t = texts(&[
        "",
        r#"valign="bottom""#,
        r#"valign="top""#,
        r#"antialias="0""#,
    ]);
    // right-aligned by its smoothed width of 50, the plain one by 30
    assert_eq!(t[0].x, (t[0].rect.right() - 50) as f32);
    assert_eq!(t[3].x, (t[3].rect.right() - 30) as f32);
    // a line of 12 in a box of 23: 5 pixels above it, the baseline 11 below that
    assert_eq!(t[0].baseline, 16.0);
    assert_eq!(t[1].baseline, 16.0, "bottom is centered as well");
    assert_eq!(t[2].baseline, 11.0);
    assert_ne!(t[3].baseline, 16.0, "plain text keeps Windows' line");
}

#[test]
fn antialias_and_the_other_style_attributes_leave_forcefixed_alone() {
    for key in ["antialias", "italic", "underline", "altfont"] {
        let mut t = TextData::default();
        assert!(t.set_param(key, "1"));
        assert!(!t.force_fixed, "{key}");
    }
    let mut t = TextData::default();
    t.set_param("forcefixed", "1");
    assert!(t.force_fixed);
}
