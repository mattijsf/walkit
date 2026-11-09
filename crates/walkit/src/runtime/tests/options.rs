// SPDX-License-Identifier: GPL-3.0-or-later

//! The options a skin's scripts list for Winamp's Options menu and a pick of one: read as
//! Winamp lists them, flipped as a click there flips it (Andamp's `SkinOptionsTest`).

use super::runtime;
use crate::host::{OptionEntry, OptionKey};
use crate::maki::{Handle, Value};
use crate::runtime::options::OPTIONS_MENU;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="200" h="100"/></container>"#;

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    rt
}

/// `Config.getItem(key)`: the item, made when no script made it yet.
fn get_item(rt: &mut SkinRuntime, key: &str) -> usize {
    rt.config_item(key, Some(key)).expect("an item")
}

/// `Config.newItem(name, guid)`.
fn new_item(rt: &mut SkinRuntime, name: &str, guid: &str) -> usize {
    rt.config_item(name, Some(guid)).expect("an item")
}

/// `item.newAttribute(name, value)`: the attribute.
fn add(rt: &mut SkinRuntime, item: usize, name: &str, value: &str) -> usize {
    match rt.maki_call(
        Handle::Made(item),
        "newattribute",
        &[Value::str(name), Value::str(value)],
    ) {
        Some(Value::Obj(Handle::Made(a))) => a,
        other => panic!("newAttribute gives the attribute, not {other:?}"),
    }
}

fn guid_of(rt: &SkinRuntime, item: usize) -> String {
    match &rt.made[item] {
        crate::runtime::maki::Made::ConfigItem { guid, .. } => guid.clone().expect("a guid"),
        _ => panic!("not an item"),
    }
}

/// The data of the attribute `name` (lowercase) of `item`.
fn data(rt: &SkinRuntime, item: usize, name: &str) -> String {
    let a = match &rt.made[item] {
        crate::runtime::maki::Made::ConfigItem { attributes, .. } => attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, a)| *a)
            .expect("the attribute"),
        _ => panic!("not an item"),
    };
    rt.attribute_data(a)
}

fn options(rt: &SkinRuntime) -> Vec<OptionEntry> {
    rt.skin_options().entries
}

fn text(e: &OptionEntry) -> String {
    match e {
        OptionEntry::Toggle { text, .. } => text.clone(),
        OptionEntry::Submenu { text, .. } => format!("{text} >"),
        OptionEntry::Separator => "-".to_string(),
    }
}

fn texts(entries: &[OptionEntry]) -> Vec<String> {
    entries.iter().map(text).collect()
}

fn checks(entries: &[OptionEntry]) -> Vec<bool> {
    entries
        .iter()
        .filter_map(|e| match e {
            OptionEntry::Toggle { checked, .. } => Some(*checked),
            _ => None,
        })
        .collect()
}

/// The one toggle reading `wanted`: its key, whether it is checked, and whether it is a radio button.
fn toggle(entries: &[OptionEntry], wanted: &str) -> (OptionKey, bool, bool) {
    let found: Vec<_> = entries
        .iter()
        .filter_map(|e| match e {
            OptionEntry::Toggle {
                key,
                text,
                checked,
                radio,
            } if text == wanted => Some((key.clone(), *checked, *radio)),
            _ => None,
        })
        .collect();
    assert_eq!(found.len(), 1, "one toggle {wanted}");
    found.into_iter().next().unwrap()
}

