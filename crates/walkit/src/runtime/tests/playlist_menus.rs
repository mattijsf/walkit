// SPDX-License-Identifier: GPL-3.0-or-later

//! What the app's playlist menus need from a modern skin: its playlist buttons (Winamp Modern's
//! Add, Rem, Sel, Misc and Manage Playlist) asking for their menus, and the entry the list has
//! selected for their items to act on.

use std::cell::RefCell;
use std::rc::Rc;

use super::{Froms, RecordingHost, SKIN_XML};
use crate::draw::Rect;
use crate::files::MemoryFiles;
use crate::host::{ActionFrom, Clock, SkinSetup};
use crate::player::{FakePlayer, Track};
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="120">"#,
    r#"<button id="pl.add" action="PE_Add" x="3" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.rem" action="PE_Rem" x="46" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.sel" action="PE_Sel" x="90" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.misc" action="PE_Misc" x="127" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.list" action="PE_List" x="160" y="4" w="30" h="12"/>"#,
    r#"<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" "#,
    r#"x="0" y="20" w="200" h="100"/>"#,
    r#"</layout></container>"#,
);

fn recording(host_actions: bool) -> (SkinRuntime, Rc<RefCell<Vec<String>>>, Froms) {
    recording_with(PLAYER_XML, host_actions)
}

fn recording_with(
    player_xml: &str,
    host_actions: bool,
) -> (SkinRuntime, Rc<RefCell<Vec<String>>>, Froms) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let froms: Froms = Rc::default();
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", SKIN_XML.as_bytes());
    files.insert("player.xml", player_xml.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    let mut player = FakePlayer::new(clock.clone());
    player.playlist = ["One", "Two", "Three"]
        .map(|title| Track {
            title: title.into(),
            length_ms: 60_000,
        })
        .into();
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(RecordingHost(actions.clone(), froms.clone())),
        Box::new(player),
        SkinSetup {
            host_actions,
            ..SkinSetup::default()
        },
        clock,
    );
    rt.load();
    rt.ensure_layout();
    (rt, actions, froms)
}

fn tap_each(rt: &mut SkinRuntime) {
    for (seq, x) in [10, 50, 100, 130, 170].into_iter().enumerate() {
        rt.pointer_down(x, 8, seq as i32 + 1);
        rt.pointer_up(x, 8);
    }
}

#[test]
fn the_playlist_buttons_ask_the_host_for_their_menus_from_their_box() {
    let (mut rt, actions, froms) = recording(true);
    tap_each(&mut rt);
    assert_eq!(
        *actions.borrow(),
        vec!["PE_ADD", "PE_REM", "PE_SEL", "PE_MISC", "PE_LIST"]
    );
    assert_eq!(
        froms.borrow()[1],
        Some(ActionFrom {
            rect: Rect::new(46, 4, 30, 12),
            point: Some((50, 8)),
        })
    );
}

#[test]
fn without_host_actions_the_playlist_buttons_send_nothing() {
    let (mut rt, actions, _) = recording(false);
    tap_each(&mut rt);
    assert!(actions.borrow().is_empty());
}

#[test]
fn the_list_tells_the_host_the_entry_it_selects_once() {
    let (mut rt, actions, _) = recording(true);
    // the list's rows start 3 px into it, 15 px each: the second is at y 38..53 of the window
    for seq in 1..=2 {
        rt.pointer_down(50, 45, seq);
        rt.pointer_up(50, 45);
    }
    assert_eq!(*actions.borrow(), vec!["PLAYLIST_SELECT 1"]);
}

#[test]
fn the_host_sets_the_selection_without_hearing_it_back() {
    let (mut rt, actions, _) = recording(true);
    rt.set_playlist_selection(Some(1));
    // the entry the host selected plays on the next tap, as one the list selected itself
    rt.pointer_down(50, 45, 1);
    rt.pointer_up(50, 45);
    assert!(actions.borrow().is_empty());
    // with none selected, a tap selects again and tells the host
    rt.set_playlist_selection(None);
    rt.pointer_down(50, 45, 2);
    rt.pointer_up(50, 45);
    assert_eq!(*actions.borrow(), vec!["PLAYLIST_SELECT 1"]);
}

/// A skin without playlist buttons (MMD3): Wasabi's status bar shows the playlist's
/// Add, Rem, Sel, Misc and List under the list, List over the frame's resize corner.
const STATUS_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="140">"#,
    r#"<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" "#,
    r#"x="0" y="0" w="300" h="110"/>"#,
    r#"<layer id="corner" x="200" y="100" w="100" h="40" resize="bottomright"/>"#,
    r#"<LayoutStatus id="status" x="10" y="120" w="280" h="17"/>"#,
    r#"</layout></container>"#,
);

