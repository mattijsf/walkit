// SPDX-License-Identifier: GPL-3.0-or-later

//! The unknown objects a host is told of: those in what the windows draw, and no others.

use super::runtime;
use crate::host::UnknownObject;
use crate::runtime::SkinRuntime;

fn loaded(player_xml: &str) -> SkinRuntime {
    let mut rt = runtime(player_xml);
    rt.load();
    rt
}

fn tags(rt: &mut SkinRuntime) -> Vec<(String, String, String)> {
    rt.unknown_objects()
        .into_iter()
        .map(|u| (u.tag, u.id, u.container))
        .collect()
}

#[test]
fn an_unknown_tag_in_the_shown_layout_is_listed_with_its_window() {
    let mut rt = loaded(concat!(
        r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="50">"#,
        r#"<group id="body" x="0" y="0" w="100" h="50">"#,
        r#"<Wasabi:StandardFrame:Status id="frame" x="0" y="0" w="100" h="50"/>"#,
        r#"</group>"#,
        r#"<layer id="known" x="0" y="0" w="10" h="10"/>"#,
        r#"</layout></container>"#,
    ));
    let main = rt.main_window().expect("a main window");
    assert_eq!(
        rt.unknown_objects(),
        vec![UnknownObject {
            tag: "wasabi:standardframe:status".into(),
            id: "frame".into(),
            window: main,
            container: "main".into(),
        }]
    );
}

#[test]
fn unknown_tags_the_windows_do_not_draw_are_not_listed() {
    let mut rt = loaded(concat!(
        // a shade layout that is not the current one
        r#"<container id="main" name="t" default_visible="1">"#,
        r#"<layout id="normal" w="100" h="50">"#,
        r#"<group id="hidden" x="0" y="0" w="100" h="50" visible="0">"#,
        r#"<Wasabi:StandardFrame:NoStatus id="in.hidden.group"/>"#,
        r#"</group>"#,
        r#"<Wasabi:Frame id="hidden.itself" visible="0"/>"#,
        r#"</layout>"#,
        r#"<layout id="shade" w="100" h="14"><Wasabi:StandardFrame:Status id="in.shade"/></layout>"#,
        r#"</container>"#,
        // a closed container
        r#"<container id="prefs" name="Preferences" default_visible="0">"#,
        r#"<layout id="normal" w="100" h="50"><Wasabi:CheckBox id="in.prefs"/></layout>"#,
        r#"</container>"#,
    ));
    // the hidden ones are there, hidden
    for id in ["in.hidden.group", "hidden.itself"] {
        assert!(rt.object_info(id).is_some(), "{id} is built");
    }
    assert_eq!(tags(&mut rt), vec![]);
    // opened, the closed container's window shows its unknown tag
    assert!(rt.open_window("prefs"));
    assert_eq!(
        tags(&mut rt),
        vec![("wasabi:checkbox".into(), "in.prefs".into(), "prefs".into())]
    );
}

#[test]
fn a_tag_a_xuitag_defines_is_not_listed() {
    let mut rt = loaded(concat!(
        r#"<groupdef id="my.frame" xuitag="Wasabi:StandardFrame:Status">"#,
        r#"<layer id="frame.bg" x="0" y="0" w="10" h="10"/>"#,
        r#"</groupdef>"#,
        r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="50">"#,
        r#"<Wasabi:StandardFrame:Status id="frame" x="0" y="0" w="100" h="50"/>"#,
        r#"</layout></container>"#,
    ));
    assert_eq!(tags(&mut rt), vec![]);
}

#[test]
fn a_known_class_the_engine_does_not_draw_is_not_listed() {
    let mut rt = loaded(concat!(
        r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="100" h="50">"#,
        r#"<edit id="field" x="0" y="0" w="100" h="20"/>"#,
        r#"</layout></container>"#,
    ));
    assert_eq!(tags(&mut rt), vec![]);
}
