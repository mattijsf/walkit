// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawer that shows Winamp's visualizer, as Winamp Modern's does: where the host draws its
//! visualizer, and the window the host shows for the skin going back into it (Andamp's
//! `VisDrawerTest`).

use super::runtime_with;
use crate::draw::Rect;
use crate::host::SkinSetup;
use crate::maki::{Handle, Value};
use crate::runtime::SkinRuntime;

// a main window 200 by 100 whose drawer group, 40 px high, shows 30 px of a visualizer holder
// under a 10 px button bar
const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100">"#,
    r#"<layer id="grip" x="190" y="90" w="10" h="10" resize="right"/>"#,
    r#"<group id="drawer" x="10" y="60" w="180" h="40">"#,
    r#"<windowholder id="vis" x="0" y="10" w="180" h="60" hold="guid:avs" visible="0"/>"#,
    r#"</group></layout></container>"#,
    r#"<container id="avs" name="Visualizations" default_visible="0" "#,
    r#"component="guid:{0000000A-000C-0010-FF7B-01014263450C}"><layout id="normal" w="200" h="100">"#,
    r#"<button id="button.vis.reattach" action="EQ_TOGGLE" x="0" y="0" w="20" h="20"/>"#,
    r#"</layout></container>"#,
);

const AVS: &str = "{0000000A-000C-0010-FF7B-01014263450C}";

fn loaded() -> SkinRuntime {
    let mut rt = runtime_with(
        PLAYER_XML,
        SkinSetup {
            host_actions: true,
            ..SkinSetup::default()
        },
    );
    rt.set_view_size(Some((200, 1000)));
    rt.load();
    rt
}

fn object(rt: &SkinRuntime, id: &str) -> usize {
    let main = rt.main_layout().expect("a main window");
    rt.find_object(main, id)
        .unwrap_or_else(|| panic!("an object {id}"))
}

fn show_vis(rt: &mut SkinRuntime, shown: bool) {
    let vis = object(rt, "vis");
    rt.maki_call(Handle::Gui(vis), if shown { "show" } else { "hide" }, &[]);
}

#[test]
fn the_visualizer_shows_where_its_holder_shows_cut_to_the_drawer_around_it() {
    let mut rt = loaded();
    assert_eq!(rt.vis_rect(), None, "hidden");

    show_vis(&mut rt, true);
    assert_eq!(rt.vis_rect(), Some(Rect::new(10, 70, 180, 30)));

    show_vis(&mut rt, false);
    assert_eq!(rt.vis_rect(), None);
}

#[test]
fn a_holder_for_the_video_which_the_host_does_not_play_shows_the_visualizer_too() {
    let mut rt = loaded();
    let holder = object(&rt, "vis");
    for (k, v) in &mut rt.objects[holder].base.attrs {
        if k == "hold" {
            *v = "guid:{F0816D7B-FFFC-4343-80F2-E8199AA15CC3}".to_string();
        }
    }
    show_vis(&mut rt, true);

    assert_eq!(rt.vis_rect(), Some(Rect::new(10, 70, 180, 30)));
}

#[test]
fn a_drawer_shut_so_far_that_none_of_its_holder_shows_shows_no_visualizer() {
    let mut rt = loaded();
    show_vis(&mut rt, true);
    let drawer = object(&rt, "drawer");
    rt.maki_call(
        Handle::Gui(drawer),
        "resize",
        &[
            Value::Int(10),
            Value::Int(60),
            Value::Int(180),
            Value::Int(10),
        ],
    );

    assert_eq!(rt.vis_rect(), None);
}

#[test]
fn the_visualizer_window_closing_presses_the_reattach_button_of_the_window_a_script_had_the_host_show_once(
) {
    let mut rt = loaded();
    rt.visualizer_window_closed();
    assert!(!rt.player().eq_enabled(), "nothing shown, nothing pressed");

    rt.show_host_window(AVS);
    rt.visualizer_window_closed();
    rt.visualizer_window_closed();

    assert!(rt.player().eq_enabled());
    assert_eq!(rt.windows().len(), 1, "the skin's window stays shut");
}
