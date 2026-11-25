// SPDX-License-Identifier: GPL-3.0-or-later

//! A standard frame's content, placed by the frame's behavior or by a skin's own script.

use super::runtime;

/// A wasabi-like standard frame whose behavior places the content at 5,15 less 10 by 34, and a
/// window that shows `content.group` in it. `redefinition` comes after the frame's groupdef.
fn player(redefinition: &str) -> String {
    format!(
        r#"
<groupdef id="frame.status" xuitag="Wasabi:StandardFrame:Status" behavior="standardframe"
  contentrect="5,15,-10,-34"/>
{redefinition}
<groupdef id="content.group"><button id="inside" x="0" y="0" w="0" h="0" relatw="1" relath="1"/></groupdef>
<container id="main" name="t" default_visible="1"><layout id="normal" w="300" h="200">
<Wasabi:StandardFrame:Status x="0" y="0" w="0" h="0" relatw="1" relath="1" content="content.group"/>
</layout></container>"#
    )
}

#[test]
fn the_frame_places_its_content_in_its_content_area() {
    let mut rt = runtime(&player(""));
    rt.load();
    rt.ensure_layout();
    let rect = rt.object_info("inside").expect("the content").rect;
    assert_eq!(
        (rect.left, rect.top, rect.width, rect.height),
        (5, 15, 290, 166)
    );
}

#[test]
fn a_redefinition_without_the_base_content_leaves_the_content_to_its_own_script() {
    // as MMD3 redefines wasabi.standardframe.statusbar: no base objects or scripts, a script
    // of its own that places the content where the skin's frame has room for it
    let mut rt = runtime(&player(
        r#"<groupdef id="frame.status" inherit_content="0"/>"#,
    ));
    rt.load();
    rt.ensure_layout();
    assert!(rt.object_info("inside").is_none());
}