fn sub(entries: &[OptionEntry], wanted: &str) -> Vec<OptionEntry> {
    let found: Vec<_> = entries
        .iter()
        .filter_map(|e| match e {
            OptionEntry::Submenu { text, entries } if text == wanted => Some(entries.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(found.len(), 1, "one submenu {wanted}");
    found.into_iter().next().unwrap()
}

fn has_submenu(entries: &[OptionEntry]) -> bool {
    entries
        .iter()
        .any(|e| matches!(e, OptionEntry::Submenu { .. }))
}

#[test]
fn attributes_list_in_the_order_made_a_known_items_guid_a_submenu_a_dash_a_separator_others_toggles_ticked_unless_0(
) {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    let drawers = new_item(&mut rt, "Drawers", "{11111111-0000-0000-0000-000000000001}");
    add(&mut rt, drawers, "Animate &Config Drawer", "0");
    add(&mut rt, root, "On", "1");
    add(&mut rt, root, "Off", "0");
    add(&mut rt, root, "-", "-");
    add(&mut rt, root, "Empty", "");
    add(&mut rt, root, "Big", "1000");
    add(&mut rt, root, "Words", "abc");
    add(&mut rt, root, "Equalizer\tAlt+G", "1");
    add(
        &mut rt,
        root,
        "My &Drawers",
        "11111111-0000-0000-0000-000000000001",
    );

    let entries = options(&rt);
    assert_eq!(
        texts(&entries),
        [
            "On",
            "Off",
            "-",
            "Empty",
            "Big",
            "Words",
            "Equalizer",
            "My Drawers >"
        ]
    );
    assert_eq!(checks(&entries), [true, false, false, true, false, true]);
    let (key, checked, _) = toggle(&sub(&entries, "My Drawers"), "Animate Config Drawer");
    assert!(!checked);
    assert_eq!(
        key,
        OptionKey {
            item: guid_of(&rt, drawers),
            attribute: "animate &config drawer".to_string()
        }
    );
}

#[test]
fn a_skin_that_never_asked_for_the_options_item_lists_nothing_and_building_the_list_makes_no_item()
{
    let mut rt = loaded();
    let other = new_item(&mut rt, "Other", "{22222222-0000-0000-0000-000000000002}");
    add(&mut rt, other, "Shown", "1");

    assert!(options(&rt).is_empty());
    assert_eq!(rt.config_item(OPTIONS_MENU, None), None);
}

#[test]
fn an_attribute_only_read_with_getattribute_is_not_listed() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    rt.maki_call(Handle::Made(root), "getattribute", &[Value::str("Read")]);
    add(&mut rt, root, "Made", "1");

    assert_eq!(texts(&options(&rt)), ["Made"]);
}

#[test]
fn submenus_back_to_an_ancestor_deeper_than_eight_or_empty_are_left_out_as_are_stray_separators() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    let lp = new_item(&mut rt, "Loop", "{33333333-0000-0000-0000-000000000001}");
    add(&mut rt, lp, "Back", OPTIONS_MENU);
    let loop_guid = guid_of(&rt, lp);
    add(&mut rt, lp, "Self", &loop_guid);
    add(&mut rt, lp, "Kept", "1");
    let empty = new_item(&mut rt, "Empty", "{33333333-0000-0000-0000-000000000002}");
    add(&mut rt, empty, "-", "-");
    // ten levels, each holding the next and a toggle
    let levels: Vec<usize> = (1..=10)
        .map(|i| {
            new_item(
                &mut rt,
                &format!("L{i}"),
                &format!("{{44444444-0000-0000-0000-0000000000{i:02}}}"),
            )
        })
        .collect();
    for pair in levels.windows(2) {
        let next = guid_of(&rt, pair[1]);
        add(&mut rt, pair[0], "Next", &next);
    }
    for l in &levels {
        add(&mut rt, *l, "Leaf", "0");
    }
    add(&mut rt, root, "-", "-");
    add(&mut rt, root, "Loop", &loop_guid);
    add(&mut rt, root, "sep1", "-");
    let empty_guid = guid_of(&rt, empty);
    add(&mut rt, root, "Empty", &empty_guid);
    add(&mut rt, root, "sep2", "-");
    let deep_guid = guid_of(&rt, levels[0]);
    add(&mut rt, root, "Deep", &deep_guid);
    add(&mut rt, root, "sep3", "-");

    let entries = options(&rt);
    assert_eq!(texts(&entries), ["Loop >", "-", "Deep >"]);
    assert_eq!(texts(&sub(&entries, "Loop")), ["Kept"]);
    let mut depth = 1;
    let mut level = sub(&entries, "Deep");
    while has_submenu(&level) {
        level = sub(&level, "Next");
        depth += 1;
    }
    assert_eq!(texts(&level), ["Leaf"]);
    assert_eq!(depth, 8);
}

#[test]
fn a_toggle_writes_1_over_0_or_empty_and_0_over_anything_else_kept_in_stored_settings_as_one_settings_change(
) {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    for (name, value) in [
        ("Zero", "0"),
        ("Empty", ""),
        ("Big", "1000"),
        ("Words", "abc"),
    ] {
        add(&mut rt, root, name, value);
    }
    for (name, after) in [("Zero", "1"), ("Empty", "1"), ("Big", "0"), ("Words", "1")] {
        let settings = rt.settings_version();
        let config = rt.config_version();
        let (key, _, _) = toggle(&options(&rt), name);
        assert!(rt.toggle_skin_option(&key));
        assert_eq!(data(&rt, root, &name.to_lowercase()), after);
        assert_eq!(
            rt.stored_settings.get(name).map(String::as_str),
            Some(after)
        );
        assert_eq!(rt.settings_version(), settings + 1);
        assert_ne!(rt.config_version(), config);
        assert_eq!(toggle(&options(&rt), name).1, after != "0");
    }
}

#[test]
fn toggling_targets_the_exact_item_the_same_attribute_name_in_another_item_is_untouched() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    add(&mut rt, root, "Shared", "0");
    let other = new_item(&mut rt, "Other", "{55555555-0000-0000-0000-000000000001}");
    let twin = add(&mut rt, other, "Shared", "0");

    let (key, _, _) = toggle(&options(&rt), "Shared");
    rt.toggle_skin_option(&key);

    assert_eq!(data(&rt, root, "shared"), "1");
    assert_eq!(rt.attribute_data(twin), "0");
}

