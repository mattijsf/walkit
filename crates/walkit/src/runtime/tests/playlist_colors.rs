// SPDX-License-Identifier: GPL-3.0-or-later

//! The playlist's scrollbar: its colors on a skin older than version 1, which names none of
//! them, worked out from the window's background as Winamp does; and its thumb.

use super::runtime_files;
use crate::draw::{DrawCmd, Rgb};
use crate::host::SkinSetup;

const OLD_SKIN_XML: &str = r#"<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>"#;

/// A window of one `color` with a playlist in it, and the skin's list header color.
fn player(color: &str, column: &str) -> String {
    format!(
        r#"
<elements>
  <bitmap id="back" file="$solid" w="8" h="8" color="{color}"/>
  <color id="studio.list.column.background" value="{column}"/>
</elements>
<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="120">
<layer id="bg" x="0" y="0" w="0" h="0" relatw="1" relath="1" image="back"/>
<component id="list" param="guid:{{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}}" x="10" y="10" w="180" h="100"/>
</layout></container>"#
    )
}

/// The scrollbar's border, light line and channel colors.
fn bar_colors(color: &str, column: &str) -> (Rgb, Rgb, Rgb) {
    let mut rt = runtime_files(OLD_SKIN_XML, &player(color, column), SkinSetup::default());
    rt.load();
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, cmds) = windows.first().expect("the main window");
    let fill = |width: i32| {
        cmds.iter()
            .find_map(|c| match c {
                DrawCmd::Fill { rect, color, .. } if rect.width == width && rect.height == 100 => {
                    Some(*color)
                }
                _ => None,
            })
            .expect("a scrollbar fill")
    };
    // 5 px of border, a light line, the 6 px channel
    (fill(5), fill(1), fill(6))
}

#[test]
fn the_window_behind_the_list_gives_the_border_and_light_lines() {
    let (border, light, _) = bar_colors("164,93,1", "102,102,102");
    assert_eq!(border, Rgb(0xA4_5D01));
    // half of it plus 7f7f7f
    assert_eq!(light, Rgb(0xD1_AD7F));
}

#[test]
fn the_channel_is_half_the_list_header_color() {
    let (_, _, channel) = bar_colors("164,93,1", "102,102,102");
    assert_eq!(channel, Rgb(0x33_3333));
}

#[test]
fn a_dark_list_header_gives_way_to_a_quarter_of_the_window_with_red_and_blue_swapped() {
    // MMD3's orange theme in Winamp 5.666: a navy channel
    let (_, _, channel) = bar_colors("164,93,1", "0,0,0");
    assert_eq!(channel, Rgb(0x00_1729));
    // and its green one
    let (_, _, channel) = bar_colors("18,83,0", "0,0,0");
    assert_eq!(channel, Rgb(0x00_1404));
}

#[test]
fn a_bright_window_gives_light_lines_of_only_half_of_it() {
    let (_, light, _) = bar_colors("240,240,240", "102,102,102");
    assert_eq!(light, Rgb(0x78_7878));
}

#[test]
fn the_thumb_is_the_skins_button_eight_by_eighteen_from_the_lists_top() {
    let player = r#"
<elements>
  <bitmap id="wasabi.button.top.left" file="$solid" w="4" h="4" color="0,0,0"/>
  <bitmap id="wasabi.button.top" file="$solid" w="25" h="4" color="0,0,0"/>
  <bitmap id="wasabi.button.top.right" file="$solid" w="4" h="4" color="0,0,0"/>
  <bitmap id="wasabi.button.left" file="$solid" w="4" h="25" color="0,0,0"/>
  <bitmap id="wasabi.button.middle" file="$solid" w="25" h="25" color="200,200,200"/>
  <bitmap id="wasabi.button.right" file="$solid" w="4" h="25" color="0,0,0"/>
  <bitmap id="wasabi.button.bottom.left" file="$solid" w="4" h="4" color="0,0,0"/>
  <bitmap id="wasabi.button.bottom" file="$solid" w="25" h="4" color="0,0,0"/>
  <bitmap id="wasabi.button.bottom.right" file="$solid" w="4" h="4" color="0,0,0"/>
</elements>
<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="120">
<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" x="10" y="10" w="180" h="100"/>
</layout></container>"#;
    let mut rt = super::runtime(player);
    rt.load();
    rt.ensure_layout();
    let windows = rt.draw_windows();
    let (_, cmds) = windows.first().expect("the main window");
    let placed = |id: &str| {
        cmds.iter()
            .rev()
            .find_map(|c| match c {
                DrawCmd::Image { bitmap, dst, .. } if &*bitmap.id == id => Some(*dst),
                _ => None,
            })
            .expect("a part of the thumb")
    };
    // the list's right edge is at 190: 15 px of scrollbar, the thumb 5 px into it
    let top_left = placed("wasabi.button.top.left");
    assert_eq!((top_left.left, top_left.top), (180, 10));
    let bottom_right = placed("wasabi.button.bottom.right");
    assert_eq!((bottom_right.right(), bottom_right.bottom()), (188, 28));
}
