// SPDX-License-Identifier: GPL-3.0-or-later

//! How a phone sets out a skin's windows as one stack: each window docked under what shows of
//! the main one at its natural height, the last list window filling the height upright, list
//! windows in a second column beside the main one when the host asks for two columns, and a
//! touch beside the stack meaning the nearest window (Andamp's `StackArrangeTest`).

use super::runtime;
use crate::draw::Rect;
use crate::gui::ObjId;
use crate::runtime::SkinRuntime;

// a main window 300x200 whose shaped body shows 200x120 of it, an equalizer and a playlist,
// both of which can be made taller, and a window held at 340 px wide
const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="200">"#,
    r#"<group id="body" x="0" y="0" w="200" h="120" sysregion="1" rectrgn="1"/>"#,
    r#"</layout></container>"#,
    r#"<container id="eq" name="Equalizer" default_visible="0"><layout id="normal" w="300" h="60">"#,
    r#"<layer id="eq.sizer" resize="bottomright" x="290" y="50" w="10" h="10"/>"#,
    r#"</layout></container>"#,
    r#"<container id="pl" name="Playlist" default_visible="0"><layout id="normal" w="300" h="80">"#,
    r#"<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" "#,
    r#"x="0" y="0" w="290" h="70"/>"#,
    r#"<layer id="pl.sizer" resize="bottomright" x="290" y="70" w="10" h="10"/>"#,
    r#"</layout></container>"#,
    r#"<container id="wide" name="Wide" default_visible="0">"#,
    r#"<layout id="normal" w="340" h="40" minimum_w="340" maximum_w="340"/></container>"#,
);

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt
}

fn open(rt: &mut SkinRuntime, ids: &[&str]) {
    for id in ids {
        assert!(rt.open_window(id), "{id} opens");
    }
}

fn window(rt: &SkinRuntime, id: &str) -> ObjId {
    let c = rt.container(id).expect("the container");
    rt.containers[c].current.expect("a layout")
}

fn rect_of(rt: &mut SkinRuntime, id: &str) -> Rect {
    rt.ensure_layout();
    let l = window(rt, id);
    rt.window_rect_of(l)
}

fn view(rt: &mut SkinRuntime, w: i32, h: i32) {
    rt.set_view_size(Some((w, h)));
}

#[test]
fn one_column_docks_each_window_under_what_shows_of_the_main_window_as_wide_as_it() {
    let mut rt = loaded();
    view(&mut rt, 200, 1000);
    rt.set_stretch_last(false);
    open(&mut rt, &["eq", "pl"]);

    assert_eq!(rect_of(&mut rt, "main"), Rect::new(0, 0, 300, 200));
    assert_eq!(
        rect_of(&mut rt, "eq"),
        Rect::new(0, 120, 200, 60),
        "under the body, as wide as it"
    );
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 200, 80));
    assert_eq!(rt.stack_size(), (300, 260));
    assert_eq!(rt.shown_width(0), 200);
    assert!(rt.list_window_open());
}

#[test]
fn upright_the_last_list_window_takes_the_height_left() {
    let mut rt = loaded();
    view(&mut rt, 200, 1000);
    open(&mut rt, &["eq", "pl"]);

    assert_eq!(rect_of(&mut rt, "eq"), Rect::new(0, 120, 200, 60));
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(0, 180, 200, 820),
        "the playlist fills the view"
    );

    // the equalizer last shows no list, so it keeps its height, and so does the playlist above it
    assert!(rt.close_window("pl"));
    open(&mut rt, &["pl"]);
    assert!(rt.close_window("eq"));
    open(&mut rt, &["eq"]);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 120, 200, 80));
    assert_eq!(rect_of(&mut rt, "eq"), Rect::new(0, 200, 200, 60));
}

#[test]
fn a_docked_windows_natural_height_is_the_one_the_skin_gives_it_also_while_it_is_stretched() {
    let mut rt = loaded();
    view(&mut rt, 200, 1000);
    open(&mut rt, &["eq", "pl"]);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 200, 820));

    let pl = window(&rt, "pl");
    let eq = window(&rt, "eq");
    assert_eq!(rt.natural_height(pl), 80);
    assert_eq!(rt.natural_height(eq), 60);
    assert_eq!(
        rt.layout_size(pl).1,
        820,
        "asking leaves the window as it was"
    );
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 200, 820));
}

#[test]
fn the_playlist_shows_a_list_the_main_window_and_the_equalizer_do_not() {
    let mut rt = loaded();
    open(&mut rt, &["eq", "pl"]);

    let shows: Vec<bool> = rt.windows().iter().map(|l| rt.shows_list(*l)).collect();
    assert_eq!(shows, [false, false, true]);
}

#[test]
fn without_a_view_every_window_keeps_its_size() {
    let mut rt = loaded();
    open(&mut rt, &["eq", "pl"]);

    assert_eq!(rect_of(&mut rt, "main"), Rect::new(0, 0, 300, 200));
    assert_eq!(rect_of(&mut rt, "eq"), Rect::new(0, 120, 300, 60));
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 300, 80));
}

#[test]
fn a_touch_just_below_the_stack_finds_the_nearest_window_within_the_slop() {
    let mut rt = loaded();
    view(&mut rt, 200, 1000);
    rt.set_stretch_last(false);
    open(&mut rt, &["eq", "pl"]);
    rt.ensure_layout();
    let slop = 6;
    let (main, eq, pl) = (window(&rt, "main"), window(&rt, "eq"), window(&rt, "pl"));

    assert_eq!(
        rt.window_at(10, 259 + slop),
        Some(pl),
        "the playlist ends at 260"
    );
    assert_eq!(rt.window_at(10, 260 + slop), None, "past the slop");
    assert_eq!(
        rt.window_at(10, 150),
        Some(eq),
        "the equalizer covers the main window's empty space under the body"
    );
    assert_eq!(
        rt.window_at(250, 150),
        Some(main),
        "beside the equalizer the main window"
    );
}

