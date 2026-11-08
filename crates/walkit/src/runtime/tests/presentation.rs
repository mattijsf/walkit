// SPDX-License-Identifier: GPL-3.0-or-later

//! How the host shows a skin's windows, as one value (the reference's `PresentationTest`).

use super::runtime;
use crate::host::Presentation;
use crate::runtime::SkinRuntime;

const PLAYER_XML: &str = concat!(
    r#"<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="40">"#,
    r#"<button id="min" action="MINIMIZE" x="0" y="0" w="20" h="20"/>"#,
    r#"</layout></container>"#,
);

fn loaded() -> SkinRuntime {
    let mut rt = runtime(PLAYER_XML);
    rt.load();
    let p = Presentation {
        view_size: Some((300, 600)),
        ..rt.presentation()
    };
    rt.set_presentation(&p);
    rt
}

#[test]
fn the_presentation_reads_and_sets_the_window_settings_as_one_value() {
    let mut rt = runtime(PLAYER_XML);
    assert_eq!(rt.presentation(), Presentation::default());

    let p = Presentation {
        view_size: Some((200, 400)),
        viewport: (800, 600),
        columns: 2,
        column_width: 150,
        stretch_last: false,
        touch_slop: 9,
        free_windows: false,
        touch: true,
        skin_list_font: true,
        list_row_extra: 6,
    };
    rt.set_presentation(&p);
    assert_eq!(rt.presentation(), p);
    assert!(rt.touch);
    assert!(rt.skin_list_font);
    assert_eq!(rt.list_row_extra, 6);
    assert_eq!(rt.view_size(), Some((200, 400)));
    assert_eq!((rt.viewport_w, rt.viewport_h), (800, 600));
    assert_eq!((rt.columns, rt.column_width, rt.touch_slop), (2, 150, 9));
    assert!(!rt.stretch_last);

    let p = Presentation {
        view_size: None,
        ..rt.presentation()
    };
    rt.set_presentation(&p);
    assert_eq!(rt.view_size(), None);
    assert_eq!(
        rt.viewport(),
        (800, 600),
        "the screen scripts see before there is a view"
    );
}

#[test]
fn setting_the_same_presentation_again_lays_nothing_out_again() {
    let mut rt = loaded();
    rt.ensure_layout();
    let before = rt.change_count;
    let same = rt.presentation();
    rt.set_presentation(&same);
    assert_eq!(rt.change_count, before);
    let two = Presentation {
        columns: 2,
        ..rt.presentation()
    };
    rt.set_presentation(&two);
    assert!(rt.change_count > before);
}