#[test]
fn toggling_a_separator_a_submenu_or_an_unknown_key_changes_nothing_and_says_so() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    let sub = new_item(&mut rt, "Sub", "{66666666-0000-0000-0000-000000000001}");
    add(&mut rt, sub, "In", "1");
    add(&mut rt, root, "A", "1");
    add(&mut rt, root, "Line", "-");
    let sub_guid = guid_of(&rt, sub);
    add(&mut rt, root, "Sub", &sub_guid);
    let settings = rt.settings_version();
    let guid = guid_of(&rt, root);
    let key = |item: &str, attribute: &str| OptionKey {
        item: item.to_string(),
        attribute: attribute.to_string(),
    };

    assert!(!rt.toggle_skin_option(&key(&guid, "line")));
    assert!(!rt.toggle_skin_option(&key(&guid, "sub")));
    assert!(!rt.toggle_skin_option(&key(&guid, "nothing")));
    assert!(!rt.toggle_skin_option(&key("{00000000-0000-0000-0000-000000000000}", "a")));
    assert_eq!(data(&rt, root, "line"), "-");
    assert_eq!(data(&rt, root, "sub"), sub_guid);
    assert_eq!(rt.settings_version(), settings);
}

#[test]
fn getitem_by_guid_before_newitem_with_that_guid_gives_one_item_so_its_attributes_show_as_a_submenu(
) {
    let mut rt = loaded();
    let guid = "{77777777-0000-0000-0000-000000000001}";
    let early = get_item(&mut rt, guid);
    let root = get_item(&mut rt, OPTIONS_MENU);
    add(&mut rt, root, "Later", guid);
    let made = new_item(&mut rt, "Later", guid);
    add(&mut rt, made, "Inside", "1");

    assert_eq!(early, made);
    assert_eq!(get_item(&mut rt, "Later"), made);
    assert_eq!(texts(&sub(&options(&rt), "Later")), ["Inside"]);
}

#[test]
fn a_guid_that_names_no_item_is_left_out_and_cannot_be_flipped() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    add(&mut rt, root, "Kept", "1");
    let lost = add(
        &mut rt,
        root,
        "Lost",
        "{99999999-0000-0000-0000-000000000001}",
    );
    add(
        &mut rt,
        root,
        "Bare",
        "99999999-0000-0000-0000-000000000002",
    );

    assert_eq!(texts(&options(&rt)), ["Kept"]);
    let key = OptionKey {
        item: guid_of(&rt, root),
        attribute: "lost".to_string(),
    };
    assert!(!rt.toggle_skin_option(&key));
    assert_eq!(
        rt.attribute_data(lost),
        "{99999999-0000-0000-0000-000000000001}"
    );
    assert_eq!(rt.stored_settings.get("Lost"), None);
}