#[test]
fn sideways_with_two_columns_the_list_windows_sit_right_of_the_main_window_and_the_last_fills_the_height(
) {
    let mut rt = loaded();
    view(&mut rt, 600, 400);
    rt.set_columns(2);
    open(&mut rt, &["eq", "pl"]);

    assert_eq!(rect_of(&mut rt, "main"), Rect::new(0, 0, 300, 200));
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(300, 0, 200, 400),
        "from the top at the main window's full width, as wide as what shows of it, as tall as the view"
    );
    assert_eq!(rt.stack_size(), (500, 400));
}

#[test]
fn the_equalizer_stays_under_the_player_in_the_left_column() {
    let mut rt = loaded();
    view(&mut rt, 600, 400);
    rt.set_columns(2);
    open(&mut rt, &["pl", "eq"]);

    assert_eq!(
        rect_of(&mut rt, "eq"),
        Rect::new(0, 120, 200, 60),
        "under the body, at its height though it is the last window"
    );
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(300, 0, 200, 400));
    assert_eq!(
        rt.left_column(),
        Some(Rect::new(0, 0, 300, 200)),
        "the left column reaches to the right column, and down to the equalizer or the main window"
    );
}

#[test]
fn the_right_column_starts_past_a_window_in_the_left_column_that_is_wider_than_the_main_one() {
    let mut rt = loaded();
    view(&mut rt, 800, 400);
    rt.set_columns(2);
    open(&mut rt, &["wide", "pl"]);

    assert_eq!(
        rect_of(&mut rt, "wide"),
        Rect::new(0, 120, 340, 40),
        "held at its minimum width, wider than the main window"
    );
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(340, 0, 200, 400),
        "the playlist from where the wide window ends"
    );
    assert_eq!(rt.stack_size(), (540, 400));
    assert_eq!(
        rt.left_column(),
        Some(Rect::new(0, 0, 340, 200)),
        "the left column reaches to where the right one starts"
    );
    assert_eq!(rt.natural_stack_size(2, 0), Some((540, 200)));
    assert_eq!(
        rt.shown_width(0),
        340,
        "the wide window makes the stack wider"
    );
}

#[test]
fn the_right_column_keeps_the_width_it_is_given_while_what_shows_of_the_main_window_changes() {
    let mut rt = loaded();
    view(&mut rt, 600, 400);
    rt.set_columns(2);
    rt.set_column_width(200);
    open(&mut rt, &["eq", "pl"]);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(300, 0, 200, 400));

    // the body widens, as MMD3's equalizer drawer slides out of its main window
    let main = window(&rt, "main");
    let body = rt.find_object(main, "body").expect("the body");
    rt.set_xml_param(body, "w", "260");
    assert_eq!(
        rect_of(&mut rt, "eq"),
        Rect::new(0, 120, 260, 60),
        "the equalizer under the main window follows it"
    );
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(300, 0, 200, 400),
        "the playlist keeps its width"
    );
    assert_eq!(rt.stack_size(), (500, 400), "so the stack keeps its size");
    assert_eq!(rt.natural_stack_size(2, 200), Some((500, 200)));
    let id = rt.window_id_of(main);
    assert_eq!(rt.content_right_of(id, false), 260);
}

#[test]
fn with_no_list_window_open_two_columns_are_one() {
    let mut rt = loaded();
    view(&mut rt, 600, 400);
    open(&mut rt, &["eq"]);
    let one = rect_of(&mut rt, "eq");

    rt.set_columns(2);
    assert_eq!(rect_of(&mut rt, "eq"), one);
    assert_eq!(one, Rect::new(0, 120, 200, 60));
    assert_eq!(rt.left_column(), None, "no left column to dock against");
    assert!(!rt.list_window_open());
}

#[test]
fn a_stacks_natural_size_in_one_column_and_in_two_leaves_every_window_as_it_is() {
    let mut rt = loaded();
    view(&mut rt, 200, 1000);
    open(&mut rt, &["eq", "pl"]);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 200, 820));
    let passes = rt.layout_passes;

    assert_eq!(
        rt.natural_stack_size(1, 0),
        Some((300, 260)),
        "one column: the playlist at its natural height"
    );
    assert_eq!(
        rt.natural_stack_size(2, 0),
        Some((500, 200)),
        "two columns: as tall as the main window"
    );
    let pl = window(&rt, "pl");
    assert_eq!(rt.layout_size(pl), (200, 820), "no window changed");
    assert_eq!(rt.layout_passes, passes, "no layout pass ran");
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 180, 200, 820));
}

#[test]
fn a_touch_beside_a_right_column_window_finds_it_by_distance_both_ways() {
    let mut rt = loaded();
    view(&mut rt, 600, 400);
    rt.set_columns(2);
    open(&mut rt, &["eq", "pl"]);
    rt.ensure_layout();
    let slop = 6;
    let (main, pl) = (window(&rt, "main"), window(&rt, "pl"));

    assert_eq!(
        rt.window_at(300 - slop, 300),
        Some(pl),
        "just left of the playlist"
    );
    assert_eq!(
        rt.window_at(100, 300),
        None,
        "in the room under the left column, far from both columns"
    );
    assert_eq!(
        rt.window_at(100, 199 + slop),
        Some(main),
        "just under the main window, whose room under its body the equalizer covers"
    );
    assert_eq!(
        rt.window_at(300 - slop, 400 + slop - 1),
        None,
        "a slop away both ways is further than the slop"
    );
}
