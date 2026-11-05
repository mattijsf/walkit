// SPDX-License-Identifier: GPL-3.0-or-later

//! What a skin keeps between runs moves in and out as one value: `restore_settings` before the
//! skin loads, `saved_settings` whenever the host saves (the reference's `SavedSettingsTest`).

use super::runtime;
use crate::host::SavedSettings;
use crate::maki::{Handle, Value};

const PLAYER_XML: &str = r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="40"/></container>"#;

fn saved() -> SavedSettings {
    SavedSettings {
        attributes: [("Animate Config Drawer".to_string(), "0".to_string())].into(),
        strings: [("bento/component".to_string(), "Video".to_string())].into(),
        ints: [("bento/width".to_string(), 412)].into(),
    }
}

#[test]
fn a_new_skin_keeps_nothing() {
    let rt = runtime(PLAYER_XML);
    assert_eq!(rt.saved_settings(), SavedSettings::default());
}

#[test]
fn what_is_put_back_before_load_comes_out_again_and_counts_as_no_change() {
    let mut rt = runtime(PLAYER_XML);
    rt.restore_settings(&saved());
    rt.load();
    assert_eq!(rt.saved_settings(), saved());
    assert_eq!(rt.settings_version(), 0);
}

#[test]
fn a_stored_attribute_wins_over_the_default_a_script_makes_it_with() {
    let mut rt = runtime(PLAYER_XML);
    rt.restore_settings(&saved());
    rt.load();
    let item = rt
        .config_item("Options", Some("{0F7A6C2E-0000-4C8F-9C2A-000000000001}"))
        .expect("the item is made");
    let made = rt.maki_call(
        Handle::Made(item),
        "newattribute",
        &[Value::str("Animate Config Drawer"), Value::str("1")],
    );
    let Some(Value::Obj(Handle::Made(a))) = made else {
        panic!("newAttribute gives the attribute, not {made:?}");
    };
    assert_eq!(rt.attribute_data(a), "0");
}

#[test]
fn a_saved_value_is_a_copy_that_later_changes_leave_as_it_was() {
    let mut rt = runtime(PLAYER_XML);
    rt.restore_settings(&saved());
    rt.load();
    let before = rt.saved_settings();
    rt.store_private_string("bento/component", "Playlist");

    assert_eq!(before.strings["bento/component"], "Video");
    assert_eq!(rt.saved_settings().strings["bento/component"], "Playlist");
    assert!(rt.settings_version() > 0, "a script's change counts");
}
