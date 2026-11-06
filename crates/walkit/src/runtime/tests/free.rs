// SPDX-License-Identifier: GPL-3.0-or-later
//! The windows set out apart, moved by a drag, docking and snapping as Winamp 5.666 does: the
//! numbers were measured with Winamp Modern on Winamp 5.666 in a VM.

use super::runtime;
use crate::draw::Rect;
use crate::host::Presentation;
use crate::runtime::SkinRuntime;

// a main window 300x200 with a closed drawer under its body (a snap margin of 80 at the
// bottom, as Winamp Modern's main window keeps), a playlist 300x100 and an equalizer 300x60
const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1">"#,
    r#"<layout id="normal" w="300" h="200" snapadjustbottom="80">"#,
    r#"<group id="body" x="0" y="0" w="300" h="120" sysregion="1" rectrgn="1"/>"#,
    r#"</layout><layout id="shade" w="300" h="25"/></container>"#,
    r#"<container id="pl" name="Playlist" default_visible="0">"#,
    r#"<layout id="normal" w="300" h="100" linkwidth="shade">"#,
    r#"<layer id="pl.sizer" resize="bottomright" x="290" y="90" w="10" h="10"/>"#,
    r#"</layout><layout id="shade" w="200" h="25" linkwidth="normal"/></container>"#,
    r#"<container id="eq" name="Equalizer" default_visible="0"><layout id="normal" w="300" h="60"/></container>"#,
);

fn free() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt.set_presentation(&Presentation {
        viewport: (1000, 1000),
        free_windows: true,
        ..Presentation::default()
    });
    rt
}

fn rect_of(rt: &mut SkinRuntime, id: &str) -> Rect {
    rt.ensure_layout();
    let c = rt.container(id).expect("the container");
    let l = rt.containers[c].current.expect("a layout");
    rt.window_rect_of(l)
}

fn id_of(rt: &SkinRuntime, id: &str) -> crate::host::WindowId {
    let c = rt.container(id).expect("the container");
    let l = rt.containers[c].current.expect("a layout");
    rt.window_id_of(l)
}

/// Drags window `id` by its top-left area (a press on no object moves the window) so that its
/// top-left corner lands at `x`, `y`, in a few steps as a finger moves.
fn drag_to(rt: &mut SkinRuntime, id: &str, x: i32, y: i32) {
    let r = rect_of(rt, id);
    let (gx, gy) = (r.left + 150, r.top + 5);
    let (dx, dy) = (x - r.left, y - r.top);
    rt.pointer_down(gx, gy, 1);
    for i in 1..=4 {
        rt.pointer_move(gx + dx * i / 4, gy + dy * i / 4);
    }
    rt.pointer_up(gx + dx, gy + dy);
}

#[test]
fn windows_start_where_the_stack_puts_them_and_keep_their_sizes() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    assert_eq!(rect_of(&mut rt, "main"), Rect::new(0, 0, 300, 200));
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(0, 120, 300, 100),
        "under the body"
    );
    assert_eq!(rt.stack_size(), (1000, 1000), "the viewport is the stack");
}

#[test]
fn a_window_snaps_under_another_from_nine_pixels_but_not_from_ten() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "pl", 0, 400);
    assert_eq!(rect_of(&mut rt, "pl").top, 400);
    // the main window docks by its visible bottom, 120, not by its rect's bottom
    drag_to(&mut rt, "pl", 0, 130);
    assert_eq!(rect_of(&mut rt, "pl").top, 130, "ten away: no snap");
    drag_to(&mut rt, "pl", 0, 129);
    assert_eq!(rect_of(&mut rt, "pl").top, 120, "nine away: docked");
}

#[test]
fn edges_align_within_the_distance_only_while_touching() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "pl", 7, 120);
    assert_eq!(rect_of(&mut rt, "pl").left, 0, "left edges align");
    drag_to(&mut rt, "pl", 15, 120);
    assert_eq!(rect_of(&mut rt, "pl").left, 15, "past the distance");
    drag_to(&mut rt, "pl", 105, 400);
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(105, 400, 300, 100),
        "far below: no align"
    );
}

#[test]
fn a_window_docks_beside_another() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "pl", 306, 20);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(300, 20, 300, 100));
}

#[test]
fn the_main_window_carries_its_group_and_another_window_moves_alone() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    assert!(rt.open_window("eq"));
    // the equalizer opens under the playlist: a chain main - playlist - equalizer
    assert_eq!(rect_of(&mut rt, "eq"), Rect::new(0, 220, 300, 60));
    drag_to(&mut rt, "main", 50, 30);
    assert_eq!(rect_of(&mut rt, "main"), Rect::new(50, 30, 300, 200));
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(50, 150, 300, 100),
        "follows the main window"
    );
    assert_eq!(
        rect_of(&mut rt, "eq"),
        Rect::new(50, 250, 300, 60),
        "and so does the chain"
    );
    drag_to(&mut rt, "pl", 50, 190);
    assert_eq!(rect_of(&mut rt, "pl").top, 190, "moved alone");
    assert_eq!(rect_of(&mut rt, "eq").top, 250, "what hung under it stays");
    assert_eq!(rect_of(&mut rt, "main").top, 30);
}

#[test]
fn a_member_snapping_to_the_viewport_shifts_the_group() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "main", 100, 100);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(100, 220, 300, 100));
    // the playlist's bottom comes within the distance of the viewport's bottom first
    drag_to(&mut rt, "main", 100, 1000 - 220 - 5);
    assert_eq!(
        rect_of(&mut rt, "pl").bottom(),
        1000,
        "the playlist snapped to the bottom"
    );
    assert_eq!(
        rect_of(&mut rt, "main").top,
        1000 - 220,
        "and took the main window along"
    );
}

