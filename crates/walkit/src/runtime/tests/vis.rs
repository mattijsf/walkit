// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawer that shows Winamp's visualizer, as Winamp Modern's does: where the host draws its
//! visualizer, and the window the host shows for the skin going back into it (Andamp's
//! `VisDrawerTest`).

use super::runtime_with;
use crate::draw::{DrawCmd, Rect};
use crate::host::SkinSetup;
use crate::maki::{Handle, Value};
use crate::runtime::SkinRuntime;
use std::cell::RefCell;
use std::rc::Rc;

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

#[test]
fn a_holder_that_takes_the_visualizer_as_it_shows_closes_the_visualizations_window() {
    let mut rt = loaded();
    let holder = object(&rt, "vis");
    rt.objects[holder]
        .base
        .attrs
        .push(("autoopen".to_string(), "1".to_string()));
    assert!(rt.open_window("avs"));
    rt.ensure_layout();
    assert_eq!(rt.windows().len(), 2);

    show_vis(&mut rt, true);
    rt.ensure_layout();
    assert_eq!(rt.windows().len(), 1, "the visualizer went into the drawer");

    // opened again while the drawer still shows it, the window stays
    assert!(rt.open_window("avs"));
    rt.ensure_layout();
    assert_eq!(rt.windows().len(), 2);
}

/// A host whose visualizer picks presets at random or not, and that keeps what it was asked.
struct RandomHost(Rc<RefCell<(bool, Vec<String>)>>);

impl crate::host::SkinHost for RandomHost {
    fn image_size(&mut self, _file: &str) -> Option<(i32, i32)> {
        None
    }

    fn text_width(
        &mut self,
        text: &str,
        _font: Option<&crate::draw::FontDef>,
        size: i32,
        _bold: bool,
    ) -> i32 {
        text.chars().count() as i32 * size / 2
    }

    fn log(&mut self, _message: &str) {}

    fn invalidate(&mut self) {}

    fn window_open(&mut self, kind: &str) -> Option<bool> {
        (kind == "visrandom").then(|| self.0.borrow().0)
    }

    fn host_action(
        &mut self,
        action: &str,
        param: Option<&str>,
        _from: Option<crate::host::ActionFrom>,
    ) {
        self.0
            .borrow_mut()
            .1
            .push(format!("{action} {}", param.unwrap_or("")));
    }
}

#[test]
fn the_visualizers_random_button_is_the_hosts() {
    let state = Rc::new(RefCell::new((true, Vec::new())));
    let mut files = crate::files::MemoryFiles::new();
    files.insert(
        "skin.xml",
        &br#"<WinampAbstractionLayer version="0.8"><include file="player.xml"/></WinampAbstractionLayer>"#[..],
    );
    files.insert(
        "player.xml",
        concat!(
            r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="50">"#,
            r#"<togglebutton id="random" x="0" y="0" w="20" h="20" "#,
            r#"cfgattrib="{0000000A-000C-0010-FF7B-01014263450C};Random"/>"#,
            r#"</layout></container>"#,
        )
        .as_bytes(),
    );
    let clock: crate::host::Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(RandomHost(state.clone())),
        Box::new(crate::player::FakePlayer::new(clock.clone())),
        SkinSetup {
            host_actions: true,
            ..SkinSetup::default()
        },
        clock,
    );
    rt.load();
    rt.ensure_layout();
    let button = object(&rt, "random");
    assert!(rt.button_is_active(button), "on, as the host says");
    rt.pointer_down(5, 5, 1);
    rt.pointer_up(5, 5);
    assert_eq!(state.borrow().1, vec!["VIS_RANDOM 0".to_string()]);
}

#[test]
fn an_eq_curve_draws_winamps_rows_with_a_boosted_preamp_along_the_bottom() {
    // a 67x13 curve, white, its preamp line red; the setting and every column's row as
    // Winamp 5.666 drew them in the VM
    let mut rt = super::runtime(concat!(
        r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="80" h="20">"#,
        r#"<eqvis id="curve" x="5" y="3" w="67" h="13" colortop="255,255,255" "#,
        r#"colormiddle="255,255,255" colorbottom="255,255,255" colorpreamp="255,0,0"/>"#,
        r#"</layout></container>"#,
    ));
    rt.load();
    rt.ensure_layout();
    // the player's 0..255 (255 the top) for a slider step 0..63 (0 the top)
    let level = |step: i32| (63 - step) * 255 / 63;
    rt.player.set_eq_preamp(level(4));
    for (band, step) in [27, 3, 32, 27, 37, 30, 41, 33, 53, 16].iter().enumerate() {
        rt.player.set_eq_band(band as i32, level(*step));
    }
    let px = rt
        .draw_main()
        .iter()
        .find_map(|c| match c {
            DrawCmd::Pixels { rect, argb, .. } if rect.width == 67 => Some(argb.to_vec()),
            _ => None,
        })
        .expect("the curve's pixels");
    let (white, red) = (0xffff_ffff, 0xffff_0000);
    // the preamp 4 steps under the top lies on row 11 of 13: (63 - 4) * 13 / 64
    assert!((0..67).all(|x| [red, white].contains(&px[11 * 67 + x])));
    assert!((0..67).any(|x| px[11 * 67 + x] == red));
    // the curve's first columns step down 5, 5, 4, 3, 2, 1, 1, 0, 0 rows from the top
    let first: Vec<usize> = (0..9)
        .map(|x| (0..13).rev().find(|r| px[r * 67 + x] == white).unwrap())
        .collect();
    assert_eq!(first, [5, 5, 5, 4, 3, 2, 1, 1, 0]);
}
