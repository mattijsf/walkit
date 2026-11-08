// SPDX-License-Identifier: GPL-3.0-or-later

//! A modern skin's playlist buttons (Winamp Modern's Add, Rem, Sel, Misc and Manage Playlist),
//! whose menus the app shows above the button.

use std::cell::RefCell;
use std::rc::Rc;

use super::{Froms, RecordingHost, SKIN_XML};
use crate::draw::Rect;
use crate::files::MemoryFiles;
use crate::host::{ActionFrom, Clock, SkinSetup};
use crate::player::FakePlayer;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="40">"#,
    r#"<button id="pl.add" action="PE_Add" x="3" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.rem" action="PE_Rem" x="46" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.sel" action="PE_Sel" x="90" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.misc" action="PE_Misc" x="127" y="4" w="30" h="12"/>"#,
    r#"<button id="pl.list" action="PE_List" x="160" y="4" w="30" h="12"/>"#,
    r#"</layout></container>"#,
);

fn recording(host_actions: bool) -> (SkinRuntime, Rc<RefCell<Vec<String>>>, Froms) {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let froms: Froms = Rc::default();
    let mut files = MemoryFiles::new();
    files.insert("skin.xml", SKIN_XML.as_bytes());
    files.insert("player.xml", PLAYER_XML.as_bytes());
    let clock: Clock = Rc::new(|| 0);
    let mut rt = SkinRuntime::new(
        Box::new(files),
        Box::new(RecordingHost(actions.clone(), froms.clone())),
        Box::new(FakePlayer::new(clock.clone())),
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
