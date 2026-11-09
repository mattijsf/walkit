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

/// A skin without playlist buttons of its own (MMD3): Wasabi's status bar shows the playlist's
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
    // the menu opens at the button: 1 px into the bar, as tall as the bar less 2
    let add = froms.borrow()[0].unwrap().rect;
    assert_eq!((add.left, add.top, add.height), (11, 121, 15));
}