#[test]
fn snap_margins_set_by_a_script_move_the_docking_edge() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "pl", 0, 400);
    let pl = rt
        .container("pl")
        .and_then(|c| rt.containers[c].current)
        .expect("the playlist");
    rt.snap_adjust(pl, 0, 0, 0, 16);
    // the playlist's docking bottom is 16 above its rect's: 5 above the main window's top docks it
    drag_to(&mut rt, "main", 0, 400 + 100 - 16 + 5);
    assert_eq!(
        rect_of(&mut rt, "main").top,
        400 + 100 - 16,
        "docked under the playlist's margin"
    );
}

#[test]
fn redock_keeps_the_docked_windows_on_an_edge_the_script_moves() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    let main = rt.main_layout().expect("the main window");
    rt.before_redock(main);
    // the drawer opens: the main window's docking bottom goes from 120 to 200
    rt.snap_adjust(main, 0, 0, 0, 0);
    rt.redock(main);
    assert_eq!(
        rect_of(&mut rt, "pl").top,
        200,
        "the playlist followed the drawer"
    );
    rt.before_redock(main);
    rt.snap_adjust(main, 0, 0, 0, 80);
    rt.redock(main);
    assert_eq!(rect_of(&mut rt, "pl").top, 120, "and came back up");
}

#[test]
fn the_host_places_a_window_where_it_kept_it() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    let pl = id_of(&rt, "pl");
    rt.place_window(pl, 400, 500);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(400, 500, 300, 100));
    assert_eq!(rt.window_rect(pl), Rect::new(400, 500, 300, 100));
}

#[test]
fn as_one_stack_nothing_moves_by_a_drag() {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "pl", 0, 400);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 120, 300, 100));
}

fn z_order(rt: &mut SkinRuntime) -> Vec<&'static str> {
    rt.ensure_layout();
    let names = [("main", "main"), ("pl", "pl"), ("eq", "eq")];
    rt.z_windows()
        .into_iter()
        .map(|l| {
            names
                .iter()
                .find(|(id, _)| rt.container(id).and_then(|c| rt.containers[c].current) == Some(l))
                .map(|(_, n)| *n)
                .unwrap_or("?")
        })
        .collect()
}

#[test]
fn a_window_that_opens_comes_on_top_and_a_press_raises_a_window() {
    let mut rt = free();
    assert_eq!(z_order(&mut rt), vec!["main"]);
    assert!(rt.open_window("pl"));
    assert_eq!(z_order(&mut rt), vec!["main", "pl"], "opened: on top");
    assert!(rt.open_window("eq"));
    assert_eq!(z_order(&mut rt), vec!["main", "pl", "eq"]);
    // a press on the main window raises it with the chain docked to it, in its order
    rt.pointer_down(250, 50, 1);
    rt.pointer_up(250, 50);
    assert_eq!(z_order(&mut rt), vec!["pl", "eq", "main"]);
    // a press on a docked window raises that window alone
    drag_to(&mut rt, "pl", 0, 400);
    rt.pointer_down(150, 405, 2);
    rt.pointer_up(150, 405);
    assert_eq!(z_order(&mut rt), vec!["eq", "main", "pl"]);
    // the main window's group is what touches it: nothing now, the equalizer hung under the playlist
    rt.pointer_down(250, 50, 3);
    rt.pointer_up(250, 50);
    assert_eq!(z_order(&mut rt), vec!["eq", "pl", "main"]);
    // drawing follows the order
    let drawn = rt.draw_windows();
    assert_eq!(drawn.len(), 3);
    assert_eq!(
        drawn[2].0,
        rect_of(&mut rt, "main"),
        "the main window drawn last, on top"
    );
}

#[test]
fn a_press_on_the_desktop_between_the_windows_moves_nothing() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    rt.pointer_down(700, 700, 1);
    rt.pointer_move(600, 600);
    rt.pointer_up(600, 600);
    assert_eq!(rect_of(&mut rt, "main"), Rect::new(0, 0, 300, 200));
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 120, 300, 100));
    // nor a press on the main window's closed drawer area, which its shape leaves open
    rt.pointer_down(250, 180, 2);
    rt.pointer_move(250, 280);
    rt.pointer_up(250, 280);
    assert_eq!(rect_of(&mut rt, "main").top, 0);
}

#[test]
fn a_layout_switch_keeps_the_place_and_the_docked_windows_follow_the_new_edges() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    drag_to(&mut rt, "main", 50, 30);
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(50, 150, 300, 100));
    let c = rt.container("main").expect("the main container");
    rt.switch_to(c, "shade");
    assert_eq!(
        rect_of(&mut rt, "main"),
        Rect::new(50, 30, 300, 25),
        "the shade bar where the window was"
    );
    assert_eq!(
        rect_of(&mut rt, "pl").top,
        55,
        "the playlist under the shade bar"
    );
    rt.switch_to(c, "normal");
    assert_eq!(
        rect_of(&mut rt, "main"),
        Rect::new(50, 30, 300, 200),
        "back where it was"
    );
    assert_eq!(
        rect_of(&mut rt, "pl").top,
        150,
        "the playlist back under the window"
    );
    // the switched window keeps its place on top
    assert_eq!(z_order(&mut rt), vec!["pl", "main"]);
}

#[test]
fn layouts_that_link_their_widths_share_it_through_a_switch() {
    let mut rt = free();
    assert!(rt.open_window("pl"));
    let c = rt.container("pl").expect("the playlist container");
    rt.switch_to(c, "shade");
    assert_eq!(
        rect_of(&mut rt, "pl"),
        Rect::new(0, 120, 300, 25),
        "the bar as wide as the window was"
    );
    rt.switch_to(c, "normal");
    assert_eq!(rect_of(&mut rt, "pl"), Rect::new(0, 120, 300, 100));
}