#[test]
fn the_status_bar_buttons_ask_the_host_for_the_same_menus() {
    let (mut rt, actions, froms) = recording_with(STATUS_XML, true);
    // Add is the first button, List the last; between them the field takes no touch
    for (seq, x) in [13, 260, 220].into_iter().enumerate() {
        rt.pointer_down(x, 128, seq as i32 + 1);
        rt.pointer_up(x, 128);
    }
    assert_eq!(*actions.borrow(), vec!["PE_ADD", "PE_LIST"]);
    // the menu opens at the button, which fills the bar's height from its left end
    let add = froms.borrow()[0].unwrap().rect;
    assert_eq!((add.left, add.top, add.height), (10, 120, 17));
}

/// A skin with its own status bar field and old-style button pictures (MMD3): the field fills
/// the bar between Misc and List, and the picture defined under the old name is the one used.
const FIELD_XML: &str = concat!(
    r#"<elements>"#,
    r#"<elementalias id="studio.button.upperLeft" target="wasabi.button.top.left"/>"#,
    r#"<bitmap id="wasabi.button.top.left" file="b.png" x="0" y="0" w="4" h="4"/>"#,
    r#"<bitmap id="studio.button.upperLeft" file="b.png" x="1" y="1" w="2" h="2"/>"#,
    r#"</elements>"#,
    r#"<groupdef id="wasabi.statusbar">"#,
    r#"<layer id="field" x="0" y="0" w="0" h="0" relatw="1" relath="1"/>"#,
    r#"</groupdef>"#,
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="140">"#,
    r#"<component id="list" param="guid:{45F3F7C1-A6F3-4ee6-A15E-125E92FC3F8D}" "#,
    r#"x="0" y="0" w="300" h="110"/>"#,
    r#"<LayoutStatus id="status" x="10" y="120" w="280" h="17"/>"#,
    r#"</layout></container>"#,
);

#[test]
fn the_status_bar_field_lies_between_the_buttons_and_old_button_names_win() {
    let (mut rt, actions, froms) = recording_with(FIELD_XML, true);
    // Misc is the last button on the left, List the one on the right
    let (misc_x, list_x) = (
        (10..290)
            .rev()
            .find(|&x| rt.status_button_at_for_test(x, 128) == Some("Misc")),
        (10..290).find(|&x| rt.status_button_at_for_test(x, 128) == Some("List")),
    );
    let (misc_x, list_x) = (misc_x.unwrap(), list_x.unwrap());
    for (seq, x) in [misc_x, list_x].into_iter().enumerate() {
        rt.pointer_down(x, 128, seq as i32 + 1);
        rt.pointer_up(x, 128);
    }
    assert_eq!(*actions.borrow(), vec!["PE_MISC", "PE_LIST"]);
    let misc = froms.borrow()[0].unwrap().rect;
    let list = froms.borrow()[1].unwrap().rect;
    let field = rt
        .objects
        .iter()
        .find(|o| o.base.id == "field")
        .expect("the field group's layer")
        .base
        .rect;
    assert_eq!(
        (field.left, field.right(), field.top, field.height),
        (misc.right(), list.left, 120, 17)
    );
    assert_eq!(rt.bitmap_size("wasabi.button.top.left"), Some((2, 2)));
}

#[test]
fn a_press_a_magnifier_steers_onto_another_button_clicks_that_one() {
    let (mut rt, actions, _) = recording(true);
    rt.pointer_down(10, 8, 1);
    rt.set_pointer_precise(true);
    rt.pointer_move(30, 8);
    rt.pointer_move(55, 8);
    rt.pointer_up(55, 8);
    assert_eq!(*actions.borrow(), vec!["PE_REM"]);
    // let go where it began, the press clicks its own button
    rt.pointer_down(10, 8, 2);
    rt.set_pointer_precise(true);
    rt.pointer_move(12, 9);
    rt.pointer_up(12, 9);
    assert_eq!(*actions.borrow(), vec!["PE_REM", "PE_ADD"]);
    // between two buttons, on no control, a steered release clicks nothing
    rt.pointer_down(10, 8, 3);
    rt.set_pointer_precise(true);
    rt.pointer_move(40, 8);
    rt.pointer_up(40, 8);
    assert_eq!(actions.borrow().len(), 2, "{:?}", actions.borrow());
}
