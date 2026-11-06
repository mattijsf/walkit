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
    r#"</layout></container>"#,
    r#"<container id="pl" name="Playlist" default_visible="0"><layout id="normal" w="300" h="100">"#,
    r#"<layer id="pl.sizer" resize="bottomright" x="290" y="90" w="10" h="10"/>"#,
    r#"</layout></container>"#,
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
