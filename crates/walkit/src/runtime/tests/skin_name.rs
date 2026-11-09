// SPDX-License-Identifier: GPL-3.0-or-later

//! A skin names itself in `<skininfo><name>`, which the host falls back on when it has no name
//! for it (Andamp's `SkinInfoNameTest`), and the host may name it at any time.

use super::runtime_files;
use crate::host::SkinSetup;

const SKIN_XML: &str = concat!(
    r#"<WinampAbstractionLayer version="0.8">"#,
    r#"<skininfo><version>1.0</version><name>Big Bento</name><author>Victor</author></skininfo>"#,
    r#"<include file="player.xml"/></WinampAbstractionLayer>"#,
);
const PLAYER_XML: &str = r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100"/></container>"#;

#[test]
fn a_skin_the_host_has_no_name_for_takes_the_one_in_skininfo() {
    let mut rt = runtime_files(SKIN_XML, PLAYER_XML, SkinSetup::default());
    rt.load();
    assert_eq!(rt.skin_name(), "Big Bento");
}

#[test]
fn a_name_the_host_gave_stays() {
    let mut rt = runtime_files(SKIN_XML, PLAYER_XML, SkinSetup::default());
    rt.set_skin_name("Bento");
    rt.load();
    assert_eq!(rt.skin_name(), "Bento");
}

#[test]
fn a_name_given_after_load_is_what_scripts_read_from_then_on() {
    let mut rt = runtime_files(SKIN_XML, PLAYER_XML, SkinSetup::default());
    rt.load();
    rt.set_skin_name("Other");
    assert_eq!(rt.skin_name(), "Other");
}