#[test]
fn an_attribute_read_before_it_is_made_takes_the_default_and_the_place_newattribute_gives_it() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    rt.maki_call(Handle::Made(root), "getattribute", &[Value::str("Late")]);
    add(&mut rt, root, "First", "0");
    let late = add(&mut rt, root, "Late", "1");

    assert_eq!(rt.attribute_data(late), "1");
    assert_eq!(texts(&options(&rt)), ["First", "Late"]);
    assert!(toggle(&options(&rt), "Late").1);
}

#[test]
fn an_item_asked_for_by_name_before_newitem_gives_it_a_guid_is_the_one_the_guid_names() {
    let mut rt = loaded();
    let early = get_item(&mut rt, "Named");
    let guid = "{88888888-0000-0000-0000-000000000001}";
    let made = new_item(&mut rt, "Named", guid);
    add(&mut rt, made, "Inside", "1");
    let root = get_item(&mut rt, OPTIONS_MENU);
    add(&mut rt, root, "Named", guid);

    assert_eq!(early, made);
    assert_eq!(guid_of(&rt, made), guid);
    assert_eq!(get_item(&mut rt, guid), made);
    assert_eq!(texts(&sub(&options(&rt), "Named")), ["Inside"]);
}

#[test]
fn a_button_bound_to_an_option_reads_its_value_as_the_menu_does() {
    let mut rt = loaded();
    let root = get_item(&mut rt, OPTIONS_MENU);
    add(&mut rt, root, "Spaced", " 1");
    add(&mut rt, root, "Two", "2");

    assert!(rt.config_flag("Spaced"));
    assert_eq!(rt.config_value("Spaced"), 1);
    assert!(!rt.config_flag("Two"), "a button on value 1 is off at 2");
    assert_eq!(rt.config_value("Two"), 2);
    assert_eq!(checks(&options(&rt)), [true, true]);
}

#[test]
fn the_version_moves_when_an_item_or_attribute_is_made_or_a_value_changes_and_stays_on_an_unchanged_setdata(
) {
    let mut rt = loaded();
    let mut last = rt.config_version();
    let mut moved = |rt: &SkinRuntime| {
        let m = rt.config_version() != last;
        last = rt.config_version();
        m
    };

    let root = get_item(&mut rt, OPTIONS_MENU);
    assert!(moved(&rt));
    rt.maki_call(Handle::Made(root), "getattribute", &[Value::str("A")]);
    assert!(moved(&rt));
    let a = add(&mut rt, root, "A", "1");
    assert!(moved(&rt));
    add(&mut rt, root, "A", "1");
    assert!(!moved(&rt));
    rt.maki_call(Handle::Made(a), "setdata", &[Value::str("0")]);
    assert!(moved(&rt));
    rt.maki_call(Handle::Made(a), "setdata", &[Value::str("0")]);
    assert!(!moved(&rt));
    get_item(&mut rt, OPTIONS_MENU);
    assert!(!moved(&rt));
}

#[test]
fn a_checkbox_naming_a_setting_with_another_note_in_brackets_at_its_end_works_the_setting_the_script_made(
) {
    let mut rt = loaded();
    let drawers = new_item(&mut rt, "Drawers", "{C338B30F-2A04-4b10-871F-4E9D52D62806}");
    let animate = add(
        &mut rt,
        drawers,
        "Animate Video/Vis Drawer (disabled if opacity < 100%)",
        "0",
    );
    let as_written = "animate video/vis drawer (disabled if opacity = 100%)";

    rt.set_config_flag(as_written, true);

    assert_eq!(rt.attribute_data(animate), "1");
    assert!(rt.config_flag(as_written));
}

#[test]
fn a_name_that_fits_two_settings_once_their_notes_are_left_out_works_neither() {
    let mut rt = loaded();
    let drawers = new_item(&mut rt, "Drawers", "{C338B30F-2A04-4b10-871F-4E9D52D62806}");
    let one = add(&mut rt, drawers, "Animate (fast)", "0");
    let two = add(&mut rt, drawers, "Animate (slow)", "0");

    rt.set_config_flag("animate (medium)", true);

    assert_eq!(
        [rt.attribute_data(one), rt.attribute_data(two)],
        ["0".to_string(), "0".to_string()]
    );
}
