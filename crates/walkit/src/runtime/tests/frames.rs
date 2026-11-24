// SPDX-License-Identifier: GPL-3.0-or-later

//! A frame's divider dragged between its panes.

use super::runtime;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = r#"
<groupdef id="pane.a"><button id="a.btn" x="0" y="0" w="10" h="10"/></groupdef>
<groupdef id="pane.b"><button id="b.btn" x="0" y="0" w="10" h="10"/></groupdef>
<groupdef id="pane.c"><button id="c.btn" x="0" y="0" w="10" h="10"/></groupdef>
<groupdef id="pane.d"><button id="d.btn" x="0" y="0" w="10" h="10"/></groupdef>
<groupdef id="pane.e"><button id="e.btn" x="0" y="0" w="10" h="10"/></groupdef>
<groupdef id="pane.f"><button id="f.btn" x="0" y="0" w="10" h="10"/></groupdef>
<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="340">
<Wasabi:Frame id="side" x="0" y="0" w="300" h="100" orientation="v" from="right" width="100"
  minwidth="40" maxwidth="-60" left="pane.a" right="pane.b"/>
<Wasabi:Frame id="stack" x="0" y="110" w="100" h="100" orientation="h" from="bottom" height="30"
  minheight="20" top="pane.c" bottom="pane.d"/>
<Wasabi:Frame id="fixed" x="0" y="220" w="300" h="100" orientation="v" from="left" width="100"
  resizable="0" left="pane.e" right="pane.f"/>
</layout></container>"#;

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt.ensure_layout();
    rt
}

/// Where the pane that holds button `id` starts, across and down.
fn pane(rt: &mut SkinRuntime, id: &str) -> (i32, i32) {
    rt.ensure_layout();
    let rect = rt.object_info(id).expect("the pane's button").rect;
    (rect.left, rect.top)
}

#[test]
fn a_divider_follows_the_pointer_between_the_frames_limits() {
    let mut rt = loaded();
    // 100 from the right of 300: the divider's 8 pixels end where the right pane starts
    assert_eq!(pane(&mut rt, "b.btn"), (200, 0));
    // taken 5 pixels into the divider and dragged 40 to the left: the pane is 40 wider
    rt.pointer_down(195, 50, 1);
    rt.pointer_move(155, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (160, 0));
    // no wider than the frame less 60, no narrower than 40
    rt.pointer_move(10, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (60, 0));
    rt.pointer_move(298, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (260, 0));
    rt.pointer_move(205, 50);
    rt.pointer_up(205, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (210, 0));
    // let go, it stays
    rt.pointer_move(100, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (210, 0));
    // the left pane never moved
    assert_eq!(pane(&mut rt, "a.btn"), (0, 0));
}

#[test]
fn a_stacked_frames_divider_goes_up_and_down() {
    let mut rt = loaded();
    // 30 from the bottom of 100: the top pane is 70 high, the divider under it
    assert_eq!(pane(&mut rt, "d.btn"), (0, 188));
    rt.pointer_down(20, 184, 1);
    rt.pointer_move(20, 164);
    assert_eq!(pane(&mut rt, "d.btn"), (0, 168));
    // the bottom pane is 20 high at least
    rt.pointer_move(20, 330);
    rt.pointer_up(20, 330);
    assert_eq!(pane(&mut rt, "d.btn"), (0, 198));
}

#[test]
fn a_press_beside_a_divider_or_on_a_fixed_frame_moves_no_pane() {
    let mut rt = loaded();
    // in a pane, off the divider
    rt.pointer_down(100, 50, 1);
    rt.pointer_move(60, 50);
    rt.pointer_up(60, 50);
    assert_eq!(pane(&mut rt, "b.btn"), (200, 0));
    // a frame that is not resizable has no divider: its panes meet at its position
    assert_eq!(pane(&mut rt, "f.btn"), (100, 220));
    rt.pointer_down(100, 270, 2);
    rt.pointer_move(150, 270);
    rt.pointer_up(150, 270);
    assert_eq!(pane(&mut rt, "f.btn"), (100, 220));
}
